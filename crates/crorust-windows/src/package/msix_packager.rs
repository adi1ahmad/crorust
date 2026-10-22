use super::manifest::WindowsPackageManifestDocument;
use super::msix_container::WindowsPackageContainer;
use anyhow::{Context, Result};
use crorust_common::identity_keypair::IdentityKeypair;
use crorust_common::ArchiveEntryOptions;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsPayloadEntry {
    pub relative_path: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct WindowsPackageSpec<'a> {
    pub manifest: &'a WindowsPackageManifestDocument,
    pub payload_files: &'a [WindowsPayloadEntry],
    pub resource_index: Option<&'a [u8]>,
    pub signing_identity: &'a IdentityKeypair,
    pub output_path: &'a Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOutputDescriptor {
    pub path: PathBuf,
    pub sha256: String,
    pub file_size_bytes: u64,
}

pub fn assemble_windows_package(spec: &WindowsPackageSpec) -> Result<PackageOutputDescriptor> {
    if let Some(parent) = spec.output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create output parent directory {}",
                parent.display()
            )
        })?;
    }

    let mut msix =
        WindowsPackageContainer::new(spec.output_path.to_path_buf(), spec.manifest.clone(), true)
            .with_context(|| {
            format!(
                "failed to initialize MSIX container at {}",
                spec.output_path.display()
            )
        })?;

    for entry in spec.payload_files {
        let dest = Path::new(&entry.relative_path);
        msix.add_data(dest, &entry.data, ArchiveEntryOptions::Compressed)
            .with_context(|| format!("failed to add entry '{}' to MSIX", entry.relative_path))?;
    }

    if let Some(pri_bytes) = spec.resource_index {
        msix.add_pri(pri_bytes)
            .context("failed to write resources.pri to MSIX")?;
    }

    msix.finalize_package(Some(spec.signing_identity.clone()))
        .with_context(|| {
            format!(
                "failed to seal and sign MSIX at {}",
                spec.output_path.display()
            )
        })?;

    let (sha256_hex, file_size_bytes) = crorust_common::hash_file_streaming(spec.output_path)
        .with_context(|| {
            format!(
                "failed to verify written MSIX at {}",
                spec.output_path.display()
            )
        })?;

    Ok(PackageOutputDescriptor {
        path: spec.output_path.to_path_buf(),
        sha256: sha256_hex,
        file_size_bytes,
    })
}
