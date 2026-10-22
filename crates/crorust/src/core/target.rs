use anyhow::Result;
use clap::ValueEnum;
use crorust_common::IdentityKeypair;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetOpt {
    Debug,
    Release,
}

impl fmt::Display for TargetOpt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Debug => "debug",
            Self::Release => "release",
        };
        f.write_str(label)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, ValueEnum)]
pub enum TargetPlatform {
    Android,
    Ios,
    Linux,
    Macos,
    Windows,
    Web,
}

impl TargetPlatform {
    pub fn detect_host() -> Result<Self> {
        if cfg!(target_os = "linux") {
            Ok(Self::Linux)
        } else if cfg!(target_os = "macos") {
            Ok(Self::Macos)
        } else if cfg!(target_os = "windows") {
            Ok(Self::Windows)
        } else {
            anyhow::bail!("Unsupported host workstation operating system environment")
        }
    }
}

impl fmt::Display for TargetPlatform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self {
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Web => "web",
        };
        f.write_str(tag)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TargetArch {
    Arm64,
    X64,
}

impl TargetArch {
    pub fn detect_host() -> Result<Self> {
        if cfg!(target_arch = "x86_64") {
            Ok(Self::X64)
        } else if cfg!(target_arch = "aarch64") {
            Ok(Self::Arm64)
        } else {
            anyhow::bail!("Unsupported host CPU instruction set for native build orchestration")
        }
    }
}

impl fmt::Display for TargetArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let arch_tag = match self {
            Self::Arm64 => "arm64",
            Self::X64 => "x64",
        };
        f.write_str(arch_tag)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PackageFormat {
    Aab,
    Apk,
    Appbundle,
    Appdir,
    Appimage,
    Dmg,
    Exe,
    Ipa,
    Msix,
    Web,
}

impl fmt::Display for PackageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match self {
            Self::Aab => "aab",
            Self::Apk => "apk",
            Self::Appbundle => "appbundle",
            Self::Appdir => "appdir",
            Self::Appimage => "appimage",
            Self::Dmg => "dmg",
            Self::Exe => "exe",
            Self::Ipa => "ipa",
            Self::Msix => "msix",
            Self::Web => "web",
        };
        f.write_str(repr)
    }
}

impl PackageFormat {
    pub fn default_for_platform(
        os: TargetPlatform,
        profile: TargetOpt,
        use_gradle_flow: bool,
    ) -> Self {
        match os {
            TargetPlatform::Android => {
                if profile == TargetOpt::Release && use_gradle_flow {
                    Self::Aab
                } else {
                    Self::Apk
                }
            }
            TargetPlatform::Ios => {
                if profile == TargetOpt::Release {
                    Self::Ipa
                } else {
                    Self::Appbundle
                }
            }
            TargetPlatform::Linux => {
                if profile == TargetOpt::Release {
                    Self::Appimage
                } else {
                    Self::Appdir
                }
            }
            TargetPlatform::Macos => {
                if profile == TargetOpt::Release {
                    Self::Dmg
                } else {
                    Self::Appbundle
                }
            }
            TargetPlatform::Windows => Self::Exe,
            TargetPlatform::Web => Self::Web,
        }
    }

