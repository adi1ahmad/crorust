use crate::cargo::manifest::{CargoPackageManifest, CargoPackageSection, WorkspaceInheritable};
use crate::core::target::{TargetOpt, TargetPlatform};
use anyhow::{Context, Result};
use crorust_android::manifest::{
    ActivityComponentDescriptor, AndroidIntentFilterDescriptor, AndroidMetaDataDescriptor,
    AndroidXmlManifestDescriptor,
};
use crorust_android::AndroidVersionCode;
use crorust_apple::DarwinPropertyList;
use crorust_common::ArchiveEntryOptions;
use crorust_windows::WindowsPackageManifestDocument;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_GRADLE_VERSION: &str = "8.7";
pub const DEFAULT_JVM_ARGS: &str = "-Xmx2048m -Dfile.encoding=UTF-8";
pub const DEFAULT_AGP_VERSION: &str = "8.7.3";
pub const DEFAULT_KOTLIN_VERSION: &str = "2.1.0";
pub const DEFAULT_JVM_TARGET: &str = "17";
pub const DEFAULT_MACOS_MIN: &str = "11.0";
pub const DEFAULT_IOS_MIN: &str = "15.0";
pub const DEFAULT_WIN_SDK: &str = "10.0.22621.0";

pub const DEFAULT_ANDROID_CONFIG_CHANGES: &[&str] = &[
    "density",
    "fontScale",
    "keyboard",
    "keyboardHidden",
    "layoutDirection",
    "locale",
    "orientation",
    "screenLayout",
    "screenSize",
    "smallestScreenSize",
    "uiMode",
];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestConfig {
    pub package: Option<String>,
    pub name: Option<String>,
    pub bin: Option<String>,
    pub version: Option<String>,
    pub version_code: Option<u32>,
    pub icon: Option<PathBuf>,

    #[serde(default)]
    pub toolchain: ToolchainConfig,

    #[serde(default)]
    pub android: AndroidPlatformConfig,

    #[serde(default)]
    pub ios: IosPlatformConfig,

    #[serde(default)]
    pub macos: MacosPlatformConfig,

    #[serde(default)]
    pub linux: LinuxPlatformConfig,

    #[serde(default)]
    pub windows: WindowsPlatformConfig,

    #[serde(default)]
    pub web: WebPlatformConfig,
}

