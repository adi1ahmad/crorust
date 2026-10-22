//! Unpacks Apple flat packages (.pkg / XAR archives) and inflates
//! the internal CPIO Payload into a macOS SDK sysroot layout.

use anyhow::{bail, Context, Result};
use flate2::read::ZlibDecoder;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ApplePkgExtractor;

impl ApplePkgExtractor {
    /// Extracts an Apple .pkg into the target destination directory.
    pub fn extract_pkg(pkg_path: &Path, destination: &Path) -> Result<()> {
        let staging = destination.with_extension("pkg_staging");
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        std::fs::create_dir_all(&staging)?;

        let res = Self::extract_pkg_internal(pkg_path, &staging);
        if let Err(e) = res {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }

        // Locate MacOSX.sdk or MacOSX*.sdk inside staging
        let sdk_source = Self::find_sdk_directory(&staging)
            .context("Failed to locate MacOSX.sdk inside extracted Apple package")?;

        if destination.exists() {
            let _ = std::fs::remove_dir_all(destination);
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&sdk_source, destination)
            .or_else(|_| crorust_common::copy_dir_all(&sdk_source, destination))
            .with_context(|| {
                format!(
                    "Failed moving extracted SDK from `{}` to `{}`",
                    sdk_source.display(),
                    destination.display()
                )
            })?;

        let _ = std::fs::remove_dir_all(&staging);
        Ok(())
    }

