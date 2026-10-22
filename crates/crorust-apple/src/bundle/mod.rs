pub mod director;
pub mod pipeline;

use anyhow::{Context, Result};
use crorust_common::IdentityKeypair;
use plist::Value;
use std::path::{Path, PathBuf};

pub use crate::plist::DarwinPropertyList;
pub use director::{AppBundleDirector, DarwinPlatform, IOS_ICON_SIZES, MACOS_ICON_SIZES};
pub use pipeline::{assemble_darwin_package, DarwinPackageSpec, PackageOutputDescriptor};

pub struct AppleBundleContainer {
    director: AppBundleDirector,
}

impl AppleBundleContainer {
    pub fn new(build_dir: &Path, info: DarwinPropertyList) -> Result<Self> {
        let director = AppBundleDirector::new(build_dir, info)?;
        Ok(Self { director })
    }

    pub fn open_or_new(build_dir: &Path, info: DarwinPropertyList) -> Result<Self> {
        let director = AppBundleDirector::open_or_new(build_dir, info)?;
        Ok(Self { director })
    }

    pub fn set_icon_cache_dir(&mut self, dir: impl Into<PathBuf>) {
        self.director.set_icon_cache_dir(dir);
    }

    pub fn register_existing_icon(&mut self) {
        self.director.register_existing_icons();
    }

    pub fn bundle_appdir(&self) -> &Path {
        self.director.bundle_appdir()
    }

    pub fn info(&self) -> &DarwinPropertyList {
        self.director.property_list()
    }

    pub fn info_mut(&mut self) -> &mut DarwinPropertyList {
        self.director.property_list_mut()
    }

    pub fn attach_application_icon(&mut self, path: &Path) -> Result<()> {
        self.director.rasterize_and_attach_icon(path)
    }

    pub fn install_resource_file(&self, path: &Path, dest: &Path) -> Result<()> {
        self.director.add_resource_file(path, dest)
    }

    pub fn install_resource_directory(&self, source: &Path, dest: &Path) -> Result<()> {
        self.director.add_resource_dir(source, dest)
    }

    pub fn stage_executable_binary(&mut self, path: &Path) -> Result<()> {
        self.director.stage_bundle_executable(path)
    }

    pub fn stage_framework(&self, path: &Path) -> Result<()> {
        self.director.add_framework_or_dylib(path)
    }

    pub fn stage_library(&self, path: &Path) -> Result<()> {
        self.director.add_framework_or_dylib(path)
    }

    pub fn install_provisioning_profile(&mut self, raw_profile: &[u8]) -> Result<()> {
        self.director
            .write_embedded_provisioning_profile(raw_profile)
    }

    pub fn set_entitlements(&mut self, entitlements: Value) {
        self.director.set_entitlements(entitlements);
    }

    pub fn finalize_bundle(&self, signer: Option<IdentityKeypair>) -> Result<()> {
        self.director.seal_and_sign(signer)
    }

    pub fn sign_dmg_archive(&self, path: &Path, signer: &IdentityKeypair) -> Result<()> {
        let bundle_id = self
            .director
            .property_list()
            .bundle_identifier
            .as_deref()
            .context("CFBundleIdentifier must be set before signing DMG")?;
        crate::dmg::sign_disk_image(path, signer, bundle_id)
    }
}
