use super::target::{
    AppStore, AppleTargetEnv, LinuxLibcVariant, PackageFormat, SessionBuildTarget, TargetArch,
    TargetOpt, TargetPlatform,
};
use crate::cargo::CargoWorkspaceSession;
use crate::config::ManifestConfig;
use crate::devices::TargetDevice;
use anyhow::Result;
use clap::Parser;
use crorust_common::IdentityKeypair;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug, Clone)]
pub struct ProjectBuildArguments {
    #[clap(flatten)]
    pub cargo: CargoPipelineArguments,
    #[clap(flatten)]
    pub build_target: PlatformTargetArguments,

    #[clap(long, short)]
    pub verbose: bool,
}

impl ProjectBuildArguments {
    pub fn build_target_args(&self) -> &PlatformTargetArguments {
        &self.build_target
    }

    pub fn cargo_args(&self) -> &CargoPipelineArguments {
        &self.cargo
    }
}

#[derive(Parser, Debug, Clone, Default)]
pub struct CargoPipelineArguments {
    /// Target specific package in a workspace
    #[clap(long, short = 'P', help_heading = "Cargo Compilation")]
    pub package: Option<String>,

    /// Target specific binary executable to compile
    #[clap(long, help_heading = "Cargo Compilation")]
    pub bin: Option<String>,

    /// Path to project directory or Cargo.toml
    #[clap(
        long,
        conflicts_with = "manifest_path",
        help_heading = "Cargo Compilation"
    )]
    pub project: Option<PathBuf>,

    /// Explicit path to Cargo.toml
    #[clap(long, conflicts_with = "project", help_heading = "Cargo Compilation")]
    pub manifest_path: Option<PathBuf>,

    /// Directory for all generated Cargo artifacts
    #[clap(long, help_heading = "Cargo Compilation")]
    pub target_dir: Option<PathBuf>,

    /// Run without accessing the network
    #[clap(long, help_heading = "Cargo Compilation")]
    pub offline: bool,

    /// Space or comma separated list of Cargo features to activate
    #[clap(long, short = 'F', help_heading = "Cargo Compilation")]
    pub features: Vec<String>,

    /// Do not activate the `default` feature of the package
    #[clap(long, help_heading = "Cargo Compilation")]
    pub no_default_features: bool,

    /// Force using project-local cache (`<project>`/.cache/crorust/)
    #[clap(
        long,
        conflicts_with = "global_cache",
        help_heading = "Toolchain Cache"
    )]
    pub project_cache: bool,

    /// Force using machine-wide global cache (~/.cache/crorust/)
    #[clap(
        long,
        conflicts_with = "project_cache",
        help_heading = "Toolchain Cache"
    )]
    pub global_cache: bool,

    /// Explicitly override toolchain cache directory destination
    #[clap(long, value_name = "PATH", help_heading = "Toolchain Cache")]
    pub cache_dir: Option<PathBuf>,

    /// Skip cryptographic hash verification when downloading precompiled platform SDKs
    #[clap(long, help_heading = "Toolchain Cache")]
    pub skip_artifact_verification: bool,
}

impl CargoPipelineArguments {
    pub fn project_cache(&self) -> bool {
        self.project_cache
    }

    pub fn global_cache(&self) -> bool {
        self.global_cache
    }

    pub fn cache_dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }

    pub fn skip_artifact_verification(&self) -> bool {
        self.skip_artifact_verification
    }

    pub fn set_project(&mut self, path: PathBuf) {
        self.project = Some(path);
    }

    pub fn cargo(self) -> Result<CargoWorkspaceSession> {
        let effective_manifest_path = if let Some(proj) = self.project {
            if !proj.exists() {
                anyhow::bail!(
                    "specified --project path does not exist: {}",
                    proj.display()
                );
            }
            if proj.is_dir() {
                let candidate = proj.join("Cargo.toml");
                if !candidate.is_file() {
                    anyhow::bail!(
                        "specified project directory does not contain a Cargo.toml: {}",
                        proj.display()
                    );
                }
                Some(candidate)
            } else if proj.file_name() == Some(std::ffi::OsStr::new("Cargo.toml")) {
                Some(proj)
            } else {
                anyhow::bail!(
                    "--project file must be a Cargo.toml or directory, got: {}",
                    proj.display()
                );
            }
        } else {
            self.manifest_path
        };

        CargoWorkspaceSession::new(
            self.package.as_deref(),
            self.bin.as_deref(),
            self.features,
            self.no_default_features,
            effective_manifest_path,
            self.target_dir,
            self.offline,
        )
    }
}

