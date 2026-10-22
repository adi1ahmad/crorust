use anyhow::{bail, ensure, Context, Result};
use crorust_common::IdentityKeypair;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub static RUNTIME_X86_64: &[u8] = include_bytes!("../../assets/runtime-x86_64");
pub static RUNTIME_AARCH64: &[u8] = include_bytes!("../../assets/runtime-aarch64");

pub mod pipeline;
pub use pipeline::{assemble_linux_package, LinuxPackageSpec, PackageOutputDescriptor};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SquashfsBackend {
    #[default]
    Auto,

    System,

    #[serde(rename = "pure-rust")]
    PureRust,
}

impl std::str::FromStr for SquashfsBackend {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "auto" => Ok(Self::Auto),
            "system" => Ok(Self::System),
            "pure-rust" => Ok(Self::PureRust),
            other => bail!(
                "Invalid squashfs backend `{other}`, expected `auto`, `system`, or `pure-rust`"
            ),
        }
    }
}

impl std::fmt::Display for SquashfsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::System => write!(f, "system"),
            Self::PureRust => write!(f, "pure-rust"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SquashfsCompression {
    #[default]
    Gzip,

    Xz,
}

impl std::str::FromStr for SquashfsCompression {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "gzip" => Ok(Self::Gzip),
            "xz" => Ok(Self::Xz),
            other => bail!("Invalid squashfs compression `{other}`, expected `gzip` or `xz`"),
        }
    }
}

