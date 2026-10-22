use super::args::{BuildMode, CargoPipelineArguments, ProjectBuildArguments};
use super::target::{
    AppleTargetEnv, LinuxLibcVariant, PackageFormat, SessionBuildTarget, TargetArch,
    TargetCompilationUnit, TargetPlatform,
};
use crate::cargo::{CargoCompilationEnvironment, CargoWorkspaceSession, CompilationCrateKind};
use crate::config::ManifestConfig;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub use crorust_common::sanitize_path;

pub fn resolve_ndk_sysroot(path: &Path) -> PathBuf {
    if path.join("usr").join("lib").exists() {
        return dunce::simplified(path).to_path_buf();
    }
    let prebuilt = path.join("toolchains").join("llvm").join("prebuilt");
    if prebuilt.exists() {
        if let Ok(entries) = std::fs::read_dir(&prebuilt) {
            for entry in entries.flatten() {
                let sysroot = entry.path().join("sysroot");
                if sysroot.join("usr").join("lib").exists() {
                    return dunce::simplified(&sysroot).to_path_buf();
                }
            }
        }
    }
    dunce::simplified(path).to_path_buf()
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AndroidSdkRoot(pub PathBuf);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AndroidNdkRoot(pub PathBuf);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AppleSdkSysroot(pub PathBuf);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WindowsSdkDirectory(pub PathBuf);

pub struct SessionExecutionContext {
    pub(crate) build_target: SessionBuildTarget,
    pub(crate) build_dir: PathBuf,
    pub(crate) cache_dir: PathBuf,
    pub(crate) cargo: CargoWorkspaceSession,
    pub(crate) config: ManifestConfig,
    pub(crate) name: String,
    pub(crate) icon: Option<PathBuf>,

    pub(crate) verbose: bool,
    pub(crate) offline: bool,
    pub(crate) skip_artifact_verification: bool,
}

impl SessionExecutionContext {
    pub fn new(args: ProjectBuildArguments) -> Result<Self> {
        Self::new_with_mode(args, BuildMode::Build)
    }

    pub fn new_with_context(args: ProjectBuildArguments, is_deploy: bool) -> Result<Self> {
        let mode = if is_deploy {
            BuildMode::Deploy
        } else {
            BuildMode::Run
        };
        Self::new_with_mode(args, mode)
    }

    pub fn new_with_mode(args: ProjectBuildArguments, mode: BuildMode) -> Result<Self> {
        let verbose = args.verbose;
        let offline = args.cargo.offline;
        let skip_artifact_verification = args.cargo.skip_artifact_verification;
        let cargo = args.cargo.clone().cargo()?;
        let package = cargo
            .package_manifest_data()
            .package
            .as_ref()
            .context("Cargo.toml is missing a [package] section")?;

        let mut config = if let Some(ws_root) = cargo.workspace_root_path() {
            let ws_cargo = ws_root.join("Cargo.toml");
            let mut ws_config = ManifestConfig::parse_project(ws_root, &ws_cargo)?;
            let pkg_root = cargo.package_root_path();
            if pkg_root != ws_root {
                let pkg_cargo = pkg_root.join("Cargo.toml");
                let pkg_config = ManifestConfig::parse_project(pkg_root, &pkg_cargo)?;
                ws_config.merge(pkg_config);
            }
            ws_config
        } else {
            let pkg_root = cargo.package_root_path();
            let pkg_cargo = pkg_root.join("Cargo.toml");
            ManifestConfig::parse_project(pkg_root, &pkg_cargo)?
        };

        let project_root = cargo
            .workspace_root_path()
            .unwrap_or_else(|| cargo.package_root_path());

        let cache_dir = Self::resolve_cache_dir(project_root, &config, &args.cargo);
        std::fs::create_dir_all(&cache_dir).ok();

        let build_dir = cargo.target_directory().join("crorust");

        let opt = args.build_target.opt_for_mode(mode);
        config.harmonize_with_cargo_package(package, cargo.workspace_manifest_data(), opt)?;
        let build_target = args.build_target.build_target_with_mode(&config, mode)?;
        let icon = config
            .app_icon(build_target.target_platform())
            .map(|icon| cargo.package_root_path().join(icon));

        let name = if build_target.target_platform() == TargetPlatform::Android {
            package.name.clone()
        } else {
            cargo.selected_bin().unwrap_or(&package.name).to_string()
        };

        Ok(Self {
            build_target,
            build_dir,
            cache_dir,
            cargo,
            config,
            name,
            icon,
            verbose,
            offline,
            skip_artifact_verification,
        })
    }

    pub fn resolve_cache_dir(
        project_root: &Path,
        config: &crate::config::ManifestConfig,
        args: &CargoPipelineArguments,
    ) -> PathBuf {
        if let Some(custom_dir) = &args.cache_dir {
            return dunce::simplified(custom_dir).to_path_buf();
        }
        if args.global_cache {
            return Self::global_cache_dir();
        }
        if args.project_cache {
            return project_root.join(".cache").join("crorust");
        }
        if let Ok(env_path) = std::env::var("CRORUST_CACHE_DIR") {
            let trimmed = env_path.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
        if let Ok(env_scope) = std::env::var("CRORUST_CACHE_SCOPE") {
            if let Ok(scope) = env_scope.parse::<crate::config::CacheScope>() {
                return match scope {
                    crate::config::CacheScope::Global => Self::global_cache_dir(),
                    crate::config::CacheScope::Project => {
                        project_root.join(".cache").join("crorust")
                    }
                };
            }
        }
        if let Some(cfg_dir) = config.toolchain.explicit_cache_dir() {
            if cfg_dir.is_absolute() {
                return cfg_dir.to_path_buf();
            } else {
                return project_root.join(cfg_dir);
            }
        }
        match config.toolchain.cache_scope() {
            crate::config::CacheScope::Global => Self::global_cache_dir(),
            crate::config::CacheScope::Project => project_root.join(".cache").join("crorust"),
        }
    }

    pub fn global_cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("crorust")
    }

    pub fn app_name(&self) -> &str {
        &self.name
    }

    pub fn package_name(&self) -> &str {
        self.cargo.package_identifier()
    }

    pub fn bin_name(&self) -> Option<&str> {
        self.cargo.selected_bin()
    }

    pub fn build_target(&self) -> &SessionBuildTarget {
        &self.build_target
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    pub fn is_offline(&self) -> bool {
        self.offline
    }

    pub fn skip_artifact_verification(&self) -> bool {
        self.skip_artifact_verification
    }

    pub fn project_root_dir(&self) -> &Path {
        self.cargo.package_root_path()
    }

    pub fn build_directory(&self) -> &Path {
        &self.build_dir
    }

    pub fn icon_cache_dir(&self) -> PathBuf {
        self.build_directory().join("cache").join("icons")
    }

    pub fn cache_directory(&self) -> &Path {
        &self.cache_dir
    }

    pub fn task_cache_dir(&self) -> PathBuf {
        self.cache_directory().join("tasks")
    }

    pub fn task_reporter(&self) -> crate::task::CliTaskReporter {
        crate::task::CliTaskReporter::new(self.is_verbose())
    }

    pub fn host_triple(&self) -> &'static str {
        self.cargo.host_triple()
    }

    pub fn optimization_directory(&self) -> PathBuf {
        self.build_directory()
            .join(self.build_target().optimization_profile().to_string())
    }

    pub fn target_platform_dir(&self) -> PathBuf {
        self.optimization_directory()
            .join(self.build_target().target_platform().to_string())
    }

    pub fn target_arch_dir(&self, arch: TargetArch) -> PathBuf {
        self.target_platform_dir().join(arch.to_string())
    }

    pub fn output_path(&self) -> PathBuf {
        let fmt = self.build_target().package_format();
        let target_platform = self.build_target().target_platform();
        let base_destination = if fmt.allows_multiarch_payload() {
            self.target_platform_dir()
        } else if target_platform == TargetPlatform::Web {
            if let Some(custom_dist) = &self.manifest_config().web.dist {
                if custom_dist.is_relative() {
                    self.project_root_dir().join(custom_dist)
                } else {
                    custom_dist.clone()
                }
            } else {
                self.project_root_dir()
                    .join(".cache")
                    .join("web-static")
                    .join("dist")
            }
        } else {
            let active_spec = self.build_target().iter_compile_targets().next().unwrap();
            self.target_arch_dir(active_spec.target_arch())
        };
        base_destination.join(format!("{}.{}", self.app_name(), fmt.file_extension()))
    }

    pub fn executable_path(&self) -> PathBuf {
        let package_output = self.output_path();
        match (
            self.build_target().package_format(),
            self.build_target().target_platform(),
        ) {
            (PackageFormat::Appdir, _) => package_output.join("AppRun"),
            (PackageFormat::Appbundle, TargetPlatform::Macos) => {
                package_output.join(format!("Contents/MacOS/{}", self.app_name()))
            }
            _ => package_output,
        }
    }

    pub fn app_icon_path(&self) -> Option<&Path> {
        self.icon.as_deref()
    }

    pub fn cargo_session(&self) -> &CargoWorkspaceSession {
        &self.cargo
    }

    pub fn manifest_config(&self) -> &ManifestConfig {
        &self.config
    }

    pub fn android_compile_sdk_version(&self) -> u32 {
        let android_cfg = self.manifest_config().android_platform_config();
        android_cfg
            .compile_sdk
            .or(android_cfg.manifest.compile_sdk_version)
            .or(android_cfg.target_sdk)
            .or(android_cfg.manifest.sdk.target_sdk_version)
            .unwrap_or(35)
    }

    pub fn android_target_sdk_version(&self) -> u32 {
        let android_cfg = self.manifest_config().android_platform_config();
        android_cfg
            .target_sdk
            .or(android_cfg.manifest.sdk.target_sdk_version)
            .unwrap_or(34)
    }

    pub fn android_min_sdk_version(&self) -> u32 {
        let android_cfg = self.manifest_config().android_platform_config();
        android_cfg
            .min_sdk
            .or(android_cfg.manifest.sdk.min_sdk_version)
            .unwrap_or(21)
    }

    fn resolve_configured_path(&self, path: Option<&Path>) -> Option<PathBuf> {
        path.map(|p| {
            let simplified = dunce::simplified(p);
            if simplified.is_absolute() {
                simplified.to_path_buf()
            } else {
                self.cargo.package_root_path().join(simplified)
            }
        })
    }

    pub fn locate_android_sdk(&self) -> PathBuf {
        if let Some(sdk) = self.resolve_configured_path(
            self.manifest_config()
                .android_platform_config()
                .sdk
                .as_deref(),
        ) {
            if sdk.exists() {
                return dunce::simplified(&sdk).to_path_buf();
            }
        }
        if let Ok(path) = std::env::var("ANDROID_HOME").map(PathBuf::from) {
            if path.exists() {
                return dunce::simplified(&path).to_path_buf();
            }
        }
        if let Ok(path) = std::env::var("ANDROID_SDK_ROOT").map(PathBuf::from) {
            if path.exists() {
                return dunce::simplified(&path).to_path_buf();
            }
        }
        let candidates: Vec<Option<PathBuf>> = if cfg!(target_os = "macos") {
            vec![
                dirs::home_dir().map(|h| h.join("Library").join("Android").join("sdk")),
                Some(PathBuf::from("/opt/homebrew/share/android-sdk")),
                Some(PathBuf::from("/usr/local/share/android-sdk")),
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                dirs::data_local_dir().map(|d| d.join("Android").join("Sdk")),
                dirs::home_dir()
                    .map(|h| h.join("AppData").join("Local").join("Android").join("Sdk")),
                std::env::var("ProgramFiles")
                    .ok()
                    .map(|p| PathBuf::from(p).join("Android").join("android-sdk")),
                std::env::var("ProgramFiles(x86)")
                    .ok()
                    .map(|p| PathBuf::from(p).join("Android").join("android-sdk")),
            ]
        } else {
            vec![
                dirs::home_dir().map(|h| h.join("Android").join("Sdk")),
                Some(PathBuf::from("/opt/android-sdk")),
                Some(PathBuf::from("/usr/lib/android-sdk")),
                Some(PathBuf::from("/opt/android-sdk-linux")),
            ]
        };

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                return dunce::simplified(&candidate).to_path_buf();
            }
        }
        self.resolve_android_sdk()
    }

    pub fn resolve_android_jar(&self) -> PathBuf {
        let sdk_version = self.android_compile_sdk_version();
        let target_relative = Path::new("platforms")
            .join(format!("android-{sdk_version}"))
            .join("android.jar");

        let discovered_sdk = self.locate_android_sdk();
        let system_jar = discovered_sdk.join(&target_relative);
        if system_jar.exists() {
            return dunce::simplified(&system_jar).to_path_buf();
        }
        self.resolve_android_sdk().join(target_relative)
    }

    pub fn resolve_windows_sdk(&self) -> PathBuf {
        let windows_cfg = self.manifest_config().windows_platform_config();

        if let Some(sdk) = self.resolve_configured_path(windows_cfg.sdk.as_deref()) {
            if sdk.exists() {
                return dunce::simplified(&sdk).to_path_buf();
            }
        }
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot("Windows.sdk") {
            let catalog_path = self
                .cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version);
            if catalog_path.exists() {
                return dunce::simplified(&catalog_path).to_path_buf();
            }
        }
        let versioned_win_sdk = self
            .cache_directory()
            .join("sysroots")
            .join("Windows.sdk")
            .join(windows_cfg.sdk_version());
        if versioned_win_sdk.exists() {
            return dunce::simplified(&versioned_win_sdk).to_path_buf();
        }

        let structured = self.cache_directory().join("sysroots").join("Windows.sdk");
        if structured.join("crt").exists() {
            return dunce::simplified(&structured).to_path_buf();
        }

        // If structured has versioned subdirectories, locate the installed SDK subfolder
        if let Ok(entries) = std::fs::read_dir(&structured) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && (p.join("crt").exists() || p.join(".complete").exists()) {
                    return dunce::simplified(&p).to_path_buf();
                }
            }
        }

        if structured.exists() {
            return dunce::simplified(&structured).to_path_buf();
        }

        dunce::simplified(&versioned_win_sdk).to_path_buf()
    }

    pub fn resolve_macos_sdk(&self) -> PathBuf {
        if let Some(sdk) = self.resolve_configured_path(
            self.manifest_config()
                .macos_platform_config()
                .sdk
                .as_deref(),
        ) {
            if sdk.exists() {
                return dunce::simplified(&sdk).to_path_buf();
            }
        }
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot("MacOSX.sdk") {
            let catalog_path = self
                .cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version);
            if catalog_path.exists() {
                return dunce::simplified(&catalog_path).to_path_buf();
            }
        }
        let structured = self.cache_directory().join("sysroots").join("MacOSX.sdk");
        if structured.exists() {
            return dunce::simplified(&structured).to_path_buf();
        }
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot("MacOSX.sdk") {
            dunce::simplified(
                &self
                    .cache_directory()
                    .join("sysroots")
                    .join(artifact.name)
                    .join(artifact.version),
            )
            .to_path_buf()
        } else {
            structured
        }
    }

    pub fn resolve_android_sdk(&self) -> PathBuf {
        self.cache_directory().join("Android.sdk")
    }

    pub fn resolve_ndk_sysroot(path: &Path) -> PathBuf {
        resolve_ndk_sysroot(path)
    }

    pub fn resolve_android_ndk(&self) -> PathBuf {
        let host_triple = self.cargo_session().host_triple();
        let android_cfg = self.manifest_config().android_platform_config();
        let expected_version = android_cfg.ndk_version.as_deref();

        if let Some(ndk) = self.resolve_configured_path(android_cfg.ndk.as_deref()) {
            if ndk.exists()
                && crate::download::catalog::is_ndk_version_compatible(&ndk, expected_version)
            {
                return resolve_ndk_sysroot(&ndk);
            }
        }

        if let Some(ndk_version) = expected_version {
            let local_sdk_ndk = self.locate_android_sdk().join("ndk").join(ndk_version);
            if local_sdk_ndk.exists() {
                return resolve_ndk_sysroot(&local_sdk_ndk);
            }

            let namespaced_ndk = self
                .cache_directory()
                .join("tools")
                .join(host_triple)
                .join("ndk")
                .join(ndk_version);
            if namespaced_ndk.exists() {
                return resolve_ndk_sysroot(&namespaced_ndk);
            }
        }

        if let Ok(ndk) = std::env::var("ANDROID_NDK_HOME")
            .or_else(|_| std::env::var("NDK_HOME"))
            .map(PathBuf::from)
        {
            if ndk.exists()
                && crate::download::catalog::is_ndk_version_compatible(&ndk, expected_version)
            {
                return resolve_ndk_sysroot(&ndk);
            }
        }

        let catalog_ndk = crate::download::catalog::ToolCatalog::get_ndk();
        let catalog_namespaced = self
            .cache_directory()
            .join("tools")
            .join(host_triple)
            .join("ndk")
            .join(catalog_ndk.version);
        if catalog_namespaced.exists() {
            return resolve_ndk_sysroot(&catalog_namespaced);
        }

        let default_namespaced = self
            .cache_directory()
            .join("tools")
            .join(host_triple)
            .join("ndk")
            .join("default");
        if default_namespaced.exists() {
            return resolve_ndk_sysroot(&default_namespaced);
        }

        if let Some(ndk_version) = expected_version {
            self.cache_directory()
                .join("tools")
                .join(host_triple)
                .join("ndk")
                .join(ndk_version)
        } else {
            catalog_namespaced
        }
    }

    pub fn locate_java_jdk(&self) -> Option<PathBuf> {
        if let Ok(java_home) = std::env::var("JAVA_HOME").map(PathBuf::from) {
            let java_bin = java_home
                .join("bin")
                .join(crorust_common::host_executable_name("java"));
            if java_bin.exists() {
                return Some(dunce::simplified(&java_home).to_path_buf());
            }
        }

        if let Ok(java_bin) = which::which("java") {
            if let Ok(real_java) = std::fs::canonicalize(&java_bin) {
                if let Some(parent) = real_java.parent() {
                    if parent.file_name().and_then(|n| n.to_str()) == Some("bin") {
                        if let Some(jdk_root) = parent.parent() {
                            if jdk_root
                                .join("bin")
                                .join(crorust_common::host_executable_name("java"))
                                .exists()
                            {
                                return Some(dunce::simplified(jdk_root).to_path_buf());
                            }
                        }
                    }
                }
            }
        }

        let candidates: Vec<Option<PathBuf>> = if cfg!(target_os = "macos") {
            vec![
                Some(PathBuf::from(
                    "/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home",
                )),
                Some(PathBuf::from("/Library/Java/Home")),
                Some(PathBuf::from("/opt/homebrew/opt/openjdk@17")),
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                std::env::var("ProgramFiles").ok().map(|p| {
                    PathBuf::from(p)
                        .join("Eclipse Adoptium")
                        .join("jdk-17.0.10.7-hotspot")
                }),
                std::env::var("ProgramFiles")
                    .ok()
                    .map(|p| PathBuf::from(p).join("Java")),
            ]
        } else {
            let mut paths = vec![
                Some(PathBuf::from("/usr/lib/jvm/default")),
                Some(PathBuf::from("/usr/lib/jvm/java-17-openjdk")),
                Some(PathBuf::from("/usr/lib/jvm/java-21-openjdk")),
                Some(PathBuf::from("/usr/lib/jvm/default-runtime")),
            ];
            if let Ok(entries) = std::fs::read_dir("/usr/lib/jvm") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path
                        .join("bin")
                        .join(crorust_common::host_executable_name("java"))
                        .exists()
                    {
                        paths.push(Some(path));
                    }
                }
            }
            paths
        };

        for candidate in candidates.into_iter().flatten() {
            if candidate
                .join("bin")
                .join(crorust_common::host_executable_name("java"))
                .exists()
            {
                return Some(dunce::simplified(&candidate).to_path_buf());
            }
        }

        let host_triple = self.cargo_session().host_triple();
        for version in &["17.0.10_7", "17.0.10+7"] {
            let namespaced_jdk = self
                .cache_directory()
                .join("tools")
                .join(host_triple)
                .join("jdk")
                .join(version);
            if namespaced_jdk.join(".complete").exists()
                && namespaced_jdk
                    .join("bin")
                    .join(crorust_common::host_executable_name("java"))
                    .exists()
            {
                return Some(dunce::simplified(&namespaced_jdk).to_path_buf());
            }
        }

        None
    }

    pub fn locate_d8(&self) -> Result<PathBuf> {
        let sdk_root = self.locate_android_sdk();
        let build_tools = sdk_root.join("build-tools");

        if build_tools.exists() {
            if let Ok(read_dir) = std::fs::read_dir(&build_tools) {
                let mut entries: Vec<_> = read_dir
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .collect();
                entries.sort_by_key(|e| e.file_name());

                // Check newest versions first for d8 executable / batch script
                for entry in entries.iter().rev() {
                    let d8_candidates = if cfg!(target_os = "windows") {
                        vec!["d8.bat", "d8.cmd", "d8.exe", "d8"]
                    } else {
                        vec!["d8"]
                    };

                    for candidate in d8_candidates {
                        let bin = entry.path().join(candidate);
                        if bin.is_file() {
                            return Ok(dunce::simplified(&bin).to_path_buf());
                        }
                    }
                }
            }
        }

        // Fallback: check PATH
        let candidates = if cfg!(target_os = "windows") {
            vec!["d8.bat", "d8.cmd", "d8.exe", "d8"]
        } else {
            vec!["d8"]
        };

        for candidate in candidates {
            if let Ok(p) = which::which(candidate) {
                return Ok(dunce::simplified(&p).to_path_buf());
            }
        }

        anyhow::bail!(
            "Failed to locate Android `d8` compiler. Please install Android SDK build-tools."
        )
    }

    pub fn resolve_ios_sdk(&self) -> PathBuf {
        if let Some(sdk) = self
            .resolve_configured_path(self.manifest_config().ios_platform_config().sdk.as_deref())
        {
            if sdk.exists() {
                return dunce::simplified(&sdk).to_path_buf();
            }
        }
        if let Some(host_sdk) =
            crate::download::apple_discovery::AppleSdkDiscovery::locate_ios_device_sdk()
        {
            return dunce::simplified(&host_sdk).to_path_buf();
        }
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot("iPhoneOS.sdk") {
            let catalog_path = self
                .cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version);
            if catalog_path.exists() {
                return dunce::simplified(&catalog_path).to_path_buf();
            }
        }
        let structured = self.cache_directory().join("sysroots").join("iPhoneOS.sdk");
        if structured.exists() {
            return dunce::simplified(&structured).to_path_buf();
        }
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot("iPhoneOS.sdk") {
            dunce::simplified(
                &self
                    .cache_directory()
                    .join("sysroots")
                    .join(artifact.name)
                    .join(artifact.version),
            )
            .to_path_buf()
        } else {
            structured
        }
    }

    pub fn resolve_ios_simulator_sdk(&self) -> PathBuf {
        if let Some(sdk) = self
            .resolve_configured_path(self.manifest_config().ios_platform_config().sdk.as_deref())
        {
            if sdk.exists() {
                return dunce::simplified(&sdk).to_path_buf();
            }
        }
        if let Some(host_sdk) =
            crate::download::apple_discovery::AppleSdkDiscovery::locate_ios_simulator_sdk()
        {
            return dunce::simplified(&host_sdk).to_path_buf();
        }
        if let Some(artifact) =
            crate::download::catalog::ToolCatalog::get_sysroot("iPhoneSimulator.sdk")
        {
            let catalog_path = self
                .cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version);
            if catalog_path.exists() {
                return dunce::simplified(&catalog_path).to_path_buf();
            }
        }
        let structured = self
            .cache_directory()
            .join("sysroots")
            .join("iPhoneSimulator.sdk");
        if structured.exists() {
            return structured;
        }
        if let Some(artifact) =
            crate::download::catalog::ToolCatalog::get_sysroot("iPhoneSimulator.sdk")
        {
            self.cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version)
        } else {
            structured
        }
    }

    pub fn resolve_developer_disk_image(&self, major: u32, minor: u32) -> PathBuf {
        self.cache_directory()
            .join("iPhoneOS.platform")
            .join("DeviceSupport")
            .join(format!("{major}.{minor}"))
            .join("DeveloperDiskImage.dmg")
    }

    pub fn resolve_lldb_server(&self, target: TargetCompilationUnit) -> Result<PathBuf> {
        match target.target_platform() {
            TargetPlatform::Android => {
                let ndk = self.resolve_android_ndk();
                let lib_dir = ndk
                    .join("usr")
                    .join("lib")
                    .join(target.android_ndk_triple());
                Ok(lib_dir.join("lldb-server"))
            }
            TargetPlatform::Ios => {
                anyhow::bail!(
                    "Direct lldb-server path resolution for iOS devices is not supported; iOS debugserver is mounted via DeveloperDiskImage"
                );
            }
            _ => Ok(which::which("lldb-server")?),
        }
    }

    pub fn linux_sysroot(&self) -> PathBuf {
        let arch_name = match self
            .build_target()
            .target_architectures()
            .first()
            .copied()
            .unwrap_or(TargetArch::X64)
        {
            TargetArch::X64 => "Linux.sysroot-x86_64",
            TargetArch::Arm64 => "Linux.sysroot-aarch64",
        };
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot(arch_name) {
            let catalog_path = self
                .cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version);
            if catalog_path.exists() {
                return catalog_path;
            }
        }
        let structured = self
            .cache_directory()
            .join("sysroots")
            .join("Linux.sysroot");
        if structured.exists() {
            return structured;
        }
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_sysroot(arch_name) {
            self.cache_directory()
                .join("sysroots")
                .join(artifact.name)
                .join(artifact.version)
        } else {
            structured
        }
    }

    pub fn prepare_cargo_compilation(
        &self,
        target: TargetCompilationUnit,
        target_dir: &Path,
    ) -> Result<CargoCompilationEnvironment> {
        let mut builder = self
            .cargo
            .create_compilation_invocation(target, target_dir)?;
        match target.target_platform() {
            TargetPlatform::Linux => self.configure_linux_cargo_invocation(&mut builder, target)?,
            TargetPlatform::Android => self.configure_android_cargo_invocation(&mut builder)?,
            TargetPlatform::Windows => {
                self.configure_windows_cargo_invocation(&mut builder, target)?
            }
            TargetPlatform::Macos => self.configure_macos_cargo_invocation(&mut builder, target)?,
            TargetPlatform::Ios => self.configure_ios_cargo_invocation(&mut builder, target)?,
            _ => {}
        }
        Ok(builder)
    }

    fn configure_linux_cargo_invocation(
        &self,
        cargo: &mut CargoCompilationEnvironment,
        target: TargetCompilationUnit,
    ) -> Result<()> {
        if !target.targets_current_host()? {
            if target.target_linux_libc() == Some(LinuxLibcVariant::Musl) {
                cargo.configure_tool_executable(
                    crate::cargo::CompilationTool::BinaryLinker,
                    "rust-lld",
                );
            } else {
                let sysroot = self.linux_sysroot();
                if sysroot.exists() {
                    cargo.attach_linux_sysroot(&sysroot, target)?;
                }
            }
        }
        cargo.append_linker_arg("-Wl,-rpath,$ORIGIN/lib");
        cargo.append_linker_arg("-Wl,-z,origin");
        Ok(())
    }

    fn configure_android_cargo_invocation(
        &self,
        cargo: &mut CargoCompilationEnvironment,
    ) -> Result<()> {
        let ndk_root = self.resolve_android_ndk();
        let min_api = self.android_min_sdk_version();
        cargo.attach_android_ndk_sysroot(&ndk_root, min_api)?;
        Ok(())
    }

    fn configure_windows_cargo_invocation(
        &self,
        cargo: &mut CargoCompilationEnvironment,
        target: TargetCompilationUnit,
    ) -> Result<()> {
        let win_sdk = self.resolve_windows_sdk();
        if win_sdk.exists() {
            cargo.attach_windows_sdk_sysroot(&win_sdk, target)?;
        }
        Ok(())
    }

    fn configure_macos_cargo_invocation(
        &self,
        cargo: &mut CargoCompilationEnvironment,
        target: TargetCompilationUnit,
    ) -> Result<()> {
        let mac_sdk = self.resolve_macos_sdk();
        if mac_sdk.exists() {
            let min_sys = self
                .manifest_config()
                .macos_platform_config()
                .minimum_system_version();
            cargo.attach_macos_sdk_sysroot(&mac_sdk, target, min_sys)?;
        } else {
            cargo.append_linker_arg("-rpath");
            cargo.append_linker_arg("@executable_path/../Frameworks");
        }
        Ok(())
    }

    fn configure_ios_cargo_invocation(
        &self,
        cargo: &mut CargoCompilationEnvironment,
        target: TargetCompilationUnit,
    ) -> Result<()> {
        let sdk_path = match target.target_apple_env() {
            Some(AppleTargetEnv::Simulator) => self.resolve_ios_simulator_sdk(),
            _ => self.resolve_ios_sdk(),
        };
        if sdk_path.exists() {
            let min_ios = self
                .manifest_config()
                .ios_platform_config()
                .minimum_os_version();
            cargo.attach_ios_sdk_sysroot(&sdk_path, target, min_ios)?;
        }
        Ok(())
    }

    pub fn resolve_cargo_artifact(
        &self,
        target_dir: &Path,
        target: TargetCompilationUnit,
        crate_kind: CompilationCrateKind,
    ) -> Result<PathBuf> {
        self.cargo
            .resolve_compiled_artifact_path(target_dir, target, None, crate_kind)
    }
}

