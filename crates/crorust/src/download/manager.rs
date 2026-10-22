use crate::download::catalog::{ArchiveKind, ExtractionPolicy, ToolArtifact, ToolCategory};
use crate::download::{extract_atomic, VerifiableDownload};
use crate::SessionExecutionContext;
use anyhow::Result;
use reqwest::blocking::Client;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use tar::Archive;
use zstd::Decoder;

/// Manages downloading, verification, caching, and atomic staging of external SDKs and tools.
///
/// `ToolManager` implements the multi-host caching architecture:
/// - Host-executable utilities (JDK, kotlinc, Gradle) are staged in `tools/<host-triple>/`.
/// - Platform sysroots (Windows SDK, macOS SDK, Android NDK headers) are staged in `sysroots/`.
/// - In-flight downloads are SHA-256 hashed on the fly and staged atomically with `fd-lock`.
pub struct ToolManager {
    cache_dir: PathBuf,
    mirror_url: Option<String>,
    is_offline: bool,
    skip_artifact_verification: bool,
    client: Client,
    cache_tools_dir: PathBuf,
    cache_sysroots_dir: PathBuf,
    cache_download_dir: PathBuf,
}

impl ToolManager {
    /// Creates a new `ToolManager` configured from the active execution session context.
    pub fn new(env: &SessionExecutionContext) -> Result<Self> {
        let host_triple = env.cargo_session().host_triple();
        let mirror_url = env.manifest_config().toolchain().mirror_url.clone();
        let is_offline =
            env.is_offline() || env.manifest_config().toolchain().is_offline_enforced();
        let skip_artifact_verification = env.skip_artifact_verification();
        let mut mgr = Self::with_cache_dir(
            env.cache_directory().to_path_buf(),
            host_triple,
            mirror_url,
            is_offline,
        )?;
        mgr.skip_artifact_verification = skip_artifact_verification;
        Ok(mgr)
    }

    /// Initializes a `ToolManager` with explicit cache path, host triple, and offline policies.
    pub fn with_cache_dir(
        cache_dir: PathBuf,
        host_triple: &str,
        mirror_url: Option<String>,
        is_offline: bool,
    ) -> Result<Self> {
        let cache_tools_dir = cache_dir.join("tools").join(host_triple);
        let cache_download_dir = cache_dir.join("download");
        let cache_sysroots_dir = cache_dir.join("sysroots");

        std::fs::create_dir_all(&cache_tools_dir)?;
        std::fs::create_dir_all(&cache_download_dir)?;
        std::fs::create_dir_all(&cache_sysroots_dir)?;

        Ok(Self {
            cache_dir,
            mirror_url,
            is_offline,
            skip_artifact_verification: false,
            client: crate::download::create_hardened_http_client()?,
            cache_tools_dir,
            cache_sysroots_dir,
            cache_download_dir,
        })
    }

    pub fn canonical_install_dir(&self, artifact: &ToolArtifact) -> PathBuf {
        match artifact.category {
            ToolCategory::Sysroot => self
                .cache_sysroots_dir
                .join(artifact.name)
                .join(artifact.version),
            ToolCategory::Ndk => self.cache_tools_dir.join("ndk").join(artifact.version),
            _ => self
                .cache_tools_dir
                .join(artifact.name)
                .join(artifact.version),
        }
    }

    pub fn find_installed(&self, artifact: &ToolArtifact) -> Option<PathBuf> {
        let canonical = self.canonical_install_dir(artifact);
        if canonical.join(".complete").exists() {
            return Some(canonical);
        }

        if artifact.category == ToolCategory::Sysroot {
            let unversioned = self.cache_sysroots_dir.join(artifact.name);
            if unversioned.join(".complete").exists() {
                return Some(unversioned);
            }
            if artifact.name.starts_with("Linux.sysroot-") {
                let generic_sysroot = self.cache_sysroots_dir.join("Linux.sysroot");
                if generic_sysroot.join(".complete").exists() {
                    return Some(generic_sysroot);
                }
            }
        }

        None
    }

    pub fn ensure_tool(&self, artifact: &ToolArtifact) -> Result<PathBuf> {
        if let Some(existing) = self.find_installed(artifact) {
            return Ok(existing);
        }

        let target_dir = self.canonical_install_dir(artifact);
        self.ensure_tool_to_dir(artifact, &target_dir)
    }