impl std::fmt::Display for SquashfsCompression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gzip => write!(f, "gzip"),
            Self::Xz => write!(f, "xz"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AppImageOptions {
    pub backend: SquashfsBackend,
    pub compression: SquashfsCompression,
}

#[derive(Clone, Debug)]
pub struct DesktopEntrySpec {
    pub application_name: String,
    pub binary_entrypoint: String,
    pub icon_name: String,
    pub spec_version: String,
    pub categories: String,
    pub terminal: bool,
}

impl DesktopEntrySpec {
    pub fn for_app(name: &str) -> Self {
        Self {
            application_name: name.to_string(),
            binary_entrypoint: format!("{name} %u"),
            icon_name: name.to_string(),
            spec_version: "1.0".to_string(),
            categories: "Utility;".to_string(),
            terminal: false,
        }
    }

    pub fn serialize_to_string(&self) -> String {
        format!(
            "[Desktop Entry]\n\
             Version={ver}\n\
             Type=Application\n\
             Terminal={term}\n\
             Name={name}\n\
             Exec={exec}\n\
             Icon={icon}\n\
             Categories={cat}\n",
            ver = self.spec_version,
            term = self.terminal,
            name = self.application_name,
            exec = self.binary_entrypoint,
            icon = self.icon_name,
            cat = self.categories,
        )
    }
}

pub struct AppImageContainerBuilder {
    bundle_directory: PathBuf,
    app_identifier: String,
    architecture_target: String,
}

impl AppImageContainerBuilder {
    pub fn new(build_root: &Path, name: String, arch: String) -> Result<Self> {
        let appdir = build_root.join(format!("{name}.AppDir"));
        let _ = std::fs::remove_dir_all(&appdir);
        std::fs::create_dir_all(&appdir)
            .with_context(|| format!("Failed creating AppDir directory at {}", appdir.display()))?;
        Ok(Self {
            bundle_directory: appdir,
            app_identifier: name,
            architecture_target: arch,
        })
    }

    pub fn open_or_new(build_root: &Path, name: String, arch: String) -> Result<Self> {
        let appdir = build_root.join(format!("{name}.AppDir"));
        std::fs::create_dir_all(&appdir).with_context(|| {
            format!("Failed creating or opening AppDir at {}", appdir.display())
        })?;
        Ok(Self {
            bundle_directory: appdir,
            app_identifier: name,
            architecture_target: arch,
        })
    }

    #[inline]
    pub fn bundle_appdir(&self) -> &Path {
        &self.bundle_directory
    }

    #[inline]
    pub fn bundle_name(&self) -> &str {
        &self.app_identifier
    }

    #[inline]
    pub fn bundle_arch(&self) -> &str {
        &self.architecture_target
    }

    pub fn configure_apprun_entrypoint(&self) -> Result<()> {
        let link_target = self.bundle_directory.join("AppRun");
        let primary_binary = self.bundle_directory.join(&self.app_identifier);

        if primary_binary.exists() {
            crorust_common::copy_symlink_entry(
                Path::new(&self.app_identifier),
                &primary_binary,
                &link_target,
            )?;
        } else {
            #[cfg(unix)]
            {
                if link_target.exists() || link_target.is_symlink() {
                    let _ = std::fs::remove_file(&link_target);
                }
                std::os::unix::fs::symlink(Path::new(&self.app_identifier), &link_target)?;
            }
            #[cfg(windows)]
            {
                let _ = std::os::windows::fs::symlink_file(
                    Path::new(&self.app_identifier),
                    &link_target,
                );
            }
        }
        Ok(())
    }

    pub fn generate_desktop_file(&self) -> Result<()> {
        let entry = DesktopEntrySpec::for_app(&self.app_identifier);
        let path = self
            .bundle_directory
            .join(format!("{}.desktop", self.app_identifier));
        std::fs::write(&path, entry.serialize_to_string())
            .with_context(|| format!("Failed writing desktop entry to {}", path.display()))?;
        Ok(())
    }

    pub fn install_icon(&self, source_icon: &Path) -> Result<()> {
        let ext = source_icon
            .extension()
            .and_then(|e| e.to_str())
            .context("Missing or invalid icon file extension")?;
        let icon_filename = format!("{}.{}", self.app_identifier, ext);
        self.install_file(source_icon, Path::new(&icon_filename))?;

        let diricon_path = self.bundle_directory.join(".DirIcon");
        let staged_icon = self.bundle_directory.join(&icon_filename);
        crorust_common::copy_symlink_entry(Path::new(&icon_filename), &staged_icon, &diricon_path)?;
        Ok(())
    }

    pub fn install_file(&self, source: &Path, relative_dest: &Path) -> Result<()> {
        let destination = self.bundle_directory.join(relative_dest);
        if let Some(parent_dir) = destination.parent() {
            std::fs::create_dir_all(parent_dir)?;
        }
        std::fs::copy(source, &destination).with_context(|| {
            format!(
                "Failed copying {} -> {}",
                source.display(),
                destination.display()
            )
        })?;

        #[cfg(unix)]
        {
            let is_primary_or_apprun = relative_dest == Path::new(&self.app_identifier)
                || relative_dest == Path::new("AppRun");
            if is_primary_or_apprun {
                std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))?;
            }
        }

        if relative_dest == Path::new(&self.app_identifier) {
            let apprun_link = self.bundle_directory.join("AppRun");
            if !apprun_link.exists() && !apprun_link.is_symlink() {
                crorust_common::copy_symlink_entry(
                    Path::new(&self.app_identifier),
                    &destination,
                    &apprun_link,
                )?;
            }
        }
        Ok(())
    }

    pub fn install_directory(&self, source_dir: &Path, relative_dest: &Path) -> Result<()> {
        let destination = self.bundle_directory.join(relative_dest);
        std::fs::create_dir_all(&destination)?;
        crorust_common::copy_dir_all(source_dir, &destination)?;
        Ok(())
    }

    pub fn runtime_stub(&self) -> Result<&'static [u8]> {
        match self.architecture_target.as_str() {
            "x86_64" | "x64" => Ok(RUNTIME_X86_64),
            "aarch64" | "arm64" => Ok(RUNTIME_AARCH64),
            unsupported => {
                bail!("Unsupported architecture for Linux AppImage runtime stub: {unsupported}")
            }
        }
    }

    pub fn build_with_options(
        self,
        output_appimage: &Path,
        signing_identity: Option<IdentityKeypair>,
        options: &AppImageOptions,
    ) -> Result<()> {
        if signing_identity.is_some() {
            tracing::warn!("AppImage digital signatures not yet implemented; proceeding unsigned");
        }

        let execute_system_tool = match options.backend {
            SquashfsBackend::System => {
                ensure!(
                    which::which("mksquashfs").is_ok(),
                    "System mksquashfs requested, but 'mksquashfs' executable was not found on PATH."
                );
                true
            }
            SquashfsBackend::PureRust => false,
            SquashfsBackend::Auto => which::which("mksquashfs").is_ok(),
        };

        if execute_system_tool {
            self.assemble_with_system_mksquashfs(output_appimage, options.compression)
        } else {
            self.assemble_with_purerust_backhand(output_appimage, options.compression)
        }
    }

    pub fn build(
        self,
        output_appimage: &Path,
        signing_identity: Option<IdentityKeypair>,
    ) -> Result<()> {
        self.build_with_options(
            output_appimage,
            signing_identity,
            &AppImageOptions::default(),
        )
    }

    fn assemble_with_system_mksquashfs(
        &self,
        output_appimage: &Path,
        compression: SquashfsCompression,
    ) -> Result<()> {
        let staging_parent = self
            .bundle_directory
            .parent()
            .context("AppDir has no valid parent folder")?;

        let intermediate_squashfs = staging_parent.join(format!(
            "{}.{}.squashfs",
            self.app_identifier,
            std::process::id()
        ));
        let mut cleanup_guard = crorust_common::TempPathGuard::new(intermediate_squashfs);

        if std::env::var("CRORUST_PRESERVE_SQUASHFS").is_ok() {
            cleanup_guard.disarm();
        }

        let comp_flag = match compression {
            SquashfsCompression::Gzip => "gzip",
            SquashfsCompression::Xz => "xz",
        };

        let execution = Command::new("mksquashfs")
            .arg(&self.bundle_directory)
            .arg(cleanup_guard.path())
            .arg("-comp")
            .arg(comp_flag)
            .arg("-root-owned")
            .arg("-noappend")
            .arg("-quiet")
            .status()
            .context("Failed executing host mksquashfs command")?;

        ensure!(
            execution.success(),
            "mksquashfs execution returned non-zero status: {:?}",
            execution
        );

        self.concatenate_runtime_and_payload(cleanup_guard.path(), output_appimage)?;
        let _ = cleanup_guard.remove_now();
        Ok(())
    }

    fn assemble_with_purerust_backhand(
        &self,
        output_appimage: &Path,
        compression: SquashfsCompression,
    ) -> Result<()> {
        use backhand::compression::Compressor;
        use backhand::{FilesystemCompressor, FilesystemWriter, NodeHeader};
        use walkdir::WalkDir;

        let mut squash_writer = FilesystemWriter::default();

        match compression {
            SquashfsCompression::Gzip => {
                let comp = FilesystemCompressor::new(Compressor::Gzip, None)
                    .context("Failed instantiating pure-Rust Gzip compressor")?;
                squash_writer.set_compressor(comp);
            }
            SquashfsCompression::Xz => {
                bail!(
                    "Pure-Rust SquashFS packaging currently supports Gzip compression across all platforms. \
                     To use XZ compression, install the host system `mksquashfs` utility or configure \
                     `linux.compression = \"gzip\"` in manifest.toml."
                );
            }
        }

        for entry_res in WalkDir::new(&self.bundle_directory)
            .min_depth(1)
            .sort_by_file_name()
        {
            let disk_node = entry_res.with_context(|| {
                format!(
                    "Failed traversing AppDir filesystem at {}",
                    self.bundle_directory.display()
                )
            })?;

            let internal_relpath = disk_node.path().strip_prefix(&self.bundle_directory)?;
            let kind = disk_node.file_type();

            let header = NodeHeader {
                permissions: if kind.is_dir()
                    || evaluate_entry_executability(&disk_node, &self.app_identifier)
                {
                    0o755
                } else {
                    0o644
                },
                uid: 0,
                gid: 0,
                mtime: 0,
            };

            if kind.is_dir() {
                squash_writer.push_dir(internal_relpath, header)?;
            } else if kind.is_symlink() {
                let symlink_dest = std::fs::read_link(disk_node.path())?;
                squash_writer.push_symlink(symlink_dest, internal_relpath, header)?;
            } else if kind.is_file() {
                let stream_handle = File::open(disk_node.path())?;
                squash_writer.push_file(stream_handle, internal_relpath, header)?;
            }
        }

        if let Some(parent) = output_appimage.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut destination_file = File::create(output_appimage).with_context(|| {
            format!("Failed creating AppImage at {}", output_appimage.display())
        })?;

        #[cfg(unix)]
        destination_file.set_permissions(std::fs::Permissions::from_mode(0o755))?;

        let runtime_bytes = self.runtime_stub()?;
        let runtime_size = runtime_bytes.len() as u64;

        let mut buffered_sink = BufWriter::new(&mut destination_file);
        buffered_sink.write_all(runtime_bytes)?;
        buffered_sink.flush()?;

        squash_writer
            .write_with_offset(&mut buffered_sink, runtime_size)
            .context("Failed writing pure-Rust SquashFS image bytes to stream")?;

        buffered_sink.flush()?;
        Ok(())
    }

    fn concatenate_runtime_and_payload(
        &self,
        squashfs_payload: &Path,
        final_dest: &Path,
    ) -> Result<()> {
        if let Some(parent) = final_dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let source_handle = File::open(squashfs_payload)?;
        let mut payload_reader = BufReader::new(source_handle);

        let mut target_file = File::create(final_dest)?;
        #[cfg(unix)]
        target_file.set_permissions(std::fs::Permissions::from_mode(0o755))?;

        let mut target_writer = BufWriter::new(&mut target_file);
        let stub = self.runtime_stub()?;
        target_writer.write_all(stub)?;
        std::io::copy(&mut payload_reader, &mut target_writer)?;
        target_writer.flush()?;
        Ok(())
    }
}

fn evaluate_entry_executability(entry: &walkdir::DirEntry, main_app_name: &str) -> bool {
    if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
        if name == "AppRun" || name == main_app_name {
            return true;
        }
    }
    #[cfg(unix)]
    {
        if let Ok(m) = entry.metadata() {
            return (m.permissions().mode() & 0o111) != 0;
        }
    }
    if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
        if !name.contains('.') {
            return true;
        }
    }
    false
}
