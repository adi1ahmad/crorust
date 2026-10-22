use crate::bundle::AppleBundleContainer;
use crate::plist::DarwinPropertyList;
use anyhow::{Context, Result};
use crorust_common::identity_keypair::IdentityKeypair;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DarwinPackageSpec<'a> {
    pub bundle_name: &'a str,
    pub executable_binary: &'a Path,
    pub metadata: DarwinPropertyList,
    pub icon_path: Option<&'a Path>,
    pub frameworks: &'a [&'a Path],
    pub provisioning_profile: Option<&'a Path>,
    pub output_dir: &'a Path,
    pub signing_identity: Option<&'a IdentityKeypair>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOutputDescriptor {
    pub path: PathBuf,
    pub sha256: String,
    pub file_size_bytes: u64,
}

pub fn assemble_darwin_package(spec: &DarwinPackageSpec) -> Result<PackageOutputDescriptor> {
    let mut bundle = AppleBundleContainer::new(spec.output_dir, spec.metadata.clone())
        .with_context(|| {
            format!(
                "failed to initialize Darwin app bundle at {}",
                spec.output_dir.display()
            )
        })?;

    bundle
        .stage_executable_binary(spec.executable_binary)
        .with_context(|| {
            format!(
                "failed to add executable `{}` to bundle",
                spec.executable_binary.display()
            )
        })?;

    if let Some(icon) = spec.icon_path {
        bundle
            .attach_application_icon(icon)
            .with_context(|| format!("failed to add icon `{}` to bundle", icon.display()))?;
    }

    for framework in spec.frameworks {
        bundle.stage_library(framework).with_context(|| {
            format!(
                "failed to add framework `{}` to bundle",
                framework.display()
            )
        })?;
    }

    bundle
        .finalize_bundle(spec.signing_identity.cloned())
        .context("failed to codesign and finalize Darwin app bundle")?;

    let appdir = bundle.bundle_appdir().to_path_buf();

    let mut hasher = Sha256::new();
    let mut total_size = 0u64;
    let mut buffer = [0u8; 131072];

    for entry in walkdir::WalkDir::new(&appdir).into_iter().flatten() {
        if entry.path().is_file() {
            if let Ok(mut f) = std::fs::File::open(entry.path()) {
                if let Ok(meta) = f.metadata() {
                    total_size += meta.len();
                }
                while let Ok(n) = f.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
            }
        }
    }
    let digest_hex = hex::encode(hasher.finalize());

    Ok(PackageOutputDescriptor {
        path: appdir,
        sha256: digest_hex,
        file_size_bytes: total_size,
    })
}