    pub fn ensure_tool_to_dir(
        &self,
        artifact: &ToolArtifact,
        target_dir: &Path,
    ) -> Result<PathBuf> {
        if target_dir.join(".complete").exists() {
            return Ok(target_dir.to_path_buf());
        }

        if self.is_offline {
            anyhow::bail!(
                "Offline build failed: Required tool `{}` version `{}` is not cached locally.\n\
                 Expected location: {}\n\
                 Offline mode is enforced via manifest.toml (`toolchain.offline = true`) or `--offline` flag.\n\
                 Please provision this tool on an internet-connected host or pre-seed the cache.",
                artifact.name,
                artifact.version,
                target_dir.display()
            );
        }

        let mirror_base = self.mirror_url.as_deref();
        let download_url = artifact.resolved_url(mirror_base);

        let lock_dir = self.cache_dir.join("locks");
        let archive_name = artifact.archive_filename();
        let archive_file = self.cache_download_dir.join(archive_name);

        if !archive_file.exists() {
            let expected_sha = if self.skip_artifact_verification || artifact.sha256.is_empty() {
                if self.skip_artifact_verification {
                    eprintln!(
                        "\x1b[33m⚠️  [crorust] Supply Chain Notice: Verification bypassed for `{}` (--skip-artifact-verification active). Supply chain guarantees are reduced.\x1b[0m",
                        artifact.name
                    );
                }
                None
            } else {
                Some(artifact.sha256)
            };
            let verifier = VerifiableDownload::new(&download_url, expected_sha);
            verifier.download(&self.client, &archive_file)?;
        }

        extract_atomic(
            &archive_file,
            target_dir,
            &lock_dir,
            artifact.name,
            |archive, staging| self.unpack_archive(archive, staging, artifact),
        )?;

        #[cfg(unix)]
        self.apply_unix_permissions(target_dir, artifact)?;

        self.verify_installed_tool(artifact, target_dir)?;

        Ok(target_dir.to_path_buf())
    }

    /// Explicitly verifies that a tool has been successfully extracted into the cache,
    /// contains the `.complete` sentinel marker, and that all expected binaries exist
    /// on disk and have executable permissions.
    pub fn verify_installed_tool(
        &self,
        artifact: &ToolArtifact,
        installed_dir: &Path,
    ) -> Result<()> {
        anyhow::ensure!(
            installed_dir.is_dir(),
            "Expected tool directory does not exist or is not a directory: {}",
            installed_dir.display()
        );

        let marker = installed_dir.join(".complete");
        anyhow::ensure!(
            marker.is_file(),
            "Missing `.complete` marker in cache directory: {}",
            installed_dir.display()
        );

        for rel_binary in artifact.binary_rel_paths {
            let binary_path = installed_dir.join(rel_binary);
            anyhow::ensure!(
                binary_path.exists(),
                "Required tool binary `{}` is missing from installed cache at: {}",
                rel_binary,
                binary_path.display()
            );

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(&binary_path) {
                    let permissions = metadata.permissions();
                    let mode = permissions.mode();
                    if !rel_binary.ends_with(".jar")
                        && !rel_binary.ends_with(".json")
                        && (mode & 0o111) == 0
                    {
                        let mut new_perms = permissions;
                        new_perms.set_mode(mode | 0o755);
                        let _ = std::fs::set_permissions(&binary_path, new_perms);
                    }
                }
            }
        }

        anyhow::ensure!(
            self.find_installed(artifact).is_some(),
            "Cache lookup `find_installed` failed for `{}` despite physical files existing.",
            artifact.name
        );

