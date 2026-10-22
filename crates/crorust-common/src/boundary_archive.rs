use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryCompressionStrategy {
    CompressedDeflate,
    StoredBoundaryAligned(u32),
}

impl EntryCompressionStrategy {
    pub fn boundary_alignment(self) -> u32 {
        match self {
            Self::CompressedDeflate => 1,
            Self::StoredBoundaryAligned(align) => align,
        }
    }

    pub fn is_stored(self) -> bool {
        matches!(self, Self::StoredBoundaryAligned(_))
    }

    pub fn resolved_compression_method(self) -> CompressionMethod {
        match self {
            Self::CompressedDeflate => CompressionMethod::Deflated,
            Self::StoredBoundaryAligned(_) => CompressionMethod::Stored,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveEntryOptions {
    Unaligned,
    Aligned(u16),
    Compressed,
}

impl ArchiveEntryOptions {
    pub fn boundary_alignment(&self) -> u16 {
        match self {
            Self::Aligned(boundary) => *boundary,
            _ => 1,
        }
    }
}

impl From<ArchiveEntryOptions> for EntryCompressionStrategy {
    fn from(opts: ArchiveEntryOptions) -> Self {
        match opts {
            ArchiveEntryOptions::Compressed => EntryCompressionStrategy::CompressedDeflate,
            ArchiveEntryOptions::Aligned(align) => {
                EntryCompressionStrategy::StoredBoundaryAligned(align as u32)
            }
            ArchiveEntryOptions::Unaligned => EntryCompressionStrategy::StoredBoundaryAligned(1),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntryOffset {
    pub header_offset: u64,
    pub data_offset: u64,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub alignment_boundary: u32,
    pub is_stored: bool,
    pub is_directory: bool,
    pub entry_name: String,
}

pub struct AlignedArchiveSink<W: Write + Seek> {
    archive_writer: ZipWriter<W>,
    recorded_offsets: Vec<ArchiveEntryOffset>,
}

impl AlignedArchiveSink<File> {
    pub fn create_target(destination: &Path) -> Result<Self> {
        Self::new_path(destination, false)
    }

    pub fn new_path(destination: &Path, _compress: bool) -> Result<Self> {
        let handle = File::create(destination).with_context(|| {
            format!(
                "failed creating zip archive target at {}",
                destination.display()
            )
        })?;
        Ok(Self::new(handle))
    }

    pub fn write_file_entry(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        let normalized = normalize_zip_entry_path(dest)?;
        let content_bytes = std::fs::read(source)
            .with_context(|| format!("failed reading entry source file {}", source.display()))?;
        self.write_entry(&normalized, &content_bytes, opts.into())
    }

    pub fn write_directory_entry(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        let normalized_prefix = normalize_zip_entry_path(dest)?;
        self.ingest_directory_recursive_with_opts(&normalized_prefix, source, opts)
    }

    pub fn ingest_directory_recursive_with_opts(
        &mut self,
        prefix: &str,
        source_dir: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        if !source_dir.is_dir() {
            return Ok(());
        }
        let mut collected = Vec::new();
        for item in walkdir::WalkDir::new(source_dir).follow_links(true) {
            let item = item?;
            let disk_path = item.path().to_path_buf();
            if disk_path.is_file() {
                let relative_name = disk_path
                    .strip_prefix(source_dir)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let qualified_name = if prefix.is_empty() {
                    relative_name
                } else {
                    format!("{prefix}/{relative_name}")
                };
                collected.push((qualified_name, disk_path));
            }
        }
        collected.sort_by(|x, y| x.0.cmp(&y.0));
        for (name, path) in collected {
            let payload = std::fs::read(&path).with_context(|| {
                format!(
                    "failed reading archive source payload at {}",
                    path.display()
                )
            })?;
            self.write_entry(&name, &payload, opts.into())?;
        }
        Ok(())
    }

    pub fn finalize_archive(self) -> Result<()> {
        self.finalize()?;
        Ok(())
    }
}

impl AlignedArchiveSink<Cursor<Vec<u8>>> {
    pub fn in_memory() -> Self {
        Self::new(Cursor::new(Vec::new()))
    }

    pub fn finalize_archive_bytes(mut self) -> Result<Vec<u8>> {
        let inner_cursor = self.archive_writer.finish()?;
        Ok(inner_cursor.into_inner())
    }
}

impl<W: Write + Seek> Write for AlignedArchiveSink<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.archive_writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.archive_writer.flush()
    }
}

impl<W: Write + Seek> AlignedArchiveSink<W> {
    pub fn new(writer: W) -> Self {
        Self {
            archive_writer: ZipWriter::new(writer),
            recorded_offsets: Vec::new(),
        }
    }

    pub fn recorded_offsets(&self) -> &[ArchiveEntryOffset] {
        &self.recorded_offsets
    }

    pub fn write_entry(
        &mut self,
        name: &str,
        data: &[u8],
        strategy: EntryCompressionStrategy,
    ) -> Result<()> {
        let align_val = strategy.boundary_alignment();
        let method_kind = strategy.resolved_compression_method();
        let entry_opts = FileOptions::default().compression_method(method_kind);

        if align_val > 1 {
            self.archive_writer
                .start_file_aligned(name, entry_opts, align_val as u16)
                .with_context(|| {
                    format!("failed creating aligned zip item '{name}' boundary={align_val}")
                })?;
        } else {
            self.archive_writer
                .start_file(name, entry_opts)
                .with_context(|| format!("failed creating zip entry '{name}'"))?;
        }

        self.archive_writer
            .write_all(data)
            .with_context(|| format!("failed writing payload for '{name}'"))?;

        Ok(())
    }

    pub fn ingest_directory_recursive(&mut self, prefix: &str, source_dir: &Path) -> Result<()> {
        if !source_dir.is_dir() {
            return Ok(());
        }
        for item in walkdir::WalkDir::new(source_dir).follow_links(true) {
            let item = item?;
            let current = item.path();
            if current.is_file() {
                let rel = current
                    .strip_prefix(source_dir)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let qualified = if prefix.is_empty() {
                    rel
                } else {
                    format!("{prefix}/{rel}")
                };
                let raw_data = std::fs::read(current).with_context(|| {
                    format!("failed reading file payload at {}", current.display())
                })?;
                self.write_entry(
                    &qualified,
                    &raw_data,
                    EntryCompressionStrategy::CompressedDeflate,
                )?;
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Result<W> {
        Ok(self.archive_writer.finish()?)
    }
}

/// Extracts all entries from a ZIP archive into `destination_dir`, stripping `strip_components`
/// leading path components from each entry while enforcing Zip Slip protection, Unix permission
/// restoration, and safe symlink replication.
pub fn unpack_archive_stream_with_strip(
    archive_path: &Path,
    destination_dir: &Path,
    strip_components: usize,
) -> Result<()> {
    let mut zip_reader = ZipArchive::new(File::open(archive_path)?)?;
    std::fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "Failed creating destination directory `{}`",
            destination_dir.display()
        )
    })?;
    let canonical_dest = dunce::canonicalize(destination_dir).with_context(|| {
        format!(
            "Failed to canonicalize destination directory `{}`",
            destination_dir.display()
        )
    })?;
    let mut pending_symlinks: Vec<(PathBuf, PathBuf)> = Vec::new();

    let entry_count = zip_reader.len();
    for entry_idx in 0..entry_count {
        let mut entry_file = zip_reader.by_index(entry_idx)?;
        let name_string = entry_file.name().to_string();
        let raw_enclosed = entry_file
            .enclosed_name()
            .context("Invalid zip entry path")?;

        let stripped: PathBuf = raw_enclosed.components().skip(strip_components).collect();
        if stripped.as_os_str().is_empty() {
            continue;
        }

        let output_destination = destination_dir.join(&stripped);

        // Security: Ensure parent directory exists and resides strictly within canonical destination
        if let Some(parent) = output_destination.parent() {
            std::fs::create_dir_all(parent)?;
            let canon_parent = dunce::canonicalize(parent).with_context(|| {
                format!(
                    "Failed to canonicalize parent directory `{}` during extraction",
                    parent.display()
                )
            })?;
            if !canon_parent.starts_with(&canonical_dest) {
                anyhow::bail!(
                    "Security violation: Zip Slip path traversal detected for '{name_string}' via parent directory `{}`",
                    canon_parent.display()
                );
            }
        }

        // Security: If destination path already exists on disk (e.g. symlink), ensure it does not escape
        if let Ok(canon_path) = dunce::canonicalize(&output_destination) {
            if !canon_path.starts_with(&canonical_dest) {
                anyhow::bail!(
                    "Security violation: Zip Slip path traversal detected for '{name_string}' escaping to `{}`",
                    canon_path.display()
                );
            }
        }

        if entry_file.is_dir() || entry_file.name().ends_with('/') {
            std::fs::create_dir_all(&output_destination)?;
            continue;
        }

        if let Some(unix_mode) = entry_file.unix_mode() {
            if (unix_mode & 0o120000) == 0o120000 {
                let mut link_target = Vec::new();
                entry_file.read_to_end(&mut link_target)?;
                let link_str = std::str::from_utf8(&link_target)?;
                pending_symlinks.push((PathBuf::from(link_str), output_destination));
                continue;
            }
        }

        let mut out_file = File::create(&output_destination)?;
        std::io::copy(&mut entry_file, &mut out_file)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(unix_mode) = entry_file.unix_mode() {
                if (unix_mode & 0o120000) != 0o120000 && output_destination.exists() {
                    let _ = std::fs::set_permissions(
                        &output_destination,
                        std::fs::Permissions::from_mode(unix_mode),
                    );
                }
            }
        }
    }

    for (target_path, link_path) in pending_symlinks {
        let resolved_src = if target_path.is_relative() {
            link_path
                .parent()
                .map(|p| p.join(&target_path))
                .unwrap_or_else(|| target_path.clone())
        } else {
            target_path.clone()
        };
        crate::symlink::copy_symlink_entry(&target_path, &resolved_src, &link_path)?;
    }

    Ok(())
}

/// Unpacks a ZIP archive without stripping leading components.
pub fn unpack_archive_stream(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    unpack_archive_stream_with_strip(archive_path, destination_dir, 0)
}

pub fn unpack_single_entry_secure(archive_path: &Path, entry_name: &str) -> Result<Vec<u8>> {
    let mut reader = ZipArchive::new(File::open(archive_path)?)?;
    let mut entry = reader.by_name(entry_name).with_context(|| {
        format!(
            "entry '{entry_name}' not found in archive {}",
            archive_path.display()
        )
    })?;
    let mut buffer = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buffer)?;
    Ok(buffer)
}

pub fn normalize_zip_entry_path(dest: &Path) -> Result<String> {
    use std::path::Component;

    let mut path_components = Vec::new();

    for comp in dest.components() {
        match comp {
            Component::Prefix(prefix) => {
                anyhow::bail!(
                    "Invalid ZIP entry path `{}`: filesystem prefix `{}` is not allowed in archive entries",
                    dest.display(),
                    prefix.as_os_str().to_string_lossy()
                );
            }
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => {
                anyhow::bail!(
                    "Security violation in ZIP entry path `{}`: parent directory traversal ('..') is strictly prohibited",
                    dest.display()
                );
            }
            Component::Normal(segment) => match segment.to_str() {
                Some(valid_str) => path_components.push(valid_str),
                None => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::ffi::OsStrExt;
                        let raw_bytes = segment.as_bytes();
                        let hex_repr: String =
                            raw_bytes.iter().map(|b| format!("\\x{:02x}", b)).collect();
                        anyhow::bail!(
                            "Invalid non-UTF-8 path segment in ZIP entry `{}`: segment contains raw bytes `{}`. \
                             All entry paths in APK, MSIX, and IPA packages must be valid UTF-8.",
                            dest.display(),
                            hex_repr
                        );
                    }
                    #[cfg(not(unix))]
                    {
                        anyhow::bail!(
                            "Invalid non-UTF-8 path segment in ZIP entry `{}`: `{}` contains invalid Unicode or unpaired surrogates. \
                             All entry paths in APK, MSIX, and IPA packages must be valid UTF-8.",
                            dest.display(),
                            segment.to_string_lossy()
                        );
                    }
                }
            },
        }
    }