#[derive(Parser, Debug, Clone)]
pub struct PlatformTargetArguments {
    /// Build with debug profile (default)
    #[clap(
        long,
        conflicts_with = "release",
        help_heading = "Optimization & Build Mode"
    )]
    pub debug: bool,

    /// Build with release optimizations
    #[clap(
        long,
        short,
        conflicts_with = "debug",
        help_heading = "Optimization & Build Mode"
    )]
    pub release: bool,

    /// Target operating system: android, ios, linux, macos, windows, web
    #[clap(
        long,
        short = 'p',
        conflicts_with = "device",
        help_heading = "Target Platform & Architecture"
    )]
    pub platform: Option<TargetPlatform>,

    /// Target architecture: arm64, x64, arm, x86
    #[clap(
        long,
        short = 'a',
        requires = "platform",
        help_heading = "Target Platform & Architecture"
    )]
    pub arch: Option<TargetArch>,

    /// Target Apple iOS Simulator runtime instead of physical device
    #[clap(long, help_heading = "Target Platform & Architecture")]
    pub simulator: bool,

    /// Target static Musl libc on Linux for fully self-contained portable binaries
    #[clap(long, help_heading = "Target Platform & Architecture")]
    pub musl: bool,

    /// Compression backend for Linux AppImage SquashFS generation (auto, backhand, system)
    #[clap(
        long,
        value_name = "BACKEND",
        help_heading = "Target Platform & Architecture"
    )]
    pub squashfs_backend: Option<crorust_linux::SquashfsBackend>,

    /// Automatically target the first detected connected physical or virtual device
    #[clap(
        long,
        short = '1',
        conflicts_with = "device",
        help_heading = "Target Device & Execution"
    )]
    pub first_device: bool,

    /// Target device identifier: adb:`<id>`, imd:`<udid>`, sim:`<udid>`, or host
    #[clap(
        long,
        conflicts_with = "store",
        help_heading = "Target Device & Execution"
    )]
    pub device: Option<String>,

    /// Package format override (apk, aab, app, dmg, ipa, appimage, msix, zip, tar)
    #[clap(
        long,
        conflicts_with = "store",
        help_heading = "Packaging & Code Signing"
    )]
    pub format: Option<PackageFormat>,

    /// Target store distribution pipeline
    #[clap(
        long,
        conflicts_with = "device",
        conflicts_with = "format",
        help_heading = "Packaging & Code Signing"
    )]
    pub store: Option<AppStore>,

    /// Path to cryptographic PEM certificate/key for signing packages
    #[clap(long, help_heading = "Packaging & Code Signing")]
    pub pem: Option<PathBuf>,

    /// Path to Apple mobileprovision provisioning profile for iOS signing
    #[clap(long, help_heading = "Packaging & Code Signing")]
    pub provisioning_profile: Option<PathBuf>,

    /// Path to Apple App Store Connect unified API key JSON for code-signing & notarization
    #[clap(long, help_heading = "Packaging & Code Signing")]
    pub api_key: Option<PathBuf>,

    /// Opt-in to legacy Gradle project generation for Android builds (default: standalone)
    #[clap(
        long,
        default_value = "false",
        help_heading = "Packaging & Code Signing"
    )]
    pub gradle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Build,
    Run,
    Deploy,
}

impl PlatformTargetArguments {
    pub fn opt(&self) -> TargetOpt {
        self.opt_for_mode(BuildMode::Build)
    }

    pub fn opt_for_mode(&self, mode: BuildMode) -> TargetOpt {
        if self.debug {
            TargetOpt::Debug
        } else if self.release || mode == BuildMode::Deploy || self.store.is_some() {
            TargetOpt::Release
        } else {
            TargetOpt::Debug
        }
    }

    pub fn opt_for_context(&self, is_deploy: bool) -> TargetOpt {
        let mode = if is_deploy {
            BuildMode::Deploy
        } else {
            BuildMode::Run
        };
        self.opt_for_mode(mode)
    }

    pub fn compile_build_target(self, config: &ManifestConfig) -> Result<SessionBuildTarget> {
        self.build_target_with_mode(config, BuildMode::Build)
    }

    pub fn build_target_with_context(
        self,
        config: &ManifestConfig,
        is_deploy: bool,
    ) -> Result<SessionBuildTarget> {
        let mode = if is_deploy {
            BuildMode::Deploy
        } else {
            BuildMode::Run
        };
        self.build_target_with_mode(config, mode)
    }

    fn resolve_signing_key(&self) -> Result<Option<IdentityKeypair>> {
        if let Some(pem_path) = self.pem.as_ref() {
            anyhow::ensure!(
                pem_path.is_file(),
                "Specified cryptographic PEM file does not exist: {}",
                pem_path.display()
            );
            return Ok(Some(IdentityKeypair::load_from_pem_file(pem_path)?));
        }
        if let Ok(pem_env_content) = std::env::var("CRORUST_PEM") {
            return Ok(Some(IdentityKeypair::new(&pem_env_content)?));
        }
        Ok(None)
    }