    pub fn file_extension(self) -> &'static str {
        match self {
            Self::Aab => "aab",
            Self::Apk => "apk",
            Self::Appbundle => "app",
            Self::Appdir => "AppDir",
            Self::Appimage => "AppImage",
            Self::Dmg => "dmg",
            Self::Exe => "exe",
            Self::Ipa => "ipa",
            Self::Msix => "msix",
            Self::Web => "web",
        }
    }

    pub fn allows_multiarch_payload(self) -> bool {
        self == Self::Aab || self == Self::Apk
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AppStore {
    Apple,
    Microsoft,
    Play,
    Sideload,
}

impl fmt::Display for AppStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let store_tag = match self {
            Self::Apple => "apple",
            Self::Microsoft => "microsoft",
            Self::Play => "play",
            Self::Sideload => "sideload",
        };
        f.write_str(store_tag)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleTargetEnv {
    Device,
    Simulator,
}

impl fmt::Display for AppleTargetEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let env_label = match self {
            Self::Device => "device",
            Self::Simulator => "sim",
        };
        f.write_str(env_label)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinuxLibcVariant {
    #[default]
    Gnu,
    Musl,
}

impl fmt::Display for LinuxLibcVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let libc_label = match self {
            Self::Gnu => "gnu",
            Self::Musl => "musl",
        };
        f.write_str(libc_label)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetCompilationUnit {
    platform: TargetPlatform,
    arch: TargetArch,
    opt: TargetOpt,
    apple_env: Option<AppleTargetEnv>,
    linux_libc: Option<LinuxLibcVariant>,
}

impl TargetCompilationUnit {
    pub fn new(
        target_os: TargetPlatform,
        instruction_set: TargetArch,
        build_opt: TargetOpt,
    ) -> Self {
        Self {
            platform: target_os,
            arch: instruction_set,
            opt: build_opt,
            apple_env: None,
            linux_libc: None,
        }
    }

    pub fn with_apple_env(mut self, environment_kind: AppleTargetEnv) -> Self {
        self.apple_env = Some(environment_kind);
        self
    }

    pub fn with_linux_libc(mut self, c_library: LinuxLibcVariant) -> Self {
        self.linux_libc = Some(c_library);
        self
    }

    #[inline]
    pub fn target_platform(self) -> TargetPlatform {
        self.platform
    }

    #[inline]
    pub fn target_arch(self) -> TargetArch {
        self.arch
    }

    #[inline]
    pub fn optimization_profile(self) -> TargetOpt {
        self.opt
    }

    #[inline]
    pub fn target_apple_env(self) -> Option<AppleTargetEnv> {
        self.apple_env
    }

    #[inline]
    pub fn target_linux_libc(self) -> Option<LinuxLibcVariant> {
        self.linux_libc
    }

    pub fn apple_version_min_flag(&self, minimum_release: &str) -> Option<String> {
        if self.platform == TargetPlatform::Ios {
            let flag = if self.apple_env == Some(AppleTargetEnv::Simulator) {
                format!("-mios-simulator-version-min={minimum_release}")
            } else {
                format!("-miphoneos-version-min={minimum_release}")
            };
            Some(flag)
        } else if self.platform == TargetPlatform::Macos {
            Some(format!("-mmacosx-version-min={minimum_release}"))
        } else {
            None
        }
    }

    pub fn android_target_abi(self) -> crorust_android::AndroidAbi {
        assert_eq!(
            self.target_platform(),
            TargetPlatform::Android,
            "ABI lookup requires Android platform"
        );
        if self.target_arch() == TargetArch::Arm64 {
            crorust_android::AndroidAbi::Arm64V8a
        } else {
            crorust_android::AndroidAbi::X86_64
        }
    }

    pub fn android_ndk_triple(self) -> &'static str {
        assert_eq!(
            self.target_platform(),
            TargetPlatform::Android,
            "NDK triple requires Android platform"
        );
        if self.target_arch() == TargetArch::Arm64 {
            "aarch64-linux-android"
        } else {
            "x86_64-linux-android"
        }
    }

    pub fn rust_target_triple(self) -> Result<&'static str> {
        let triple = match self.platform {
            TargetPlatform::Android => match self.arch {
                TargetArch::Arm64 => "aarch64-linux-android",
                TargetArch::X64 => "x86_64-linux-android",
            },
            TargetPlatform::Ios => match self.arch {
                TargetArch::Arm64 if self.apple_env == Some(AppleTargetEnv::Simulator) => {
                    "aarch64-apple-ios-sim"
                }
                TargetArch::Arm64 => "aarch64-apple-ios",
                TargetArch::X64 => "x86_64-apple-ios",
            },
            TargetPlatform::Macos => match self.arch {
                TargetArch::Arm64 => "aarch64-apple-darwin",
                TargetArch::X64 => "x86_64-apple-darwin",
            },
            TargetPlatform::Windows => match self.arch {
                TargetArch::Arm64 => "aarch64-pc-windows-msvc",
                TargetArch::X64 => "x86_64-pc-windows-msvc",
            },
            TargetPlatform::Linux => match (self.arch, self.linux_libc) {
                (TargetArch::Arm64, Some(LinuxLibcVariant::Musl)) => "aarch64-unknown-linux-musl",
                (TargetArch::Arm64, _) => "aarch64-unknown-linux-gnu",
                (TargetArch::X64, Some(LinuxLibcVariant::Musl)) => "x86_64-unknown-linux-musl",
                (TargetArch::X64, _) => "x86_64-unknown-linux-gnu",
            },
            TargetPlatform::Web => "wasm32-unknown-unknown",
        };
        Ok(triple)
    }

    pub fn targets_current_host(self) -> Result<bool> {
        let match_os = self.target_platform() == TargetPlatform::detect_host()?;
        let match_cpu = self.target_arch() == TargetArch::detect_host()?;
        let standard_runtime = self.apple_env.is_none()
            && self.linux_libc.unwrap_or(LinuxLibcVariant::Gnu) == LinuxLibcVariant::Gnu;
        Ok(match_os && match_cpu && standard_runtime)
    }
}