    anyhow::ensure!(
        !path_components.is_empty(),
        "ZIP entry path cannot be empty: `{}`",
        dest.display()
    );

    Ok(path_components.join("/"))
}

pub struct StreamArchiveWriter<W: Write + Seek> {
    archive_writer: ZipWriter<W>,
}

impl<W: Write + Seek> StreamArchiveWriter<W> {
    pub fn new(sink: W) -> Self {
        Self {
            archive_writer: ZipWriter::new(sink),
        }
    }

    pub fn stream_entry(
        &mut self,
        entry_name: &str,
        payload: &mut impl Read,
        policy: ArchiveEntryOptions,
    ) -> Result<u64> {
        let (method, alignment) = match policy {
            ArchiveEntryOptions::Compressed => (CompressionMethod::Deflated, 1),
            ArchiveEntryOptions::Aligned(align) => (CompressionMethod::Stored, align),
            ArchiveEntryOptions::Unaligned => (CompressionMethod::Stored, 1),
        };

        let options = FileOptions::default().compression_method(method);

        if alignment > 1 {
            self.archive_writer
                .start_file_aligned(entry_name, options, alignment)?;
        } else {
            self.archive_writer.start_file(entry_name, options)?;
        }

        let bytes_copied = std::io::copy(payload, &mut self.archive_writer)
            .with_context(|| format!("Failed streaming payload for entry `{entry_name}`"))?;

        Ok(bytes_copied)
    }