#[cfg(test)]
pub static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::config::{CacheScope, ManifestConfig};
    use tempfile::tempdir;

    #[test]
    fn test_cache_resolution_cascade_exhaustive() {
        let _guard = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_cache = std::env::var("CRORUST_CACHE_DIR").ok();
        let prev_scope = std::env::var("CRORUST_CACHE_SCOPE").ok();
        std::env::remove_var("CRORUST_CACHE_DIR");
        std::env::remove_var("CRORUST_CACHE_SCOPE");

        let temp = tempdir().unwrap();
        let project_root = temp.path();

        let make_args = |cache_dir: Option<PathBuf>, project_cache: bool, global_cache: bool| {
            CargoPipelineArguments {
                package: None,
                bin: None,
                project: None,
                manifest_path: None,
                target_dir: None,
                offline: false,
                features: Vec::new(),
                no_default_features: false,
                project_cache,
                global_cache,
                cache_dir,
                skip_artifact_verification: false,
            }
        };

        let config = ManifestConfig::default();
        let args = make_args(None, false, false);
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &config, &args),
            project_root.join(".cache").join("crorust")
        );

        let mut cfg_global = ManifestConfig::default();
        cfg_global.toolchain.cache_scope = Some(CacheScope::Global);
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &cfg_global, &args),
            SessionExecutionContext::global_cache_dir()
        );

        let mut cfg_dir = cfg_global.clone();
        cfg_dir.toolchain.cache_dir = Some(PathBuf::from("custom_manifest_cache"));
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &cfg_dir, &args),
            project_root.join("custom_manifest_cache")
        );

        std::env::set_var("CRORUST_CACHE_SCOPE", "project");
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &cfg_dir, &args),
            project_root.join(".cache").join("crorust")
        );
        std::env::remove_var("CRORUST_CACHE_SCOPE");

        let env_cache = temp.path().join("env_cache");
        std::env::set_var("CRORUST_CACHE_DIR", env_cache.to_str().unwrap());
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &cfg_dir, &args),
            env_cache
        );
        std::env::remove_var("CRORUST_CACHE_DIR");

        let args_global = make_args(None, false, true);
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &cfg_dir, &args_global),
            SessionExecutionContext::global_cache_dir()
        );

        let cli_override = temp.path().join("cli_cache");
        let args_cli = make_args(Some(cli_override.clone()), false, true);
        assert_eq!(
            SessionExecutionContext::resolve_cache_dir(project_root, &cfg_dir, &args_cli),
            cli_override
        );

        if let Some(val) = prev_cache {
            std::env::set_var("CRORUST_CACHE_DIR", val);
        }
        if let Some(val) = prev_scope {
            std::env::set_var("CRORUST_CACHE_SCOPE", val);
        }
    }
}