    fn resolve_target_device(
        &self,
        mode: BuildMode,
        store: Option<AppStore>,
    ) -> Result<Option<TargetDevice>> {
        if let Some(dev_spec) = self.device.as_deref() {
            return Ok(Some(TargetDevice::parse_target_device(dev_spec)?));
        }
        if self.first_device {
            return Ok(Some(TargetDevice::first_connected(self.platform)?));
        }
        match mode {
            BuildMode::Run | BuildMode::Deploy => {
                if self.platform == Some(TargetPlatform::Web) {
                    Ok(None)
                } else {
                    Ok(Some(TargetDevice::auto_select(self.platform)?))
                }
            }
            BuildMode::Build => {
                if self.platform.is_none() && store.is_none() {
                    Ok(Some(TargetDevice::local_host_device()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn resolve_deployment_platform(
        &self,
        store: Option<AppStore>,
        device: Option<&TargetDevice>,
    ) -> Result<TargetPlatform> {
        if let Some(target_os) = self.platform {
            return Ok(target_os);
        }
        if let Some(target_store) = store {
            return match target_store {
                AppStore::Apple => anyhow::bail!(
                    "Targeting Apple App Store requires explicit platform flag (-p/--platform)"
                ),
                AppStore::Microsoft => Ok(TargetPlatform::Windows),
                AppStore::Play => Ok(TargetPlatform::Android),
                AppStore::Sideload => anyhow::bail!(
                    "Direct Sideload distribution requires explicit platform flag (-p/--platform)"
                ),
            };
        }
        if let Some(connected_dev) = device {
            return connected_dev.platform();
        }
        TargetPlatform::detect_host()
    }

    fn resolve_target_architectures(
        &self,
        platform: TargetPlatform,
        store: Option<AppStore>,
        device: Option<&TargetDevice>,
    ) -> Result<Vec<TargetArch>> {
        if let Some(explicit_arch) = self.arch {
            return Ok(vec![explicit_arch]);
        }
        if platform == TargetPlatform::Web {
            return Ok(vec![TargetArch::X64]);
        }
        if let Some(target_store) = store {
            return match target_store {
                AppStore::Apple => Ok(vec![TargetArch::X64, TargetArch::Arm64]),
                AppStore::Microsoft => Ok(vec![TargetArch::X64]),
                AppStore::Play => Ok(vec![TargetArch::Arm64]),
                AppStore::Sideload => anyhow::bail!(
                    "Sideload packaging requires explicit target architecture (-a/--arc)"
                ),
            };
        }
        if let Some(connected_dev) = device {
            return Ok(vec![connected_dev.arch()?]);
        }
        Ok(vec![TargetArch::detect_host()?])
    }

    fn resolve_packaging_container(
        &self,
        config: &ManifestConfig,
        platform: TargetPlatform,
        opt: TargetOpt,
        store: Option<AppStore>,
    ) -> PackageFormat {
        if let Some(explicit_format) = self.format {
            return explicit_format;
        }
        if store == Some(AppStore::Play) {
            return PackageFormat::Aab;
        }
        let user_wants_gradle = self.gradle || config.android_platform_config().use_gradle();
        PackageFormat::default_for_platform(platform, opt, user_wants_gradle)
    }

    fn resolve_apple_profile(&self) -> Result<Option<Vec<u8>>> {
        if let Some(profile_path) = self.provisioning_profile.as_ref() {
            anyhow::ensure!(
                profile_path.is_file(),
                "Provisioning profile does not exist on disk: {}",
                profile_path.display()
            );
            return Ok(Some(std::fs::read(profile_path)?));
        }
        if let Ok(mut encoded) = std::env::var("CRORUST_PROVISIONING_PROFILE") {
            encoded.retain(|ch| !ch.is_whitespace());
            return Ok(Some(base64::decode(&encoded)?));
        }
        Ok(None)
    }

    fn resolve_apple_environment(&self, platform: TargetPlatform) -> Option<AppleTargetEnv> {
        if self.simulator || (platform == TargetPlatform::Ios && self.arch == Some(TargetArch::X64))
        {
            Some(AppleTargetEnv::Simulator)
        } else if platform == TargetPlatform::Ios {
            Some(AppleTargetEnv::Device)
        } else {
            None
        }
    }

    fn resolve_linux_standard_library(&self, platform: TargetPlatform) -> Option<LinuxLibcVariant> {
        if self.musl {
            Some(LinuxLibcVariant::Musl)
        } else if platform == TargetPlatform::Linux {
            Some(LinuxLibcVariant::Gnu)
        } else {
            None
        }
    }

    pub fn build_target_with_mode(
        self,
        config: &ManifestConfig,
        mode: BuildMode,
    ) -> Result<SessionBuildTarget> {
        let opt = self.opt_for_mode(mode);
        let store = self.store;
        let signer = self.resolve_signing_key()?;
        let device = self.resolve_target_device(mode, store)?;
        let platform = self.resolve_deployment_platform(store, device.as_ref())?;
        let archs = self.resolve_target_architectures(platform, store, device.as_ref())?;
        let format = self.resolve_packaging_container(config, platform, opt, store);
        let android_gradle = self.gradle || config.android_platform_config().use_gradle();
        let provisioning_profile = self.resolve_apple_profile()?;
        let apple_env = self.resolve_apple_environment(platform);
        let linux_libc = self.resolve_linux_standard_library(platform);
        let squashfs_backend = self
            .squashfs_backend
            .or(config.linux_platform_config().squashfs_backend);
        let api_key = self.api_key;

        Ok(SessionBuildTarget {
            opt,
            platform,
            archs,
            format,
            apple_env,
            linux_libc,
            squashfs_backend,
            device,
            store,
            signer,
            provisioning_profile,
            api_key,
            android_gradle,
        })
    }
}