    pub fn finalize_archive(mut self) -> Result<W> {
        Ok(self.archive_writer.finish()?)
    }
}

pub struct BoundaryZipArchive(pub AlignedArchiveSink<std::fs::File>);

impl BoundaryZipArchive {
    pub fn open_or_create(path: &Path, compress: bool) -> Result<Self> {
        let sink = AlignedArchiveSink::new_path(path, compress)?;
        Ok(Self(sink))
    }

    pub fn new(path: &Path, compress: bool) -> Result<Self> {
        Self::open_or_create(path, compress)
    }

    pub fn from_file(file: File, _compress: bool) -> Self {
        Self(AlignedArchiveSink::new(file))
    }

    pub fn append_file(file: File, _compress: bool) -> Result<Self> {
        Ok(Self(AlignedArchiveSink {
            archive_writer: ZipWriter::new_append(file)?,
            recorded_offsets: Vec::new(),
        }))
    }

    pub fn append(path: &Path, compress: bool) -> Result<Self> {
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        Self::append_file(handle, compress)
    }

    pub fn add_file(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        self.write_file_entry(source, dest, opts)
    }

    pub fn add_zip_file(&mut self, file: zip::read::ZipFile<'_>) -> Result<()> {
        self.0.archive_writer.raw_copy_file(file)?;
        Ok(())
    }