impl ManifestConfig {
    pub fn parse_project(project_root: &Path, cargo_manifest_path: &Path) -> Result<Self> {
        let manifest_path = project_root.join("manifest.toml");
        if manifest_path.is_file() {
            return Self::parse_file(&manifest_path);
        }

        if cargo_manifest_path.is_file() {
            if let Some(config) = Self::parse_cargo_metadata(cargo_manifest_path)? {
                return Ok(config);
            }
        }

        Ok(Default::default())
    }

    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        if !p.exists() {
            return Ok(Default::default());
        }
        let raw = std::fs::read_to_string(p)
            .with_context(|| format!("Failed to read manifest file: {}", p.display()))?;
        let config: ManifestConfig = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse TOML manifest schema: {}", p.display()))?;
        Ok(config)
    }

    pub fn parse_cargo_metadata(cargo_toml_path: &Path) -> Result<Option<Self>> {
        let text = std::fs::read_to_string(cargo_toml_path).with_context(|| {
            format!(
                "Failed to read Cargo.toml at `{}`",
                cargo_toml_path.display()
            )
        })?;
        let val: toml::Value = toml::from_str(&text).with_context(|| {
            format!(
                "Failed to parse Cargo.toml at `{}`",
                cargo_toml_path.display()
            )
        })?;

        if let Some(meta) = val
            .get("package")
            .and_then(|p| p.get("metadata"))
            .and_then(|m| m.get("crorust"))
        {
            let config: ManifestConfig = meta.clone().try_into().with_context(|| {
                format!(
                    "Failed to deserialize [package.metadata.crorust] in `{}`",
                    cargo_toml_path.display()
                )
            })?;
            return Ok(Some(config));
        }

        Ok(None)
    }

    pub fn package_id(&self) -> Option<&str> {
        self.package.as_deref()
    }

    pub fn app_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn bin(&self) -> Option<&str> {
        self.bin.as_deref()
    }

    pub fn app_version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn app_version_code(&self) -> Option<u32> {
        self.version_code
    }

    pub fn app_icon(&self, _platform: TargetPlatform) -> Option<&Path> {
        self.icon.as_deref()
    }

    pub fn external_runtime_libs(&self, _platform: TargetPlatform) -> Vec<PathBuf> {
        Vec::new()
    }

    pub fn toolchain(&self) -> &ToolchainConfig {
        &self.toolchain
    }

    pub fn android(&self) -> &AndroidPlatformConfig {
        &self.android
    }

    pub fn android_platform_config(&self) -> &AndroidPlatformConfig {
        &self.android
    }

    pub fn android_platform_config_mut(&mut self) -> &mut AndroidPlatformConfig {
        &mut self.android
    }

    pub fn ios(&self) -> &IosPlatformConfig {
        &self.ios
    }

    pub fn ios_platform_config(&self) -> &IosPlatformConfig {
        &self.ios
    }

    pub fn macos(&self) -> &MacosPlatformConfig {
        &self.macos
    }

    pub fn macos_platform_config(&self) -> &MacosPlatformConfig {
        &self.macos
    }

    pub fn linux(&self) -> &LinuxPlatformConfig {
        &self.linux
    }

    pub fn linux_platform_config(&self) -> &LinuxPlatformConfig {
        &self.linux
    }

    pub fn windows(&self) -> &WindowsPlatformConfig {
        &self.windows
    }

    pub fn windows_platform_config(&self) -> &WindowsPlatformConfig {
        &self.windows
    }

    pub fn web(&self) -> &WebPlatformConfig {
        &self.web
    }

    pub fn web_platform_config(&self) -> &WebPlatformConfig {
        &self.web
    }

    pub fn merge(&mut self, incoming: ManifestConfig) {
        if incoming.package.is_some() {
            self.package = incoming.package;
        }
        if incoming.name.is_some() {
            self.name = incoming.name;
        }
        if incoming.bin.is_some() {
            self.bin = incoming.bin;
        }
        if incoming.version.is_some() {
            self.version = incoming.version;
        }
        if incoming.version_code.is_some() {
            self.version_code = incoming.version_code;
        }
        if incoming.icon.is_some() {
            self.icon = incoming.icon;
        }

        if incoming.toolchain.offline.is_some() {
            self.toolchain.offline = incoming.toolchain.offline;
        }
        if incoming.toolchain.mirror_url.is_some() {
            self.toolchain.mirror_url = incoming.toolchain.mirror_url;
        }
        if incoming.toolchain.nix.is_some() {
            self.toolchain.nix = incoming.toolchain.nix;
        }
        if incoming.toolchain.cache_scope.is_some() {
            self.toolchain.cache_scope = incoming.toolchain.cache_scope;
        }
        if incoming.toolchain.cache_dir.is_some() {
            self.toolchain.cache_dir = incoming.toolchain.cache_dir;
        }
        if let Some(incoming_android) = incoming.toolchain.android {
            let android = self.toolchain.android.get_or_insert_with(Default::default);
            for (k, v) in incoming_android.platform {
                android.platform.insert(k, v);
            }
            if incoming_android.ndk.is_some() {
                android.ndk = incoming_android.ndk;
            }
        }
        if let Some(incoming_windows) = incoming.toolchain.windows {
            let windows = self.toolchain.windows.get_or_insert_with(Default::default);
            if incoming_windows.sdk.is_some() {
                windows.sdk = incoming_windows.sdk;
            }
        }
        if let Some(incoming_apple) = incoming.toolchain.apple {
            let apple = self.toolchain.apple.get_or_insert_with(Default::default);
            if incoming_apple.macos_sdk.is_some() {
                apple.macos_sdk = incoming_apple.macos_sdk;
            }
            if incoming_apple.ios_sdk.is_some() {
                apple.ios_sdk = incoming_apple.ios_sdk;
            }
            if incoming_apple.ios_simulator_sdk.is_some() {
                apple.ios_simulator_sdk = incoming_apple.ios_simulator_sdk;
            }
        }

        if incoming.android.min_sdk.is_some() {
            self.android.min_sdk = incoming.android.min_sdk;
        }
        if incoming.android.target_sdk.is_some() {
            self.android.target_sdk = incoming.android.target_sdk;
        }
        if incoming.android.compile_sdk.is_some() {
            self.android.compile_sdk = incoming.android.compile_sdk;
        }
        if incoming.android.ndk_version.is_some() {
            self.android.ndk_version = incoming.android.ndk_version;
        }
        if incoming.android.sdk.is_some() {
            self.android.sdk = incoming.android.sdk;
        }
        if incoming.android.ndk.is_some() {
            self.android.ndk = incoming.android.ndk;
        }
        if incoming.android.use_gradle.is_some() {
            self.android.use_gradle = incoming.android.use_gradle;
        }
        if incoming.android.gradle.is_some() {
            self.android.gradle = incoming.android.gradle;
        }
        if incoming.android.agp_version.is_some() {
            self.android.agp_version = incoming.android.agp_version;
        }
        if incoming.android.kotlin_version.is_some() {
            self.android.kotlin_version = incoming.android.kotlin_version;
        }
        if incoming.android.jvm_target.is_some() {
            self.android.jvm_target = incoming.android.jvm_target;
        }
        if !incoming.android.repositories.is_empty() {
            self.android.repositories = incoming.android.repositories;
        }
        if !incoming.android.dependencies.is_empty() {
            self.android.dependencies = incoming.android.dependencies;
        }
        if incoming.android.game_activity.is_some() {
            self.android.game_activity = incoming.android.game_activity;
        }
        if !incoming.android.assets.is_empty() {
            self.android.assets = incoming.android.assets;
        }

        let incoming_m = incoming.android.manifest;
        let local_m = &mut self.android.manifest;
        if incoming_m.package.is_some() {
            local_m.package = incoming_m.package;
        }
        if incoming_m.version_code.is_some() {
            local_m.version_code = incoming_m.version_code;
        }
        if incoming_m.version_name.is_some() {
            local_m.version_name = incoming_m.version_name;
        }
        if incoming_m.compile_sdk_version.is_some() {
            local_m.compile_sdk_version = incoming_m.compile_sdk_version;
        }
        if incoming_m.sdk.min_sdk_version.is_some() {
            local_m.sdk.min_sdk_version = incoming_m.sdk.min_sdk_version;
        }
        if incoming_m.sdk.target_sdk_version.is_some() {
            local_m.sdk.target_sdk_version = incoming_m.sdk.target_sdk_version;
        }
        if incoming_m.sdk.max_sdk_version.is_some() {
            local_m.sdk.max_sdk_version = incoming_m.sdk.max_sdk_version;
        }

        for perm in incoming_m.uses_permission {
            if !local_m.uses_permission.iter().any(|p| p.name == perm.name) {
                local_m.uses_permission.push(perm);
            }
        }
        for feat in incoming_m.uses_feature {
            local_m.uses_feature.push(feat);
        }
        if incoming_m.application.debuggable.is_some() {
            local_m.application.debuggable = incoming_m.application.debuggable;
        }
        if incoming_m.application.theme.is_some() {
            local_m.application.theme = incoming_m.application.theme;
        }
        if incoming_m.application.has_code.is_some() {
            local_m.application.has_code = incoming_m.application.has_code;
        }
        if incoming_m.application.icon.is_some() {
            local_m.application.icon = incoming_m.application.icon;
        }
        if incoming_m.application.label.is_some() {
            local_m.application.label = incoming_m.application.label;
        }
        if incoming_m.application.extract_native_libs.is_some() {
            local_m.application.extract_native_libs = incoming_m.application.extract_native_libs;
        }
        if !incoming_m.application.activities.is_empty() {
            local_m.application.activities = incoming_m.application.activities;
        }
        if !incoming_m.application.providers.is_empty() {
            local_m.application.providers = incoming_m.application.providers;
        }
        if !incoming_m.application.meta_data.is_empty() {
            local_m.application.meta_data = incoming_m.application.meta_data;
        }

        for (k, v) in incoming.android.debug.reverse {
            self.android.debug.reverse.insert(k, v);
        }
        for (k, v) in incoming.android.debug.forward {
            self.android.debug.forward.insert(k, v);
        }

        if incoming.ios.simulator.is_some() {
            self.ios.simulator = incoming.ios.simulator;
        }
        if incoming.ios.sdk.is_some() {
            self.ios.sdk = incoming.ios.sdk;
        }
        if incoming.ios.sdk_version.is_some() {
            self.ios.sdk_version = incoming.ios.sdk_version;
        }
        if incoming.ios.minimum_os_version.is_some() {
            self.ios.minimum_os_version = incoming.ios.minimum_os_version;
        }
        if incoming.ios.bundle_identifier.is_some() {
            self.ios.bundle_identifier = incoming.ios.bundle_identifier;
        }
        if incoming.ios.assets_car.is_some() {
            self.ios.assets_car = incoming.ios.assets_car;
        }
        if incoming.ios.info.bundle_identifier.is_some() {
            self.ios.info.bundle_identifier = incoming.ios.info.bundle_identifier;
        }
        if incoming.ios.info.bundle_name.is_some() {
            self.ios.info.bundle_name = incoming.ios.info.bundle_name;
        }
        if incoming.ios.info.display_name.is_some() {
            self.ios.info.display_name = incoming.ios.info.display_name;
        }
        if incoming.ios.info.executable_name.is_some() {
            self.ios.info.executable_name = incoming.ios.info.executable_name;
        }
        if incoming.ios.info.package_type.is_some() {
            self.ios.info.package_type = incoming.ios.info.package_type;
        }
        if incoming.ios.info.short_version_string.is_some() {
            self.ios.info.short_version_string = incoming.ios.info.short_version_string;
        }
        if incoming.ios.info.bundle_version.is_some() {
            self.ios.info.bundle_version = incoming.ios.info.bundle_version;
        }
        if incoming.ios.info.icon_file.is_some() {
            self.ios.info.icon_file = incoming.ios.info.icon_file;
        }
        if !incoming.ios.info.icon_files.is_empty() {
            self.ios.info.icon_files = incoming.ios.info.icon_files;
        }
        if !incoming.ios.info.supported_platforms.is_empty() {
            self.ios.info.supported_platforms = incoming.ios.info.supported_platforms;
        }
        if incoming.ios.info.development_region.is_some() {
            self.ios.info.development_region = incoming.ios.info.development_region;
        }
        if incoming.ios.info.min_ios_version.is_some() {
            self.ios.info.min_ios_version = incoming.ios.info.min_ios_version;
        }
        if incoming.ios.info.requires_ios.is_some() {
            self.ios.info.requires_ios = incoming.ios.info.requires_ios;
        }
        for (k, v) in incoming.ios.info.custom_keys {
            self.ios.info.custom_keys.insert(k, v);
        }

        if incoming.macos.sdk.is_some() {
            self.macos.sdk = incoming.macos.sdk;
        }
        if incoming.macos.sdk_version.is_some() {
            self.macos.sdk_version = incoming.macos.sdk_version;
        }
        if incoming.macos.minimum_system_version.is_some() {
            self.macos.minimum_system_version = incoming.macos.minimum_system_version;
        }
        if incoming.macos.bundle_identifier.is_some() {
            self.macos.bundle_identifier = incoming.macos.bundle_identifier;
        }
        if incoming.macos.info.bundle_identifier.is_some() {
            self.macos.info.bundle_identifier = incoming.macos.info.bundle_identifier;
        }
        if incoming.macos.info.bundle_name.is_some() {
            self.macos.info.bundle_name = incoming.macos.info.bundle_name;
        }
        if incoming.macos.info.display_name.is_some() {
            self.macos.info.display_name = incoming.macos.info.display_name;
        }
        if incoming.macos.info.executable_name.is_some() {
            self.macos.info.executable_name = incoming.macos.info.executable_name;
        }
        if incoming.macos.info.package_type.is_some() {
            self.macos.info.package_type = incoming.macos.info.package_type;
        }
        if incoming.macos.info.short_version_string.is_some() {
            self.macos.info.short_version_string = incoming.macos.info.short_version_string;
        }
        if incoming.macos.info.bundle_version.is_some() {
            self.macos.info.bundle_version = incoming.macos.info.bundle_version;
        }
        if incoming.macos.info.icon_file.is_some() {
            self.macos.info.icon_file = incoming.macos.info.icon_file;
        }
        if !incoming.macos.info.icon_files.is_empty() {
            self.macos.info.icon_files = incoming.macos.info.icon_files;
        }
        if !incoming.macos.info.supported_platforms.is_empty() {
            self.macos.info.supported_platforms = incoming.macos.info.supported_platforms;
        }
        if incoming.macos.info.development_region.is_some() {
            self.macos.info.development_region = incoming.macos.info.development_region;
        }
        if incoming.macos.info.min_macos_version.is_some() {
            self.macos.info.min_macos_version = incoming.macos.info.min_macos_version;
        }
        if incoming.macos.info.high_resolution_capable.is_some() {
            self.macos.info.high_resolution_capable = incoming.macos.info.high_resolution_capable;
        }
        for (k, v) in incoming.macos.info.custom_keys {
            self.macos.info.custom_keys.insert(k, v);
        }

        if incoming.linux.libc.is_some() {
            self.linux.libc = incoming.linux.libc;
        }
        if incoming.linux.squashfs_backend.is_some() {
            self.linux.squashfs_backend = incoming.linux.squashfs_backend;
        }
        if incoming.linux.compression.is_some() {
            self.linux.compression = incoming.linux.compression;
        }

        if incoming.windows.sdk.is_some() {
            self.windows.sdk = incoming.windows.sdk;
        }
        if incoming.windows.sdk_version.is_some() {
            self.windows.sdk_version = incoming.windows.sdk_version;
        }
        if incoming.windows.publisher.is_some() {
            self.windows.publisher = incoming.windows.publisher;
        }
        if incoming.windows.manifest.identity.name.is_some() {
            self.windows.manifest.identity = incoming.windows.manifest.identity;
        }
        if incoming.windows.manifest.properties.display_name.is_some() {
            self.windows.manifest.properties = incoming.windows.manifest.properties;
        }
        if !incoming.windows.manifest.resources.resource.is_empty() {
            self.windows.manifest.resources = incoming.windows.manifest.resources;
        }
        if !incoming
            .windows
            .manifest
            .dependencies
            .target_device_family
            .is_empty()
        {
            self.windows.manifest.dependencies = incoming.windows.manifest.dependencies;
        }
        if !incoming.windows.manifest.capabilities.is_empty() {
            self.windows.manifest.capabilities = incoming.windows.manifest.capabilities;
        }
        if !incoming
            .windows
            .manifest
            .applications
            .application
            .is_empty()
        {
            self.windows.manifest.applications = incoming.windows.manifest.applications;
        }

        if incoming.web.port.is_some() {
            self.web.port = incoming.web.port;
        }
        if incoming.web.open.is_some() {
            self.web.open = incoming.web.open;
        }
        if incoming.web.dist.is_some() {
            self.web.dist = incoming.web.dist;
        }
        if incoming.web.address.is_some() {
            self.web.address = incoming.web.address;
        }
        if incoming.web.watch.is_some() {
            self.web.watch = incoming.web.watch;
        }
        if incoming.web.watch_ignore.is_some() {
            self.web.watch_ignore = incoming.web.watch_ignore;
        }
    }

    pub fn harmonize_with_cargo_package(
        &mut self,
        manifest_package: &CargoPackageSection,
        workspace_manifest: Option<&CargoPackageManifest>,
        opt: TargetOpt,
    ) -> Result<()> {
        let is_game = self.android.game_activity();

        if is_game && self.android.gradle.is_none() {
            self.android.gradle = Some(GradleConfig::default());
        }
        if is_game {
            let game_act = "androidx.games:games-activity:3.0.5".to_string();
            if !self.android.dependencies.contains(&game_act) {
                self.android.dependencies.push(game_act);
            }
            let appcompat = "androidx.appcompat:appcompat:1.7.0".to_string();
            if !self.android.dependencies.contains(&appcompat) {
                self.android.dependencies.push(appcompat);
            }
        }

        let pkg_version = match &manifest_package.version {
            WorkspaceInheritable::Value(v) => v.clone(),
            WorkspaceInheritable::Inherited { workspace: true } => {
                let ws_pkg = workspace_manifest
                    .and_then(|m| m.workspace.as_ref())
                    .and_then(|w| w.package.as_ref())
                    .context("Root workspace manifest lacks `[workspace.package]` table")?;
                ws_pkg
                    .version
                    .clone()
                    .context("Missing `workspace.package.version` in root manifest")?
            }
            WorkspaceInheritable::Inherited { workspace: false } => {
                anyhow::bail!("Unscoped workspace inheritance not allowed")
            }
        };

        let pkg_desc = match &manifest_package.description {
            Some(WorkspaceInheritable::Value(d)) => d.clone(),
            Some(WorkspaceInheritable::Inherited { workspace: true }) => {
                let ws_pkg = workspace_manifest
                    .and_then(|m| m.workspace.as_ref())
                    .and_then(|w| w.package.as_ref())
                    .context("Root workspace manifest lacks `[workspace.package]` table")?;
                ws_pkg
                    .description
                    .clone()
                    .context("Missing `workspace.package.description` in root manifest")?
            }
            Some(WorkspaceInheritable::Inherited { workspace: false }) => {
                anyhow::bail!("Unscoped workspace inheritance not allowed")
            }
            None => String::new(),
        };

        self.harmonize_android_manifest(manifest_package, &pkg_version, opt, is_game);

        self.harmonize_ios_plist(manifest_package, &pkg_version);

        self.harmonize_macos_plist(manifest_package, &pkg_version);

        self.harmonize_windows_manifest(manifest_package, &pkg_version, &pkg_desc);

        Ok(())
    }

    fn harmonize_android_manifest(
        &mut self,
        pkg: &CargoPackageSection,
        version: &str,
        opt: TargetOpt,
        is_game: bool,
    ) {
        let manifest = &mut self.android.manifest;
        manifest
            .package
            .get_or_insert_with(|| format!("com.example.{}", pkg.name.replace('-', "_")));

        manifest
            .version_name
            .get_or_insert_with(|| version.to_string());
        if let Ok(parsed_code) = AndroidVersionCode::from_semver(version) {
            manifest
                .version_code
                .get_or_insert_with(|| parsed_code.to_code(1));
        }

        let min_api = self.android.min_sdk.unwrap_or(21);
        let target_api = self.android.target_sdk.unwrap_or(34);
        let compile_api = self.android.compile_sdk.unwrap_or(35.max(target_api));

        manifest.compile_sdk_version.get_or_insert(compile_api);
        manifest
            .platform_build_version_code
            .get_or_insert(compile_api);
        manifest.sdk.target_sdk_version.get_or_insert(target_api);
        manifest.sdk.min_sdk_version.get_or_insert(min_api);

        let app = &mut manifest.application;
        app.label.get_or_insert_with(|| pkg.name.clone());
        if is_game {
            app.theme
                .get_or_insert_with(|| "@style/Theme.AppCompat.NoActionBar".into());
        }
        app.debuggable
            .get_or_insert_with(|| opt == TargetOpt::Debug);
        app.has_code.get_or_insert(is_game);

        if app.activities.is_empty() {
            app.activities.push(ActivityComponentDescriptor::default());
        }
        let act = app.activities.get_mut(0).unwrap();
        act.config_changes
            .get_or_insert_with(|| DEFAULT_ANDROID_CONFIG_CHANGES.join("|"));
        act.launch_mode.get_or_insert_with(|| "singleTop".into());
        act.name.get_or_insert_with(|| {
            if is_game {
                String::from("com.google.androidgamesdk.GameActivity")
            } else {
                String::from("android.app.NativeActivity")
            }
        });
        act.window_soft_input_mode
            .get_or_insert_with(|| "adjustResize".into());
        act.hardware_accelerated.get_or_insert(true);
        act.exported.get_or_insert(true);

        if !act
            .meta_data
            .iter()
            .any(|m| m.name.as_deref() == Some("android.app.lib_name"))
        {
            act.meta_data.push(AndroidMetaDataDescriptor {
                name: Some("android.app.lib_name".to_string()),
                value: Some(pkg.name.replace('-', "_")),
                resource: None,
            });
        }
        if act.intent_filters.is_empty() {
            act.intent_filters
                .push(AndroidIntentFilterDescriptor::launcher());
        }
    }

    fn harmonize_ios_plist(&mut self, pkg: &CargoPackageSection, version: &str) {
        let min_ver = self.ios.minimum_os_version().to_string();
        let info = &mut self.ios.info;
        if info.bundle_identifier.is_none() {
            info.bundle_identifier = Some(
                self.ios
                    .bundle_identifier
                    .clone()
                    .unwrap_or_else(|| pkg.name.clone()),
            );
        }
        if info.bundle_name.is_none() {
            info.bundle_name = Some(pkg.name.clone());
        }
        if info.package_type.is_none() {
            info.package_type = Some("APPL".into());
        }
        if info.short_version_string.is_none() {
            info.short_version_string = Some(version.to_string());
        }
        if info.bundle_version.is_none() {
            info.bundle_version = Some(version.to_string());
        }
        if info.supported_platforms.is_empty() {
            info.supported_platforms.push("iPhoneOS".into());
        }

        info.set_default("DTCompiler", "com.apple.compilers.llvm.clang.1_0");
        info.set_default("DTPlatformBuild", "19C51");
        info.set_default("DTPlatformName", "iphoneos");
        info.set_default("DTPlatformVersion", "15.2");
        info.set_default("DTSDKBuild", "19C51");
        info.set_default("DTSDKName", "iphoneos15.2");
        info.set_default("DTXcode", "1321");
        info.set_default("DTXcodeBuild", "13C100");

        if info.requires_ios.is_none() {
            info.requires_ios = Some(true);
        }
        if info.min_ios_version.is_none() {
            info.min_ios_version = Some(min_ver);
        }
        info.set_default(
            "UIDeviceFamily",
            plist::Value::Array(vec![
                plist::Value::Integer(1.into()),
                plist::Value::Integer(2.into()),
            ]),
        );
        info.set_default("UILaunchStoryboardName", "");
        info.set_default(
            "UIRequiredDeviceCapabilities",
            plist::Value::Array(vec![plist::Value::String("arm64".into())]),
        );

        info.set_default(
            "UISupportedInterfaceOrientations~ipad",
            plist::Value::Array(vec![
                plist::Value::String("UIInterfaceOrientationPortrait".into()),
                plist::Value::String("UIInterfaceOrientationPortraitUpsideDown".into()),
                plist::Value::String("UIInterfaceOrientationLandscapeLeft".into()),
                plist::Value::String("UIInterfaceOrientationLandscapeRight".into()),
            ]),
        );
        info.set_default(
            "UISupportedInterfaceOrientations~iphone",
            plist::Value::Array(vec![
                plist::Value::String("UIInterfaceOrientationPortrait".into()),
                plist::Value::String("UIInterfaceOrientationLandscapeLeft".into()),
                plist::Value::String("UIInterfaceOrientationLandscapeRight".into()),
            ]),
        );
    }

    fn harmonize_macos_plist(&mut self, pkg: &CargoPackageSection, version: &str) {
        let min_ver = self.macos.minimum_system_version().to_string();
        let info = &mut self.macos.info;
        if info.bundle_identifier.is_none() {
            info.bundle_identifier = Some(
                self.macos
                    .bundle_identifier
                    .clone()
                    .unwrap_or_else(|| pkg.name.clone()),
            );
        }
        if info.bundle_name.is_none() {
            info.bundle_name = Some(pkg.name.clone());
        }
        if info.short_version_string.is_none() {
            info.short_version_string = Some(version.to_string());
        }
        if info.bundle_version.is_none() {
            info.bundle_version = Some(version.to_string());
        }
        if info.package_type.is_none() {
            info.package_type = Some("APPL".into());
        }
        if info.min_macos_version.is_none() {
            info.min_macos_version = Some(min_ver);
        }
    }

    fn harmonize_windows_manifest(
        &mut self,
        pkg: &CargoPackageSection,
        version: &str,
        description: &str,
    ) {
        self.windows
            .manifest
            .properties
            .display_name
            .get_or_insert_with(|| pkg.name.clone());
        self.windows
            .manifest
            .identity
            .version
            .get_or_insert_with(|| version.to_string());
        self.windows
            .manifest
            .properties
            .description
            .get_or_insert_with(|| description.to_string());
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    #[default]
    Project,
    Global,
}