        Ok(())
    }

    pub fn ensure_tools_parallel(&self, artifacts: &[&ToolArtifact]) -> Result<Vec<PathBuf>> {
        if artifacts.is_empty() {
            return Ok(Vec::new());
        }

        use rayon::prelude::*;
        artifacts
            .par_iter()
            .map(|artifact| self.ensure_tool(artifact))
            .collect()
    }

    pub fn ensure_dynamic_zip(&self, name: &str, url: &str, target_dir: &Path) -> Result<()> {
        if target_dir.join(".complete").exists() {
            return Ok(());
        }

        if self.is_offline {
            anyhow::bail!(
                "Offline build failed: Required archive `{}` is not cached locally.\n\
                 Expected location: {}\n\
                 Offline mode is enforced via manifest.toml (`toolchain.offline = true`) or `--offline` flag.",
                name,
                target_dir.display()
            );
        }

        let lock_dir = self.cache_dir.join("locks");
        let archive_name = url.rsplit_once('/').map(|(_, f)| f).unwrap_or(name);
        let archive_file = self.cache_download_dir.join(archive_name);

        if !archive_file.exists() {
            let verifier = VerifiableDownload::new(url, None);
            verifier.download(&self.client, &archive_file)?;
        }

        extract_atomic(
            &archive_file,
            target_dir,
            &lock_dir,
            name,
            crorust_common::unpack_archive_stream,
        )?;

        Ok(())
    }

    pub fn unpack_archive(
        &self,
        archive_path: &Path,
        staging_dir: &Path,
        artifact: &ToolArtifact,
    ) -> Result<()> {
        match artifact.archive_kind {
            ArchiveKind::TarGz => {
                let file = BufReader::new(File::open(archive_path)?);
                let gz = flate2::read::GzDecoder::new(file);
                let mut archive = Archive::new(gz);
                self.unpack_tar_with_strip(
                    &mut archive,
                    staging_dir,
                    artifact.strip_components,
                    artifact.extraction_policy,
                )?;
            }
            ArchiveKind::TarZst => {
                let file = BufReader::new(File::open(archive_path)?);
                let zst = Decoder::new(file)?;
                let mut archive = Archive::new(zst);
                self.unpack_tar_with_strip(
                    &mut archive,
                    staging_dir,
                    artifact.strip_components,
                    artifact.extraction_policy,
                )?;
            }
            ArchiveKind::TarXz => {
                let file = BufReader::new(File::open(archive_path)?);
                let xz = xz2::read::XzDecoder::new(file);
                let mut archive = Archive::new(xz);
                self.unpack_tar_with_strip(
                    &mut archive,
                    staging_dir,
                    artifact.strip_components,
                    artifact.extraction_policy,
                )?;
            }
            ArchiveKind::Zip => {
                crorust_common::boundary_archive::unpack_archive_stream_with_strip(
                    archive_path,
                    staging_dir,
                    artifact.strip_components,
                )?;
            }
            ArchiveKind::RawBinary => {
                let target_file = staging_dir.join(artifact.binary_rel_paths[0]);
                if let Some(parent) = target_file.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(archive_path, target_file)?;
            }
        }
        Ok(())
    }

    fn unpack_tar_with_strip<R: std::io::Read>(
        &self,
        archive: &mut Archive<R>,
        staging_dir: &Path,
        strip: usize,
        policy: ExtractionPolicy,
    ) -> Result<()> {
        unpack_tar_archive_with_strip(archive, staging_dir, strip, policy)
    }

    #[cfg(unix)]
    fn apply_unix_permissions(&self, base_dir: &Path, artifact: &ToolArtifact) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        for rel_path in artifact.binary_rel_paths {
            let bin = base_dir.join(rel_path);
            if bin.exists() {
                let mut perms = std::fs::metadata(&bin)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&bin, perms)?;
            }
        }
        Ok(())
    }
}