    pub fn start_file(&mut self, dest: &Path, opts: ArchiveEntryOptions) -> Result<()> {
        let normalized = normalize_zip_entry_path(dest)?;
        let align = opts.boundary_alignment();
        let method = match opts {
            ArchiveEntryOptions::Compressed => CompressionMethod::Deflated,
            _ => CompressionMethod::Stored,
        };
        let file_opts = FileOptions::default().compression_method(method);
        if align > 1 {
            self.0
                .archive_writer
                .start_file_aligned(&normalized, file_opts, align)?;
        } else {
            self.0.archive_writer.start_file(&normalized, file_opts)?;
        }
        Ok(())
    }

    pub fn create_file(
        &mut self,
        dest: &Path,
        opts: ArchiveEntryOptions,
        data: &[u8],
    ) -> Result<()> {
        let normalized = normalize_zip_entry_path(dest)?;
        self.0.write_entry(&normalized, data, opts.into())
    }

    pub fn add_directory(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        self.write_directory_entry(source, dest, opts)
    }

    pub fn write_file_entry(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        self.0.write_file_entry(source, dest, opts)
    }

    pub fn write_directory_entry(
        &mut self,
        source: &Path,
        dest: &Path,
        opts: ArchiveEntryOptions,
    ) -> Result<()> {
        self.0.write_directory_entry(source, dest, opts)
    }

    pub fn finish(self) -> Result<()> {
        self.finalize_archive()
    }

    pub fn finish_into_inner(self) -> Result<File> {
        self.0.finalize()
    }

    pub fn finalize_archive(self) -> Result<()> {
        self.0.finalize_archive()
    }
}

impl Write for BoundaryZipArchive {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}