impl std::str::FromStr for CacheScope {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "project" | "local" => Ok(Self::Project),
            "global" | "shared" => Ok(Self::Global),
            unknown => anyhow::bail!(
                "Invalid cache scope `{unknown}`. Supported values: `project` (default), `global`."
            ),
        }
    }
}

impl std::fmt::Display for CacheScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => write!(f, "project"),
            Self::Global => write!(f, "global"),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolchainConfig {
    pub offline: Option<bool>,
    pub mirror_url: Option<String>,
    pub nix: Option<bool>,
    pub cache_scope: Option<CacheScope>,
    pub cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub android: Option<ToolchainAndroidConfig>,
    #[serde(default)]
    pub windows: Option<ToolchainWindowsConfig>,
    #[serde(default)]
    pub apple: Option<ToolchainAppleConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolchainAndroidConfig {
    #[serde(default)]
    pub platform: std::collections::BTreeMap<String, CustomPlatformArtifactConfig>,
    pub ndk: Option<CustomPlatformArtifactConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolchainWindowsConfig {
    pub sdk: Option<CustomPlatformArtifactConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolchainAppleConfig {
    pub macos_sdk: Option<CustomPlatformArtifactConfig>,
    pub ios_sdk: Option<CustomPlatformArtifactConfig>,
    pub ios_simulator_sdk: Option<CustomPlatformArtifactConfig>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CustomPlatformArtifactConfig {
    pub url: String,
    pub sha256: Option<String>,
}

impl ToolchainConfig {
    pub fn is_offline_enforced(&self) -> bool {
        self.offline.unwrap_or(false)
    }

    pub fn offline(&self) -> bool {
        self.is_offline_enforced()
    }

    pub fn cache_scope(&self) -> CacheScope {
        self.cache_scope.unwrap_or_default()
    }

    pub fn explicit_cache_dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }

    pub fn cache_dir(&self) -> Option<&Path> {
        self.explicit_cache_dir()
    }

    pub fn get_platform_override(&self, api_level: u32) -> Option<&CustomPlatformArtifactConfig> {
        self.android
            .as_ref()
            .and_then(|a| a.platform.get(&api_level.to_string()))
    }

    pub fn get_android_ndk_override(&self) -> Option<&CustomPlatformArtifactConfig> {
        self.android.as_ref().and_then(|a| a.ndk.as_ref())
    }

    pub fn get_windows_sdk_override(&self) -> Option<&CustomPlatformArtifactConfig> {
        self.windows.as_ref().and_then(|w| w.sdk.as_ref())
    }

    pub fn get_macos_sdk_override(&self) -> Option<&CustomPlatformArtifactConfig> {
        self.apple.as_ref().and_then(|a| a.macos_sdk.as_ref())
    }

    pub fn get_ios_sdk_override(&self) -> Option<&CustomPlatformArtifactConfig> {
        self.apple.as_ref().and_then(|a| a.ios_sdk.as_ref())
    }

    pub fn get_ios_simulator_sdk_override(&self) -> Option<&CustomPlatformArtifactConfig> {
        self.apple
            .as_ref()
            .and_then(|a| a.ios_simulator_sdk.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GradleConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default = "default_gradle_version")]
    pub version: String,
    pub distribution_url: Option<String>,
    #[serde(default = "default_jvm_args")]
    pub jvm_args: String,
}

impl Default for GradleConfig {
    fn default() -> Self {
        Self {
            enabled: None,
            version: default_gradle_version(),
            distribution_url: None,
            jvm_args: default_jvm_args(),
        }
    }
}

fn default_gradle_version() -> String {
    DEFAULT_GRADLE_VERSION.to_string()
}
fn default_jvm_args() -> String {
    DEFAULT_JVM_ARGS.to_string()
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidDebugIdentityConfig {
    #[serde(default)]
    pub reverse: HashMap<String, String>,
    #[serde(default)]
    pub forward: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidPlatformConfig {
    pub min_sdk: Option<u32>,
    pub target_sdk: Option<u32>,
    pub compile_sdk: Option<u32>,
    pub ndk_version: Option<String>,
    pub sdk: Option<PathBuf>,
    pub ndk: Option<PathBuf>,

    #[serde(default)]
    pub use_gradle: Option<bool>,

    #[serde(default)]
    pub gradle: Option<GradleConfig>,

    pub agp_version: Option<String>,
    pub kotlin_version: Option<String>,
    pub jvm_target: Option<String>,

    #[serde(default)]
    pub repositories: Vec<RepositoryEntry>,

    #[serde(default)]
    pub dependencies: Vec<String>,

    pub game_activity: Option<bool>,

    #[serde(default)]
    pub assets: Vec<PackagingAssetMapping>,

    #[serde(default)]
    pub debug: AndroidDebugIdentityConfig,

    #[serde(default)]
    pub manifest: AndroidXmlManifestDescriptor,
}

impl AndroidPlatformConfig {
    pub fn use_gradle(&self) -> bool {
        if let Some(explicit) = self.use_gradle {
            return explicit;
        }
        if let Some(g_cfg) = &self.gradle {
            if let Some(en) = g_cfg.enabled {
                return en;
            }
        }
        false
    }

    pub fn agp_version(&self) -> &str {
        self.agp_version.as_deref().unwrap_or(DEFAULT_AGP_VERSION)
    }

    pub fn kotlin_version(&self) -> &str {
        self.kotlin_version
            .as_deref()
            .unwrap_or(DEFAULT_KOTLIN_VERSION)
    }

    pub fn jvm_target(&self) -> &str {
        self.jvm_target.as_deref().unwrap_or(DEFAULT_JVM_TARGET)
    }

    pub fn game_activity(&self) -> bool {
        self.game_activity.unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum RepositoryEntry {
    Standard {
        standard: String,
    },
    Maven {
        maven: String,
        username_env: Option<String>,
        password_env: Option<String>,
    },
}

pub use crate::core::target::LinuxLibcVariant;
pub use crorust_linux::{SquashfsBackend, SquashfsCompression};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinuxPlatformConfig {
    pub libc: Option<LinuxLibcVariant>,
    pub squashfs_backend: Option<SquashfsBackend>,
    pub compression: Option<SquashfsCompression>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacosPlatformConfig {
    pub sdk: Option<PathBuf>,
    pub sdk_version: Option<String>,
    pub minimum_system_version: Option<String>,
    pub bundle_identifier: Option<String>,
    #[serde(default)]
    pub info: DarwinPropertyList,
}

impl MacosPlatformConfig {
    pub fn minimum_system_version(&self) -> &str {
        self.minimum_system_version
            .as_deref()
            .unwrap_or(DEFAULT_MACOS_MIN)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IosPlatformConfig {
    pub sdk: Option<PathBuf>,
    pub sdk_version: Option<String>,
    pub minimum_os_version: Option<String>,
    pub bundle_identifier: Option<String>,
    pub simulator: Option<bool>,
    #[serde(default)]
    pub assets_car: Option<PathBuf>,
    #[serde(default)]
    pub info: DarwinPropertyList,
}

impl IosPlatformConfig {
    pub fn minimum_os_version(&self) -> &str {
        self.minimum_os_version
            .as_deref()
            .unwrap_or(DEFAULT_IOS_MIN)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsPlatformConfig {
    pub sdk_version: Option<String>,
    pub publisher: Option<String>,
    pub sdk: Option<PathBuf>,
    #[serde(default)]
    pub manifest: WindowsPackageManifestDocument,
}

impl WindowsPlatformConfig {
    pub fn sdk_version(&self) -> &str {
        self.sdk_version.as_deref().unwrap_or(DEFAULT_WIN_SDK)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageCompressionMode {
    Unaligned,
    #[default]
    Compressed,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ContainerEntryAlignment {
    Aligned(u16),
    Storage(StorageCompressionMode),
}

impl Default for ContainerEntryAlignment {
    fn default() -> Self {
        Self::Storage(StorageCompressionMode::Compressed)
    }
}

impl ContainerEntryAlignment {
    pub fn into_archive_entry_options(self) -> ArchiveEntryOptions {
        match self {
            Self::Aligned(b) => ArchiveEntryOptions::Aligned(b),
            Self::Storage(StorageCompressionMode::Unaligned) => ArchiveEntryOptions::Unaligned,
            Self::Storage(StorageCompressionMode::Compressed) => ArchiveEntryOptions::Compressed,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum PackagingAssetMapping {
    Direct(PathBuf),
    Configured {
        path: PathBuf,
        #[serde(default)]
        dest: Option<PathBuf>,
        #[serde(default)]
        optional: bool,
        #[serde(default)]
        alignment: ContainerEntryAlignment,
    },
}

impl PackagingAssetMapping {
    pub fn source_path(&self) -> &Path {
        match self {
            Self::Direct(p) => p,
            Self::Configured { path, .. } => path,
        }
    }

    pub fn destination_target(&self) -> Option<&Path> {
        match self {
            Self::Direct(_) => None,
            Self::Configured { dest, .. } => dest.as_deref(),
        }
    }

    pub fn is_optional_asset(&self) -> bool {
        match self {
            Self::Direct(_) => false,
            Self::Configured { optional, .. } => *optional,
        }
    }

    pub fn entry_alignment(&self) -> ContainerEntryAlignment {
        match self {
            Self::Direct(_) => Default::default(),
            Self::Configured { alignment, .. } => *alignment,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WebPlatformConfig {
    pub port: Option<u16>,
    pub open: Option<bool>,
    pub dist: Option<PathBuf>,
    pub address: Option<String>,
    pub watch: Option<Vec<PathBuf>>,
    pub watch_ignore: Option<Vec<PathBuf>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_config_parsing() {
        let toml_str = r#"
package = "test-app"

[web]
port = 3000
open = false
address = "127.0.0.1"
watch = ["src", "assets"]
watch_ignore = [".cache", "custom_dir"]
"#;
        let config: ManifestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.web.port, Some(3000));
        assert_eq!(config.web.open, Some(false));
        assert_eq!(config.web.address.as_deref(), Some("127.0.0.1"));
        assert_eq!(
            config.web.watch,
            Some(vec![PathBuf::from("src"), PathBuf::from("assets")])
        );
        assert_eq!(
            config.web.watch_ignore,
            Some(vec![PathBuf::from(".cache"), PathBuf::from("custom_dir")])
        );
    }

    #[test]
    fn test_web_config_deny_unknown_fields() {
        let toml_str = r#"
package = "test-app"

[web]
port = 3000
unknown_option = true
"#;
        let result: Result<ManifestConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_toolchain_cache_serde_and_merge() {
        let toml_proj = "[toolchain]\ncache_scope = \"project\"\n";
        let cfg_proj: ManifestConfig = toml::from_str(toml_proj).unwrap();
        assert_eq!(cfg_proj.toolchain.cache_scope(), CacheScope::Project);

        let toml_glob = "[toolchain]\ncache_scope = \"global\"\n";
        let cfg_glob: ManifestConfig = toml::from_str(toml_glob).unwrap();
        assert_eq!(cfg_glob.toolchain.cache_scope(), CacheScope::Global);

        let toml_bad = "[toolchain]\ncache_scope = \"unknown_scope\"\n";
        assert!(toml::from_str::<ManifestConfig>(toml_bad).is_err());

        let mut parent: ManifestConfig = toml::from_str(
            r#"
[toolchain]
offline = true
cache_scope = "global"
cache_dir = "/shared/cache"
"#,
        )
        .unwrap();

        let child: ManifestConfig = toml::from_str(
            r#"
[toolchain]
cache_dir = "/project/local/cache"
"#,
        )
        .unwrap();

        parent.merge(child);
        assert!(parent.toolchain.is_offline_enforced());
        assert_eq!(parent.toolchain.cache_scope(), CacheScope::Global);
        assert_eq!(
            parent.toolchain.explicit_cache_dir(),
            Some(Path::new("/project/local/cache"))
        );
    }

    #[test]
    fn test_toolchain_android_platform_overrides() {
        let toml_str = r#"
[toolchain.android.platform.35]
url = "https://dl.google.com/android/repository/platform-35_r02.zip"
sha256 = "0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0"

[toolchain.android.platform.36]
url = "https://dl.google.com/android/repository/platform-36_r01.zip"
"#;
        let cfg: ManifestConfig = toml::from_str(toml_str).unwrap();
        let p35 = cfg
            .toolchain
            .get_platform_override(35)
            .expect("Override 35 must exist");
        assert_eq!(
            p35.url,
            "https://dl.google.com/android/repository/platform-35_r02.zip"
        );
        assert_eq!(
            p35.sha256.as_deref(),
            Some("0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0")
        );

        let p36 = cfg
            .toolchain
            .get_platform_override(36)
            .expect("Override 36 must exist");
        assert_eq!(
            p36.url,
            "https://dl.google.com/android/repository/platform-36_r01.zip"
        );
        assert_eq!(p36.sha256, None);

        assert!(cfg.toolchain.get_platform_override(34).is_none());

        // Test merge
        let mut parent = ManifestConfig::default();
        parent.merge(cfg);
        assert!(parent.toolchain.get_platform_override(35).is_some());

        // Test deny_unknown_fields rejects invalid keys
        let bad_toml = r#"
[toolchain.android.platform.35]
url = "https://example.com/test.zip"
unknown_field = "invalid"
"#;
        assert!(toml::from_str::<ManifestConfig>(bad_toml).is_err());
    }
}