pub fn unpack_tar_archive_with_strip<R: std::io::Read>(
    archive: &mut Archive<R>,
    staging_dir: &Path,
    strip: usize,
    policy: ExtractionPolicy,
) -> Result<()> {
    let skip_symlinks = policy.skip_symlinks_on_non_linux && !cfg!(target_os = "linux");
    let skip_colons = policy.skip_colons_on_windows && cfg!(target_os = "windows");
    let mut deferred_symlinks = Vec::new();
    let mut deferred_hardlinks = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_type = entry.header().entry_type();
        if skip_symlinks && entry_type == tar::EntryType::Symlink {
            continue;
        }
        let path = entry.path()?;
        if skip_colons {
            if let Some(s) = path.to_str() {
                if s.contains(':') {
                    continue;
                }
            }
        }
        let stripped_path: PathBuf = path.components().skip(strip).collect();
        if stripped_path.as_os_str().is_empty() {
            continue;
        }
        let out_path = staging_dir.join(stripped_path);
        if entry_type.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else if entry_type == tar::EntryType::Symlink {
            if let Some(target) = entry.link_name()? {
                deferred_symlinks.push((out_path, target.into_owned()));
            }
        } else if entry_type == tar::EntryType::Link {
            if let Some(target) = entry.link_name()? {
                deferred_hardlinks.push((out_path, target.into_owned()));
            }
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&out_path)?;
        }
    }

    for (hardlink_path, target_rel) in deferred_hardlinks {
        let stripped_target: PathBuf = target_rel.components().skip(strip).collect();
        let resolved_target = staging_dir.join(stripped_target);
        if let Some(parent) = hardlink_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if hardlink_path.exists() {
            let _ = std::fs::remove_file(&hardlink_path);
        }
        if resolved_target.exists() {
            if std::fs::hard_link(&resolved_target, &hardlink_path).is_err() {
                std::fs::copy(&resolved_target, &hardlink_path)?;
            }
        } else {
            tracing::warn!(
                "Hardlink target missing in archive: `{}` -> `{}` (skipped)",
                hardlink_path.display(),
                resolved_target.display()
            );
        }
    }

    for (symlink_path, target_rel) in deferred_symlinks {
        if let Some(parent) = symlink_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let native_success = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&target_rel, &symlink_path).is_ok()
            }
            #[cfg(windows)]
            {
                let normalized_rel = PathBuf::from(
                    target_rel
                        .to_string_lossy()
                        .replace('/', std::path::MAIN_SEPARATOR_STR),
                );
                let resolved_target = symlink_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(&normalized_rel);
                if resolved_target.is_dir() {
                    std::os::windows::fs::symlink_dir(&normalized_rel, &symlink_path).is_ok()
                } else {
                    std::os::windows::fs::symlink_file(&normalized_rel, &symlink_path).is_ok()
                }
            }
            #[cfg(not(any(unix, windows)))]
            {
                false
            }
        };

        if native_success {
            continue;
        }

        let normalized_rel = if cfg!(windows) {
            PathBuf::from(
                target_rel
                    .to_string_lossy()
                    .replace('/', std::path::MAIN_SEPARATOR_STR),
            )
        } else {
            target_rel.clone()
        };

        let base_parent = symlink_path.parent().unwrap_or(Path::new("."));
        let direct_target = base_parent.join(&normalized_rel);
        let resolved_target = if direct_target.exists() {
            Some(direct_target)
        } else if let Ok(canonical) = dunce::canonicalize(&direct_target) {
            Some(canonical)
        } else {
            crorust_common::case_fold_lookup(base_parent, &normalized_rel)
        };

        if let Some(target) = resolved_target {
            tracing::debug!(
                "Symlink unprivileged fallback: copying `{}` to `{}`",
                target.display(),
                symlink_path.display()
            );
            if target.is_dir() {
                crorust_common::copy_dir_all(&target, &symlink_path)?;
            } else {
                std::fs::copy(&target, &symlink_path)?;
            }
        } else {
            tracing::warn!(
                "Dangling symlink in archive: `{}` -> `{}` (skipped)",
                symlink_path.display(),
                target_rel.display()
            );
        }
    }

    Ok(())
}

pub fn extract_any_archive_with_strip(
    archive_path: &Path,
    staging_dir: &Path,
    strip: usize,
    policy: ExtractionPolicy,
) -> Result<()> {
    let mut header = [0u8; 6];
    if let Ok(mut f) = File::open(archive_path) {
        let _ = f.read_exact(&mut header);
    }
    let filename = archive_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if header.starts_with(b"PK\x03\x04") || filename.ends_with(".zip") {
        crorust_common::boundary_archive::unpack_archive_stream_with_strip(
            archive_path,
            staging_dir,
            strip,
        )
    } else if header.starts_with(b"\xfd7zXZ\x00")
        || filename.ends_with(".tar.xz")
        || filename.ends_with(".xz")
    {
        let file = BufReader::new(File::open(archive_path)?);
        let xz = xz2::read::XzDecoder::new(file);
        let mut archive = Archive::new(xz);
        unpack_tar_archive_with_strip(&mut archive, staging_dir, strip, policy)
    } else if header.starts_with(b"\x28\xb5\x2f\xfd")
        || filename.ends_with(".tar.zst")
        || filename.ends_with(".zst")
    {
        let file = BufReader::new(File::open(archive_path)?);
        let zst = Decoder::new(file)?;
        let mut archive = Archive::new(zst);
        unpack_tar_archive_with_strip(&mut archive, staging_dir, strip, policy)
    } else if header.starts_with(b"\x1f\x8b")
        || filename.ends_with(".tar.gz")
        || filename.ends_with(".tgz")
    {
        let file = BufReader::new(File::open(archive_path)?);
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = Archive::new(gz);
        unpack_tar_archive_with_strip(&mut archive, staging_dir, strip, policy)
    } else {
        // Fallback: try tar archive directly, or zip
        if let Ok(file) = File::open(archive_path) {
            let mut archive = Archive::new(BufReader::new(file));
            if unpack_tar_archive_with_strip(&mut archive, staging_dir, strip, policy).is_ok() {
                return Ok(());
            }
        }
        crorust_common::boundary_archive::unpack_archive_stream_with_strip(
            archive_path,
            staging_dir,
            strip,
        )
    }
}