impl fmt::Display for TargetCompilationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.platform, self.arch)?;
        if let Some(env_marker) = self.apple_env {
            write!(f, "-{env_marker}")?;
        }
        if let Some(libc_kind) = self.linux_libc {
            if libc_kind == LinuxLibcVariant::Musl {
                write!(f, "-{libc_kind}")?;
            }
        }
        write!(f, "-{}", self.opt)
    }
}

#[derive(Clone, Debug)]
pub struct SessionBuildTarget {
    pub(crate) opt: TargetOpt,
    pub(crate) platform: TargetPlatform,
    pub(crate) archs: Vec<TargetArch>,
    pub(crate) format: PackageFormat,
    pub(crate) apple_env: Option<AppleTargetEnv>,
    pub(crate) linux_libc: Option<LinuxLibcVariant>,
    pub(crate) squashfs_backend: Option<crorust_linux::SquashfsBackend>,
    pub(crate) device: Option<crate::devices::TargetDevice>,
    pub(crate) store: Option<AppStore>,
    pub(crate) signer: Option<IdentityKeypair>,
    pub(crate) provisioning_profile: Option<Vec<u8>>,
    pub(crate) api_key: Option<PathBuf>,
    pub(crate) android_gradle: bool,
}

impl SessionBuildTarget {
    #[inline]
    pub fn optimization_profile(&self) -> TargetOpt {
        self.opt
    }

    #[inline]
    pub fn target_platform(&self) -> TargetPlatform {
        self.platform
    }

    #[inline]
    pub fn target_architectures(&self) -> &[TargetArch] {
        &self.archs
    }

    #[inline]
    pub fn package_format(&self) -> PackageFormat {
        self.format
    }

    #[inline]
    pub fn target_apple_env(&self) -> Option<AppleTargetEnv> {
        self.apple_env
    }

    #[inline]
    pub fn target_linux_libc(&self) -> Option<LinuxLibcVariant> {
        self.linux_libc
    }

    #[inline]
    pub fn target_squashfs_backend(&self) -> Option<crorust_linux::SquashfsBackend> {
        self.squashfs_backend
    }

    #[inline]
    pub fn target_device(&self) -> Option<&crate::devices::TargetDevice> {
        self.device.as_ref()
    }

    #[inline]
    pub fn target_store(&self) -> Option<AppStore> {
        self.store
    }

    pub fn iter_compile_targets(&self) -> impl Iterator<Item = TargetCompilationUnit> + '_ {
        self.archs.iter().map(move |&instruction_set| {
            let mut unit = TargetCompilationUnit::new(self.platform, instruction_set, self.opt);
            if let Some(target_env) = self.apple_env {
                unit = unit.with_apple_env(target_env);
            }
            if let Some(target_libc) = self.linux_libc {
                unit = unit.with_linux_libc(target_libc);
            }
            unit
        })
    }

    pub fn targets_current_host(&self) -> bool {
        self.device
            .as_ref()
            .map(|d| d.targets_current_host())
            .unwrap_or(false)
    }

    #[inline]
    pub fn target_signer(&self) -> Option<&IdentityKeypair> {
        self.signer.as_ref()
    }

    #[inline]
    pub fn apple_provisioning_profile(&self) -> Option<&[u8]> {
        self.provisioning_profile.as_deref()
    }

    #[inline]
    pub fn apple_api_key(&self) -> Option<&Path> {
        self.api_key.as_deref()
    }

    #[inline]
    pub fn is_android_gradle_enabled(&self) -> bool {
        self.android_gradle
    }
}
