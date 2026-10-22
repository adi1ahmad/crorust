#[cfg(feature = "platform-android")]
pub mod android;
#[cfg(feature = "platform-apple")]
pub mod apple;
#[cfg(feature = "platform-linux")]
pub mod linux;
#[cfg(feature = "platform-web")]
pub mod web;
#[cfg(feature = "platform-windows")]
pub mod windows;

use crate::core::env::SessionExecutionContext;
use crate::core::target::{TargetOpt, TargetPlatform};
use crate::packaging_cache::{IncrementalDecision, PackagingStamp};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PackagingContext {
    pub project_name: String,
    pub root_dir: PathBuf,
    pub platform_dir: PathBuf,
    pub cargo_dir: PathBuf,
    pub has_lib: bool,
    pub has_cdylib: bool,
    pub compiled_binaries: Vec<PathBuf>,
    pub compiled_cdylibs: Vec<PathBuf>,
    pub icon_path: Option<PathBuf>,
    pub is_release: bool,
    pub verbose: bool,
    pub offline: bool,
    pub config_bytes: Vec<u8>,
}

impl PackagingContext {
    pub fn from_env(
        env: &SessionExecutionContext,
        compiled_binaries: Vec<PathBuf>,
        compiled_cdylibs: Vec<PathBuf>,
    ) -> Self {
        let has_lib = env.project_root_dir().join("src").join("lib.rs").exists();
        let has_cdylib = env.cargo_session().package_manifest_data().has_cdylib();

        let config_bytes = {
            let manifest_path = env.project_root_dir().join("manifest.toml");
            if manifest_path.is_file() {
                std::fs::read(&manifest_path).unwrap_or_default()
            } else {
                let cargo_path = env.project_root_dir().join("Cargo.toml");
                std::fs::read(&cargo_path).unwrap_or_default()
            }
        };

        Self {
            project_name: env.app_name().to_string(),
            root_dir: env.project_root_dir().to_path_buf(),
            platform_dir: env.target_platform_dir(),
            cargo_dir: env.build_directory().join("cargo"),
            has_lib,
            has_cdylib,
            compiled_binaries,
            compiled_cdylibs,
            icon_path: env.app_icon_path().map(Path::to_path_buf),
            is_release: env.build_target().optimization_profile() == TargetOpt::Release,
            verbose: env.is_verbose(),
            offline: env.is_offline(),
            config_bytes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackagingOutput {
    pub primary_artifact: PathBuf,

    pub auxiliary_artifacts: Vec<PathBuf>,

    pub verified: bool,
}

impl PackagingOutput {
    pub fn single(primary_artifact: PathBuf) -> Self {
        Self {
            primary_artifact,
            auxiliary_artifacts: Vec::new(),
            verified: false,
        }
    }

    pub fn with_auxiliary(mut self, aux: PathBuf) -> Self {
        self.auxiliary_artifacts.push(aux);
        self
    }

    pub fn set_verified(mut self, verified: bool) -> Self {
        self.verified = verified;
        self
    }
}

pub trait PlatformPackager: Send + Sync {
    fn name(&self) -> &'static str;

    fn target_platform(&self) -> TargetPlatform;

    fn check_incremental(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
    ) -> Result<(IncrementalDecision, PackagingStamp)>;

    fn package(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
        decision: IncrementalDecision,
        stamp: PackagingStamp,
    ) -> Result<PackagingOutput>;

    fn verify(&self, artifact: &Path) -> Result<()>;
}

pub struct PackagerRegistry {
    packagers: HashMap<TargetPlatform, Box<dyn PlatformPackager>>,
}

impl Default for PackagerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PackagerRegistry {
    pub fn new() -> Self {
        Self {
            packagers: HashMap::new(),
        }
    }

    pub fn register<P: PlatformPackager + 'static>(&mut self, packager: P) {
        self.packagers
            .insert(packager.target_platform(), Box::new(packager));
    }

    pub fn get(&self, platform: TargetPlatform) -> Result<&dyn PlatformPackager> {
        self.packagers
            .get(&platform)
            .map(|b| b.as_ref())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No packager registered for platform '{platform}'. \
                     Ensure required platform feature is enabled in crorust."
                )
            })
    }

    pub fn standard() -> Self {
        #[allow(unused_mut)]
        let mut registry = Self::new();
        #[cfg(feature = "platform-linux")]
        registry.register(linux::LinuxPackager::new());
        #[cfg(feature = "platform-android")]
        registry.register(android::AndroidPackager::new());
        #[cfg(feature = "platform-apple")]
        {
            registry.register(apple::MacosPackager::new());
            registry.register(apple::IosPackager::new());
        }
        #[cfg(feature = "platform-windows")]
        registry.register(windows::WindowsPackager::new());
        #[cfg(feature = "platform-web")]
        registry.register(web::WebPackager::new());
        registry
    }
}
