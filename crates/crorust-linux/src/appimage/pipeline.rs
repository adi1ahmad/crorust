use crate::appimage::{AppImageContainerBuilder, AppImageOptions};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct LinuxPackageSpec<'a> {
    pub application_name: &'a str,
    pub target_arch: &'a str,
    pub binary_path: &'a Path,
    pub icon_path: Option<&'a Path>,
    pub shared_libraries: &'a [&'a Path],
    pub desktop_file_content: Option<&'a str>,
    pub output_path: &'a Path,
    pub options: Option<AppImageOptions>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOutputDescriptor {
    pub path: PathBuf,
    pub sha256: String,
    pub file_size_bytes: u64,
}

pub fn assemble_linux_package(spec: &LinuxPackageSpec) -> Result<PackageOutputDescriptor> {
    let parent = spec.output_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create output parent directory at {}",
            parent.display()
        )
    })?;

    let staging_dir = parent.join(format!(
        ".tmp-appdir-{}-{}",
        std::process::id(),
        spec.application_name
    ));
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)?;

    let app = AppImageContainerBuilder::new(
        &staging_dir,
        spec.application_name.to_string(),
        spec.target_arch.to_string(),
    )
    .with_context(|| {
        format!(
            "failed to initialize AppImage compounder in {}",
            staging_dir.display()
        )
    })?;

    app.configure_apprun_entrypoint()?;

    if let Some(desktop_content) = spec.desktop_file_content {
        let desktop_name = format!("{}.desktop", spec.application_name);
        std::fs::write(app.bundle_appdir().join(desktop_name), desktop_content)?;
    } else {
        app.generate_desktop_file()?;
    }

    if let Some(icon) = spec.icon_path {
        app.install_icon(icon)?;
    }

    app.install_file(spec.binary_path, Path::new(spec.application_name))?;

    for lib in spec.shared_libraries {
        let filename = lib.file_name().context("invalid library path")?;
        let dest = Path::new("usr").join("lib").join(filename);
        app.install_file(lib, &dest)?;
    }

    let options = spec.options.unwrap_or_default();
    app.build_with_options(spec.output_path, None, &options)?;

    let _ = std::fs::remove_dir_all(&staging_dir);

    let (sha256_hex, file_size_bytes) = crorust_common::hash_file_streaming(spec.output_path)
        .with_context(|| {
            format!(
                "Failed computing streaming hash of AppImage at {}",
                spec.output_path.display()
            )
        })?;

    Ok(PackageOutputDescriptor {
        path: spec.output_path.to_path_buf(),
        sha256: sha256_hex,
        file_size_bytes,
    })
}