    fn extract_pkg_internal(pkg_path: &Path, staging: &Path) -> Result<()> {
        // Primary cross-platform path: Parse XAR TOC, decompress pbzx payload chunks via xz2,
        // and inflate via in-tree Pure-Rust CpioExtractor.
        if let Ok(()) = Self::extract_xar_pbzx(pkg_path, staging) {
            return Ok(());
        }

        // Fallback: bsdtar (supports XAR and CPIO out-of-the-box on macOS / some Linux distros)
        if which::which("bsdtar").is_ok() {
            let xar_dir = staging.join("xar");
            let _ = std::fs::create_dir_all(&xar_dir);

            if let Ok(status) = Command::new("bsdtar")
                .arg("-xf")
                .arg(pkg_path)
                .arg("-C")
                .arg(&xar_dir)
                .status()
            {
                if status.success() {
                    let payload = xar_dir.join("Payload");
                    if payload.exists() {
                        let unpacked = staging.join("unpacked");
                        let _ = std::fs::create_dir_all(&unpacked);
                        if let Ok(status2) = Command::new("bsdtar")
                            .arg("-xf")
                            .arg(&payload)
                            .arg("-C")
                            .arg(&unpacked)
                            .status()
                        {
                            if status2.success() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        Self::extract_xar_pbzx(pkg_path, staging)
    }

    fn extract_xar_pbzx(pkg_path: &Path, staging: &Path) -> Result<()> {
        let mut file = File::open(pkg_path)?;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != b"xar!" {
            bail!("Invalid Apple package format: missing 'xar!' magic bytes");
        }

        let mut header_buf = [0u8; 24];
        file.read_exact(&mut header_buf)?;
        let header_size = u16::from_be_bytes([header_buf[0], header_buf[1]]) as u64;
        let toc_compressed_len = u64::from_be_bytes(header_buf[4..12].try_into().unwrap());

        file.seek(SeekFrom::Start(header_size))?;
        let mut toc_compressed = vec![0u8; toc_compressed_len as usize];
        file.read_exact(&mut toc_compressed)?;

        let mut decoder = ZlibDecoder::new(&toc_compressed[..]);
        let mut toc_xml = String::new();
        decoder
            .read_to_string(&mut toc_xml)
            .context("Failed to decompress XAR Table of Contents")?;

        let heap_offset = header_size + toc_compressed_len;
        let payload_info = Self::parse_payload_location_from_toc(&toc_xml)
            .context("Failed to locate Payload metadata in XAR TOC")?;

        file.seek(SeekFrom::Start(heap_offset + payload_info.offset))?;

        let unpacked = staging.join("unpacked");
        std::fs::create_dir_all(&unpacked)?;

        // Check if payload starts with pbzx stream
        let mut p_magic = [0u8; 4];
        file.read_exact(&mut p_magic)?;

        if &p_magic == b"pbzx" {
            // pbzx stream format:
            // 4 bytes: 'pbzx'
            // 8 bytes: flags (big-endian u64)
            // Sequence of chunks:
            //   8 bytes: uncompressed length (be u64)
            //   8 bytes: compressed length (be u64)
            //   compressed bytes
            let mut flags_buf = [0u8; 8];
            file.read_exact(&mut flags_buf)?;

            let cpio_path = staging.join("payload.cpio");
            {
                let mut cpio_out = std::io::BufWriter::new(File::create(&cpio_path)?);
                let mut chunk_hdr = [0u8; 16];

                while file.read_exact(&mut chunk_hdr).is_ok() {
                    let _uncompressed_len = u64::from_be_bytes(chunk_hdr[0..8].try_into().unwrap());
                    let compressed_len = u64::from_be_bytes(chunk_hdr[8..16].try_into().unwrap());

                    let mut chunk_buf = vec![0u8; compressed_len as usize];
                    file.read_exact(&mut chunk_buf)?;

                    // Check if chunk is XZ compressed
                    if chunk_buf.starts_with(b"\xfd7zX") {
                        let mut decompressor = xz2::read::XzDecoder::new(&chunk_buf[..]);
                        std::io::copy(&mut decompressor, &mut cpio_out)
                            .context("Failed decompressing XZ chunk in pbzx payload")?;
                    } else {
                        use std::io::Write;
                        cpio_out.write_all(&chunk_buf)?;
                    }
                }
            }

            // Inflate CPIO archive into `unpacked` directory
            Self::unpack_cpio_archive(&cpio_path, &unpacked)?;
            let _ = std::fs::remove_file(&cpio_path);
            Ok(())
        } else {
            // Raw uncompressed or gzip-compressed payload
            file.seek(SeekFrom::Start(heap_offset + payload_info.offset))?;
            let raw_path = staging.join("payload.raw");
            {
                let mut raw_out = std::io::BufWriter::new(File::create(&raw_path)?);
                let mut take_reader = (&mut file).take(payload_info.length);
                std::io::copy(&mut take_reader, &mut raw_out)?;
            }
            Self::unpack_cpio_archive(&raw_path, &unpacked)?;
            let _ = std::fs::remove_file(&raw_path);
            Ok(())
        }
    }

    fn unpack_cpio_archive(cpio_path: &Path, destination: &Path) -> Result<()> {
        // Fast primary path: Pure-Rust in-tree CPIO extraction with Windows symlink resolution
        if let Ok(file) = File::open(cpio_path) {
            let reader = std::io::BufReader::new(file);
            match CpioExtractor::extract(reader, destination) {
                Ok(()) => {
                    if let Ok(mut entries) = std::fs::read_dir(destination) {
                        if entries.next().is_some() {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[crorust] Pure-Rust CpioExtractor failed: {e:#}; attempting system toolchain fallbacks"
                    );
                    let _ = std::fs::remove_dir_all(destination);
                    let _ = std::fs::create_dir_all(destination);
                }
            }
        }

        // Secondary fallback 1: system bsdtar
        if which::which("bsdtar").is_ok() {
            let status = Command::new("bsdtar")
                .arg("-xf")
                .arg(cpio_path)
                .arg("-C")
                .arg(destination)
                .status();
            if let Ok(s) = status {
                if s.success() {
                    return Ok(());
                }
            }
        }

        // Secondary fallback 2: system cpio
        if which::which("cpio").is_ok() {
            let cpio_file = File::open(cpio_path)?;
            let mut child = Command::new("cpio")
                .arg("-idm")
                .current_dir(destination)
                .stdin(cpio_file)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn cpio")?;

            let status = child.wait()?;
            if status.success() {
                return Ok(());
            }
        }

        // Note: 7-Zip is intentionally excluded as a CPIO fallback because 7-Zip on Windows
        // does not handle POSIX symlinks and writes symlink targets as raw text files,
        // which corrupts .tbd library stubs for the linker.
        bail!(
            "Could not extract CPIO payload archive. In-tree CPIO extractor failed and system `bsdtar` or `cpio` were unavailable."
        );
    }

    fn find_sdk_directory(root: &Path) -> Option<PathBuf> {
        Self::find_named_sdk_directory(root, "MacOSX")
    }

    pub fn find_named_sdk_directory(root: &Path, sdk_prefix: &str) -> Option<PathBuf> {
        let exact_name = format!("{sdk_prefix}.sdk");
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                let name = entry.file_name().to_string_lossy();
                let name_matches =
                    name == exact_name || (name.starts_with(sdk_prefix) && name.ends_with(".sdk"));
                let sdk_structures_exist = entry
                    .path()
                    .join("System")
                    .join("Library")
                    .join("Frameworks")
                    .exists()
                    || entry.path().join("usr").join("lib").exists()
                    || entry.path().join("SDKSettings.plist").exists()
                    || entry.path().join("SDKSettings.json").exists();
                if name_matches && sdk_structures_exist {
                    return Some(entry.path().to_path_buf());
                }
            }
        }
        None
    }

    /// Imports Apple SDKs from an Xcode installation directory (.app) or archive (.xip / .pkg)
    /// into the provided cache sysroots directory.
    pub fn import_xcode_sdks(
        xcode_path: &Path,
        sysroots_cache_dir: &Path,
    ) -> Result<Vec<(String, PathBuf)>> {
        let mut imported = Vec::new();
        std::fs::create_dir_all(sysroots_cache_dir)?;

        if xcode_path.is_dir() {
            for (prefix, target_name) in [
                ("iPhoneOS", "iPhoneOS.sdk"),
                ("iPhoneSimulator", "iPhoneSimulator.sdk"),
                ("MacOSX", "MacOSX.sdk"),
            ] {
                if let Some(sdk_src) = Self::find_named_sdk_directory(xcode_path, prefix) {
                    let dest = sysroots_cache_dir.join(target_name);
                    if dest.exists() {
                        let _ = std::fs::remove_dir_all(&dest);
                    }
                    crorust_common::copy_dir_all(&sdk_src, &dest)?;
                    std::fs::write(dest.join(".complete"), b"ok\n")?;
                    imported.push((target_name.to_string(), dest));
                }
            }
        } else if xcode_path.is_file() {
            let staging = sysroots_cache_dir.join(".xcode_import_staging");
            if staging.exists() {
                let _ = std::fs::remove_dir_all(&staging);
            }
            std::fs::create_dir_all(&staging)?;

            let res = Self::extract_pkg_internal(xcode_path, &staging);
            if let Err(e) = res {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(e).context("Failed extracting Xcode archive");
            }

            for (prefix, target_name) in [
                ("iPhoneOS", "iPhoneOS.sdk"),
                ("iPhoneSimulator", "iPhoneSimulator.sdk"),
                ("MacOSX", "MacOSX.sdk"),
            ] {
                if let Some(sdk_src) = Self::find_named_sdk_directory(&staging, prefix) {
                    let dest = sysroots_cache_dir.join(target_name);
                    if dest.exists() {
                        let _ = std::fs::remove_dir_all(&dest);
                    }
                    std::fs::rename(&sdk_src, &dest)
                        .or_else(|_| crorust_common::copy_dir_all(&sdk_src, &dest))?;
                    std::fs::write(dest.join(".complete"), b"ok\n")?;
                    imported.push((target_name.to_string(), dest));
                }
            }

            let _ = std::fs::remove_dir_all(&staging);
        } else {
            bail!(
                "Specified Xcode path does not exist: {}",
                xcode_path.display()
            );
        }

        Ok(imported)
    }

    fn parse_payload_location_from_toc(xml: &str) -> Option<PayloadLocation> {
        for file_block in xml.split("<file") {
            if file_block.contains("<name>Payload</name>") {
                let offset_start = file_block.find("<offset>")? + "<offset>".len();
                let offset_end = file_block[offset_start..].find("</offset>")? + offset_start;
                let offset: u64 = file_block[offset_start..offset_end].trim().parse().ok()?;

                let len_start = file_block.find("<length>")? + "<length>".len();
                let len_end = file_block[len_start..].find("</length>")? + len_start;
                let length: u64 = file_block[len_start..len_end].trim().parse().ok()?;

                return Some(PayloadLocation { offset, length });
            }
        }
        None
    }
}

struct PayloadLocation {
    offset: u64,
    length: u64,
}

struct CpioHeaderMeta {
    ino: u64,
    mode: u32,
    filesize: u64,
    namesize: usize,
    pad_name_boundary: Option<usize>,
    pad_data: bool,
}

pub struct CpioExtractor;

impl CpioExtractor {
    pub fn extract<R: Read>(mut reader: R, destination: &Path) -> Result<()> {
        std::fs::create_dir_all(destination)?;
        let canonical_dest =
            dunce::canonicalize(destination).unwrap_or_else(|_| destination.to_path_buf());

        let mut deferred_symlinks: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut seen_inodes: std::collections::HashMap<u64, PathBuf> =
            std::collections::HashMap::new();
        let mut magic = [0u8; 6];

        loop {
            match reader.read_exact(&mut magic) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e).context("Error reading CPIO magic"),
            }

            let meta = if &magic == b"070707" {
                // POSIX odc format (76 bytes total header): read remaining 70 bytes
                let mut rest = [0u8; 70];
                reader
                    .read_exact(&mut rest)
                    .context("Truncated CPIO odc header")?;

                let ino_str =
                    std::str::from_utf8(&rest[6..12]).context("Malformed CPIO odc ino")?;
                let ino = u64::from_str_radix(ino_str.trim(), 8)
                    .context("Failed parsing CPIO odc ino")?;

                let mode_str =
                    std::str::from_utf8(&rest[12..18]).context("Malformed CPIO odc mode")?;
                let mode = u32::from_str_radix(mode_str.trim(), 8)
                    .context("Failed parsing CPIO odc mode")?;

                let namesize_str =
                    std::str::from_utf8(&rest[53..59]).context("Malformed CPIO odc namesize")?;
                let namesize = usize::from_str_radix(namesize_str.trim(), 8)
                    .context("Failed parsing CPIO odc namesize")?;

                let filesize_str =
                    std::str::from_utf8(&rest[59..70]).context("Malformed CPIO odc filesize")?;
                let filesize = u64::from_str_radix(filesize_str.trim(), 8)
                    .context("Failed parsing CPIO odc filesize")?;

                CpioHeaderMeta {
                    ino,
                    mode,
                    filesize,
                    namesize,
                    pad_name_boundary: None,
                    pad_data: false,
                }
            } else if &magic == b"070701" || &magic == b"070702" {
                // SVR4 newc / crc format (110 bytes total header): read remaining 104 bytes
                let mut rest = [0u8; 104];
                reader
                    .read_exact(&mut rest)
                    .context("Truncated CPIO newc header")?;

                let ino_str =
                    std::str::from_utf8(&rest[0..8]).context("Malformed CPIO newc ino")?;
                let ino = u64::from_str_radix(ino_str.trim(), 16)
                    .context("Failed parsing CPIO newc ino")?;

                let mode_str =
                    std::str::from_utf8(&rest[8..16]).context("Malformed CPIO newc mode")?;
                let mode = u32::from_str_radix(mode_str.trim(), 16)
                    .context("Failed parsing CPIO newc mode")?;

                let filesize_str =
                    std::str::from_utf8(&rest[48..56]).context("Malformed CPIO newc filesize")?;
                let filesize = u64::from_str_radix(filesize_str.trim(), 16)
                    .context("Failed parsing CPIO newc filesize")?;

                let namesize_str =
                    std::str::from_utf8(&rest[88..96]).context("Malformed CPIO newc namesize")?;
                let namesize = usize::from_str_radix(namesize_str.trim(), 16)
                    .context("Failed parsing CPIO newc namesize")?;

                CpioHeaderMeta {
                    ino,
                    mode,
                    filesize,
                    namesize,
                    pad_name_boundary: Some(110),
                    pad_data: true,
                }
            } else {
                bail!("Invalid CPIO magic: {:?}", String::from_utf8_lossy(&magic));
            };

            let mut name_buf = vec![0u8; meta.namesize];
            reader.read_exact(&mut name_buf)?;

            if let Some(header_len) = meta.pad_name_boundary {
                let header_plus_name = header_len + meta.namesize;
                let name_pad = ((header_plus_name + 3) & !3) - header_plus_name;
                if name_pad > 0 {
                    let mut pad = [0u8; 4];
                    reader.read_exact(&mut pad[..name_pad])?;
                }
            }

            let raw_name = String::from_utf8_lossy(&name_buf)
                .trim_matches('\0')
                .to_string();

            if raw_name == "TRAILER!!!" {
                break;
            }

            let normalized = raw_name.replace('\\', "/");
            let mut clean = normalized.as_str();
            while let Some(stripped) = clean.strip_prefix("./") {
                clean = stripped;
            }
            while let Some(stripped) = clean.strip_prefix('/') {
                clean = stripped;
            }

            if clean.is_empty() || clean == "." {
                if meta.pad_data {
                    Self::skip_payload_padding(&mut reader, meta.filesize)?;
                }
                continue;
            }

            let has_parent_dir = Path::new(clean)
                .components()
                .any(|c| c == std::path::Component::ParentDir);
            if has_parent_dir {
                bail!("Zip slip attempt detected in CPIO entry: {clean}");
            }

            let file_type = meta.mode & 0o170000;

            // Prune unneeded documentation / manual pages that are never used by compilers or linkers
            // and skip entries with Windows-illegal filename characters (such as Perl manpages with `::`).
            if Self::is_skippable_sdk_entry(clean)
                || (cfg!(windows) && Self::has_invalid_windows_filename_chars(clean))
            {
                Self::skip_entry_payload(&mut reader, meta.filesize, file_type, meta.pad_data)?;
                continue;
            }

            // Convert to native OS path separators to prevent Win32 \\?\ path syntax errors (os error 123)
            #[cfg(windows)]
            let clean_native = clean.replace('/', "\\");
            #[cfg(not(windows))]
            let clean_native = clean.to_string();

            let target_path = canonical_dest.join(&clean_native);

            if file_type == 0o040000 {
                std::fs::create_dir_all(&target_path)?;
                if meta.pad_data {
                    Self::skip_payload_padding(&mut reader, meta.filesize)?;
                }
            } else if file_type == 0o120000 {
                let mut link_target_buf = vec![0u8; meta.filesize as usize];
                reader.read_exact(&mut link_target_buf)?;
                if meta.pad_data {
                    Self::skip_payload_padding(&mut reader, meta.filesize)?;
                }

                let link_target_str = String::from_utf8_lossy(&link_target_buf)
                    .trim_matches('\0')
                    .to_string();
                deferred_symlinks.push((target_path, PathBuf::from(link_target_str)));
            } else {
                // Hardlink optimization: in CPIO, repeated inodes emit filesize == 0
                if meta.filesize == 0 && meta.ino != 0 && seen_inodes.contains_key(&meta.ino) {
                    let src = &seen_inodes[&meta.ino];
                    if let Some(parent) = target_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(src, &target_path).with_context(|| {
                        format!(
                            "Failed hard linking `{}` to `{}`",
                            target_path.display(),
                            src.display()
                        )
                    })?;
                    if meta.pad_data {
                        Self::skip_payload_padding(&mut reader, meta.filesize)?;
                    }
                    continue;
                }

                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out_file = std::fs::File::create(&target_path).with_context(|| {
                    format!("Failed creating extracted file `{}`", target_path.display())
                })?;

                let mut take = (&mut reader).take(meta.filesize);
                std::io::copy(&mut take, &mut out_file)?;
                if meta.pad_data {
                    Self::skip_payload_padding(&mut reader, meta.filesize)?;
                }

                if meta.filesize > 0 && meta.ino != 0 {
                    seen_inodes.insert(meta.ino, target_path.clone());
                }

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(meta.mode & 0o777);
                    let _ = std::fs::set_permissions(&target_path, perms);
                }
            }
        }

        Self::resolve_symlinks(deferred_symlinks)?;
        Ok(())
    }

    /// Check if path belongs to SDK documentation or manual pages that are not needed for compiling or linking.
    pub fn is_skippable_sdk_entry(clean: &str) -> bool {
        let p = clean.to_ascii_lowercase();
        p.contains("/share/man/")
            || p.starts_with("share/man/")
            || p.starts_with("usr/share/man/")
            || p.contains("/share/doc/")
            || p.starts_with("share/doc/")
            || p.starts_with("usr/share/doc/")
            || p.contains("/share/info/")
            || p.starts_with("share/info/")
            || p.starts_with("usr/share/info/")
    }

    /// Checks if any segment of the relative entry path contains Windows-reserved characters:
    /// `:`, `<`, `>`, `"`, `|`, `?`, `*`.
    pub fn has_invalid_windows_filename_chars(clean: &str) -> bool {
        clean.split(['/', '\\']).any(|segment| {
            segment
                .chars()
                .any(|c| matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'))
        })
    }

    /// Consumes and discards payload data and padding for skipped archive entries.
    fn skip_entry_payload<R: Read>(
        reader: &mut R,
        filesize: u64,
        file_type: u32,
        pad_data: bool,
    ) -> Result<()> {
        if file_type == 0o040000 {
            // Directory: no payload
            if pad_data {
                Self::skip_payload_padding(reader, filesize)?;
            }
        } else {
            // File or symlink: consume exact payload bytes
            if filesize > 0 {
                let mut take = reader.take(filesize);
                std::io::copy(&mut take, &mut std::io::sink())?;
            }
            if pad_data {
                Self::skip_payload_padding(reader, filesize)?;
            }
        }
        Ok(())
    }

    fn skip_payload_padding<R: Read>(reader: &mut R, filesize: u64) -> Result<()> {
        let pad = ((filesize + 3) & !3) - filesize;
        if pad > 0 {
            let mut pad_buf = [0u8; 4];
            reader.read_exact(&mut pad_buf[..pad as usize])?;
        }
        Ok(())
    }

    fn resolve_symlinks(mut deferred: Vec<(PathBuf, PathBuf)>) -> Result<()> {
        let max_passes = 12;
        for _ in 0..max_passes {
            if deferred.is_empty() {
                break;
            }
            let mut remaining = Vec::new();

            let prev_len = deferred.len();
            for (link_path, target_rel) in deferred {
                if let Some(parent) = link_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                let base_parent = link_path.parent().unwrap_or(Path::new("."));
                #[cfg(windows)]
                let target_clean = PathBuf::from(target_rel.to_string_lossy().replace('/', "\\"));
                #[cfg(not(windows))]
                let target_clean = PathBuf::from(target_rel.to_string_lossy().replace('\\', "/"));
                let direct = base_parent.join(&target_clean);

                let resolved = if direct.exists() {
                    Some(direct)
                } else if let Ok(canon) = dunce::canonicalize(&direct) {
                    Some(canon)
                } else {
                    crorust_common::case_fold_lookup(base_parent, &target_clean)
                };

                let native_ok = {
                    #[cfg(unix)]
                    {
                        if link_path.exists() || link_path.is_symlink() {
                            let _ = std::fs::remove_file(&link_path);
                            let _ = std::fs::remove_dir_all(&link_path);
                        }
                        std::os::unix::fs::symlink(&target_rel, &link_path).is_ok()
                    }
                    #[cfg(windows)]
                    {
                        if let Some(ref target) = resolved {
                            if link_path.exists() || link_path.is_symlink() {
                                let _ = std::fs::remove_file(&link_path);
                                let _ = std::fs::remove_dir_all(&link_path);
                            }
                            if target.is_dir() {
                                std::os::windows::fs::symlink_dir(&target_clean, &link_path).is_ok()
                            } else {
                                // For files on Windows (especially .tbd library stubs), materialize by copying
                                // rather than relying on unprivileged NTFS symlinks which may not resolve across toolchains.
                                false
                            }
                        } else {
                            // Target does not exist on disk yet; defer to next pass
                            false
                        }
                    }
                    #[cfg(not(any(unix, windows)))]
                    {
                        false
                    }
                };

                if native_ok {
                    continue;
                }

                if let Some(target) = resolved {
                    if link_path.exists() || link_path.is_symlink() {
                        let _ = std::fs::remove_file(&link_path);
                        let _ = std::fs::remove_dir_all(&link_path);
                    }
                    if target.is_dir() {
                        // Guard 1: Prevent copying an ancestor directory into its own descendant
                        // (e.g. `Ruby.framework/Versions/2.6/Headers/ruby/ruby -> .`), which would
                        // create a self-referential directory cycle when symlinks are materialized as copies.
                        if let (Ok(canon_parent), Ok(canon_target)) = (
                            dunce::canonicalize(base_parent),
                            dunce::canonicalize(&target),
                        ) {
                            if canon_parent.starts_with(&canon_target) {
                                continue;
                            }
                        }

                        // Guard 2: Skip duplicating top-level `.sdk` directory aliases on Windows
                        // (e.g. `MacOSX14.sdk -> MacOSX14.5.sdk`), since `find_named_sdk_directory`
                        // locates the primary `MacOSX*.sdk` directory directly.
                        let is_sdk_alias = link_path
                            .file_name()
                            .map(|n| n.to_string_lossy().ends_with(".sdk"))
                            .unwrap_or(false)
                            && target
                                .file_name()
                                .map(|n| n.to_string_lossy().ends_with(".sdk"))
                                .unwrap_or(false);
                        if is_sdk_alias {
                            continue;
                        }

                        crorust_common::copy_dir_all(&target, &link_path)?;
                    } else {
                        std::fs::copy(&target, &link_path).with_context(|| {
                            format!(
                                "Failed copying resolved symlink `{}` to `{}`",
                                target.display(),
                                link_path.display()
                            )
                        })?;
                    }
                } else {
                    remaining.push((link_path, target_rel));
                }
            }

            if remaining.len() == prev_len {
                for (link_path, target_rel) in remaining {
                    tracing::warn!(
                        "Dangling symlink in Apple package: `{}` -> `{}`",
                        link_path.display(),
                        target_rel.display()
                    );
                }
                break;
            }
            deferred = remaining;
        }

        Ok(())
    }
}
