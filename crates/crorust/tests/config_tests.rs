use crorust::config::{
    CacheScope, LinuxLibcVariant, ManifestConfig, RepositoryEntry, SquashfsBackend,
    SquashfsCompression,
};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn test_parse_complete_valid_manifest_toml() {
    let toml_str = r#"
package = "com.example.modernapp"
name = "ModernApp"
version = "2.1.0"
version_code = 210
icon = "assets/icon.png"

[toolchain]
offline = true
mirror_url = "https://artifactory.corp.internal/dist"
nix = true

[android]
min_sdk = 26
target_sdk = 35
compile_sdk = 35
ndk_version = "26.3.11579264"
agp_version = "8.8.0"
kotlin_version = "2.1.10"
jvm_target = "21"
game_activity = false

dependencies = [
  "androidx.core:core-ktx:1.13.1"
]

[android.gradle]
version = "8.8"
jvm_args = "-Xmx6144m"

[[android.repositories]]
standard = "google"

[[android.repositories]]
standard = "mavenCentral"

[[android.repositories]]
maven = "https://nexus.corp.internal/repository/maven-public/"
username_env = "NEXUS_USER"
password_env = "NEXUS_PASS"

[macos]
sdk_version = "14.4"
minimum_system_version = "12.0"
bundle_identifier = "com.example.modernapp.mac"

[ios]
sdk_version = "17.4"
minimum_os_version = "16.0"
bundle_identifier = "com.example.modernapp.ios"
simulator = true

[linux]
libc = "musl"

[windows]
sdk_version = "10.0.22621.0"
publisher = "CN=Contoso"
"#;

    let config: ManifestConfig =
        toml::from_str(toml_str).expect("TOML manifest must parse successfully");

    assert_eq!(config.package_id(), Some("com.example.modernapp"));
    assert_eq!(config.app_name(), Some("ModernApp"));
    assert_eq!(config.app_version(), Some("2.1.0"));
    assert_eq!(config.app_version_code(), Some(210));
    assert_eq!(
        config.app_icon(crorust::TargetPlatform::Android),
        Some(Path::new("assets/icon.png"))
    );
    assert!(config.toolchain().offline());
    assert_eq!(
        config.toolchain().mirror_url.as_deref(),
        Some("https://artifactory.corp.internal/dist")
    );
    assert_eq!(config.toolchain().nix, Some(true));

    let android = config.android();
    assert_eq!(android.min_sdk, Some(26));
    assert_eq!(android.target_sdk, Some(35));
    assert_eq!(android.compile_sdk, Some(35));
    assert_eq!(android.ndk_version.as_deref(), Some("26.3.11579264"));
    let gradle = android.gradle.as_ref().expect("Gradle config present");
    assert_eq!(gradle.version, "8.8");
    assert_eq!(gradle.jvm_args, "-Xmx6144m");
    assert_eq!(android.agp_version(), "8.8.0");
    assert_eq!(android.kotlin_version(), "2.1.10");
    assert_eq!(android.jvm_target(), "21");
    assert_eq!(android.repositories.len(), 3);
    assert!(
        matches!(&android.repositories[0], RepositoryEntry::Standard { standard } if standard == "google")
    );
    assert!(
        matches!(&android.repositories[2], RepositoryEntry::Maven { maven, username_env, .. } if maven.contains("nexus") && username_env.as_deref() == Some("NEXUS_USER"))
    );
    assert!(!android.game_activity());

    assert_eq!(config.ios().simulator, Some(true));
    assert_eq!(config.ios().minimum_os_version(), "16.0");
    assert_eq!(config.macos().minimum_system_version(), "12.0");
    assert_eq!(config.linux().libc, Some(LinuxLibcVariant::Musl));
    assert_eq!(config.windows().sdk_version(), "10.0.22621.0");
}

#[test]
fn test_cargo_metadata_fallback_extraction() {
    let temp = tempdir().unwrap();
    let cargo_path = temp.path().join("Cargo.toml");
    let cargo_str = r#"
[package]
name = "embedded-config-app"
version = "0.1.0"
edition = "2021"

[package.metadata.crorust]
package = "com.example.embedded"
icon = "res/icon.png"

[package.metadata.crorust.android]
min_sdk = 26
target_sdk = 34

[package.metadata.crorust.toolchain]
offline = true
cache_scope = "global"
"#;
    std::fs::write(&cargo_path, cargo_str).unwrap();

    let config = ManifestConfig::parse_project(temp.path(), &cargo_path).unwrap();
    assert_eq!(config.package_id(), Some("com.example.embedded"));
    assert_eq!(config.android().min_sdk, Some(26));
    assert_eq!(config.android().target_sdk, Some(34));
    assert!(config.toolchain().offline());
}

#[test]
fn test_deny_unknown_fields_rejects_typos_in_toml() {
    let toml_typo_top = r#"
package = "com.example.app"
invalid_top_key = true
"#;
    let err = toml::from_str::<ManifestConfig>(toml_typo_top).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `invalid_top_key`"),
        "Expected unknown field rejection, got: {err}"
    );

    let toml_typo_android = r#"
[android]
agp_vesrion = "8.7.3"
"#;
    let err = toml::from_str::<ManifestConfig>(toml_typo_android).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `agp_vesrion`"),
        "Expected unknown field rejection in android block, got: {err}"
    );

    let toml_typo_toolchain = r#"
[toolchain]
offlin = true
"#;
    let err = toml::from_str::<ManifestConfig>(toml_typo_toolchain).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `offlin`"),
        "Expected unknown field rejection in toolchain block, got: {err}"
    );
}

#[test]
fn test_default_values_populated_when_omitted() {
    let toml_str = r#"
package = "com.example.defaults"

[android]
min_sdk = 21
"#;
    let config: ManifestConfig = toml::from_str(toml_str).unwrap();
    let android = config.android();

    assert_eq!(android.agp_version(), "8.7.3");
    assert_eq!(android.kotlin_version(), "2.1.0");
    assert_eq!(android.jvm_target(), "17");
    assert_eq!(config.macos().minimum_system_version(), "11.0");
    assert_eq!(config.ios().minimum_os_version(), "15.0");
    assert_eq!(config.windows().sdk_version(), "10.0.22621.0");
    assert!(!config.toolchain().offline());
    assert_eq!(config.toolchain().mirror_url, None);
}

#[test]
fn test_linux_libc_enum_validation() {
    let toml_valid_gnu = "[linux]\nlibc = \"gnu\"\n";
    let cfg: ManifestConfig = toml::from_str(toml_valid_gnu).unwrap();
    assert_eq!(cfg.linux().libc, Some(LinuxLibcVariant::Gnu));

    let toml_valid_musl = "[linux]\nlibc = \"musl\"\n";
    let cfg: ManifestConfig = toml::from_str(toml_valid_musl).unwrap();
    assert_eq!(cfg.linux().libc, Some(LinuxLibcVariant::Musl));

    let toml_invalid = "[linux]\nlibc = \"glibc_custom\"\n";
    let err = toml::from_str::<ManifestConfig>(toml_invalid).unwrap_err();
    assert!(err.to_string().contains("unknown variant `glibc_custom`"));
}

#[test]
fn test_template_manifest_parses_successfully() {
    let manifest_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/crorust.ruxt/manifest.toml");
    assert!(
        manifest_path.exists(),
        "Template manifest must exist at '{}'.",
        manifest_path.display()
    );
    let config = ManifestConfig::parse_file(&manifest_path).expect(
        "templates/crorust.ruxt/manifest.toml must parse cleanly against ManifestConfig schema",
    );
    assert_eq!(config.package_id(), Some("ruxt"));
    assert_eq!(config.app_name(), Some("ruxt"));
    assert_eq!(config.bin(), Some("ruxt-app"));
    assert_eq!(config.app_version(), Some("0.1.0"));
    assert_eq!(config.app_version_code(), Some(1));
    assert_eq!(config.toolchain().cache_scope(), CacheScope::Project);
    assert!(!config.toolchain().offline());
    assert_eq!(config.android().target_sdk, Some(34));
    assert_eq!(config.android().compile_sdk, Some(35));
    assert!(!config.android().use_gradle());
    assert_eq!(config.linux().libc, Some(LinuxLibcVariant::Gnu));
    assert_eq!(
        config.macos().bundle_identifier.as_deref(),
        Some("crorust.ruxt")
    );
    assert_eq!(
        config.ios().bundle_identifier.as_deref(),
        Some("crorust.ruxt")
    );
    assert_eq!(config.windows().publisher.as_deref(), Some("CN=crorust"));
    assert_eq!(config.web().port, Some(8080));
    assert_eq!(config.web().open, Some(false));
    assert_eq!(
        config.web().dist.as_deref(),
        Some(Path::new(".cache/web-static/dist"))
    );
    assert_eq!(config.web().address.as_deref(), Some("127.0.0.1"));
}

#[test]
fn test_exhaustive_manifest_with_all_options_enabled() {
    let toml_str = r##"
package = "exhaustive-app"
name = "exhaustive_app"
bin = "exhaustive-bin"
version = "1.2.3"
version_code = 42
icon = "assets/icon_512.png"

[toolchain]
cache_scope = "global"
offline = true
mirror_url = "https://mirror.internal.net"
nix = false
cache_dir = "/custom/cache"

[toolchain.android.platform.35]
url = "https://dl.google.com/android/repository/platform-35_r02.zip"
sha256 = "0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0"

[android]
min_sdk = 21
target_sdk = 34
compile_sdk = 35
use_gradle = true
agp_version = "8.7.3"
kotlin_version = "2.1.0"
jvm_target = "17"
sdk = "/opt/android-sdk"
ndk = "/opt/android-ndk"
ndk_version = "27.2.12479018"
game_activity = true

repositories = [
    { standard = "google" },
    { standard = "mavenCentral" },
    { maven = "https://maven.pkg.github.com/my-org/packages", username_env = "USER", password_env = "TOKEN" },
]

dependencies = [
    "androidx.appcompat:appcompat:1.6.1",
]

assets = [
    "assets",
    { path = "assets/model.glb", dest = "model.glb", optional = true, alignment = 4096 },
    { path = "assets/raw.bin", alignment = "unaligned" },
    { path = "assets/data.zip", alignment = "compressed" },
]

[android.gradle]
enabled = true
version = "8.7"
distribution_url = "https://services.gradle.org/distributions/gradle-8.7-bin.zip"
jvm_args = "-Xmx2048m -Dfile.encoding=UTF-8"

[android.debug.reverse]
"tcp:8080" = "tcp:8080"

[android.debug.forward]
"tcp:5005" = "tcp:5005"

[android.manifest]
package = "com.example.exhaustive"
version_code = 42
version_name = "1.2.3"
compile_sdk_version = 35
compile_sdk_version_codename = 35
platform_build_version_code = 35
platform_build_version_name = 35

[android.manifest.sdk]
min_sdk_version = 21
target_sdk_version = 34
max_sdk_version = 35

[[android.manifest.permission]]
name = "com.example.CUSTOM_PERMISSION"
protection_level = "signature"

[[android.manifest.uses_feature]]
name = "android.hardware.camera"
required = false
version = 1
opengles_version = [3, 2]

[[android.manifest.uses_permission]]
name = "android.permission.INTERNET"
max_sdk_version = 34

[android.manifest.application]
label = "Exhaustive App"
debuggable = false
has_code = true
theme = "@style/Theme.AppCompat.NoActionBar"
icon = "@mipmap/ic_launcher"
app_component_factory = "androidx.core.app.CoreComponentFactory"
extract_native_libs = false

[[android.manifest.application.meta_data]]
name = "app_meta"
value = "meta_val"

[[android.manifest.application.providers]]
name = "androidx.startup.InitializationProvider"
authorities = "com.example.exhaustive.startup"
exported = false
grant_uri_permissions = true

[[android.manifest.application.activities]]
name = "com.example.exhaustive.MainActivity"
theme = "@style/Theme.AppCompat.NoActionBar"
exported = true
hardware_accelerated = true
window_soft_input_mode = "adjustResize"
config_changes = "orientation|screenSize"
launch_mode = "singleTop"
orientation = "sensor"
color_mode = "wideColorGamut"

[[android.manifest.application.activities.intent_filters]]
actions = ["android.intent.action.VIEW"]
categories = ["android.intent.category.DEFAULT"]

[[android.manifest.application.activities.intent_filters.data]]
scheme = "https"
host = "crorust.com"
path_prefix = "/open"

[linux]
libc = "musl"
squashfs_backend = "pure-rust"
compression = "xz"

[macos]
sdk_version = "14.4"
minimum_system_version = "11.0"
bundle_identifier = "com.example.exhaustive"
sdk = "/opt/MacOSX.sdk"

[macos.info]
CFBundleDisplayName = "Exhaustive App"
NSCameraUsageDescription = "Camera required for inspection"

[ios]
sdk_version = "17.4"
minimum_os_version = "15.0"
bundle_identifier = "com.example.exhaustive"
simulator = true
sdk = "/opt/iPhoneOS.sdk"
assets_car = "assets/Assets.car"

[ios.info]
CFBundleDisplayName = "Exhaustive App"
MinimumOSVersion = "15.0"
LSRequiresIPhoneOS = true
UIDeviceFamily = [1, 2]

[windows]
sdk_version = "10.0.22621.0"
publisher = "CN=ExhaustiveCorp"
sdk = "C:\\SDK"

[windows.manifest.identity]
name = "ExhaustiveApp"
version = "1.2.3.0"
publisher = "CN=ExhaustiveCorp"
processor_architecture = "x64"

[windows.manifest.properties]
display_name = "Exhaustive App"
publisher_display_name = "ExhaustiveCorp"
logo = "Assets\\Logo.png"
description = "Full featured app"

[[windows.manifest.resources.resource]]
language = "en-US"

[[windows.manifest.dependencies.target_device_family]]
name = "Windows.Desktop"
min_version = "10.0.0.0"
max_version = "10.0.22621.0"

[[windows.manifest.capabilities]]
capability = { name = "internetClient" }

[[windows.manifest.applications.application]]
id = "App"
executable = "exhaustive-bin.exe"
entry_point = "Windows.FullTrustApplication"

[windows.manifest.applications.application.visual_elements]
display_name = "Exhaustive App"
description = "Full featured app"
background_color = "#1E1E1E"

[web]
port = 9090
open = true
dist = "custom_dist"
address = "0.0.0.0"
watch_ignore = [".cache", "tmp"]
"##;

    let cfg: ManifestConfig = toml::from_str(toml_str)
        .expect("Exhaustive manifest with all features enabled must deserialize without error");
    assert_eq!(cfg.package_id(), Some("exhaustive-app"));
    assert_eq!(cfg.toolchain.cache_scope(), CacheScope::Global);
    assert!(cfg.toolchain.is_offline_enforced());
    assert_eq!(cfg.android.min_sdk, Some(21));
    assert_eq!(cfg.linux.libc, Some(LinuxLibcVariant::Musl));
    assert_eq!(cfg.linux.squashfs_backend, Some(SquashfsBackend::PureRust));
    assert_eq!(cfg.linux.compression, Some(SquashfsCompression::Xz));
    assert_eq!(cfg.web.port, Some(9090));
    assert_eq!(cfg.web.address.as_deref(), Some("0.0.0.0"));
    assert_eq!(
        cfg.web.watch_ignore,
        Some(vec![PathBuf::from(".cache"), PathBuf::from("tmp")])
    );
    assert_eq!(
        cfg.android
            .debug
            .reverse
            .get("tcp:8080")
            .map(|s| s.as_str()),
        Some("tcp:8080")
    );
    assert_eq!(
        cfg.macos.info.display_name.as_deref(),
        Some("Exhaustive App")
    );
    assert_eq!(cfg.ios.simulator, Some(true));
    assert_eq!(cfg.windows.publisher.as_deref(), Some("CN=ExhaustiveCorp"));
}

#[test]
fn test_manifest_config_merge_exhaustive() {
    let base_toml = r#"
package = "base-app"
name = "base_app"

[android.debug.reverse]
"tcp:8080" = "tcp:8080"

[web]
port = 8080
address = "127.0.0.1"
watch_ignore = [".cache"]

[macos.info]
CFBundleName = "BaseApp"

[windows.manifest.identity]
name = "BaseIdentity"
"#;

    let override_toml = r#"
[android.debug.reverse]
"tcp:9090" = "tcp:9090"

[android.debug.forward]
"tcp:5005" = "tcp:5005"

[web]
port = 9090
address = "0.0.0.0"
watch_ignore = [".cache", "target"]

[macos.info]
CFBundleDisplayName = "OverrideApp"

[windows.manifest.identity]
name = "OverrideIdentity"
"#;

    let mut base: ManifestConfig = toml::from_str(base_toml).unwrap();
    let child: ManifestConfig = toml::from_str(override_toml).unwrap();
    base.merge(child);

    assert_eq!(base.package_id(), Some("base-app"));
    assert_eq!(base.web.port, Some(9090));
    assert_eq!(base.web.address.as_deref(), Some("0.0.0.0"));
    assert_eq!(
        base.web.watch_ignore,
        Some(vec![PathBuf::from(".cache"), PathBuf::from("target")])
    );
    assert_eq!(
        base.android
            .debug
            .reverse
            .get("tcp:8080")
            .map(|s| s.as_str()),
        Some("tcp:8080")
    );
    assert_eq!(
        base.android
            .debug
            .reverse
            .get("tcp:9090")
            .map(|s| s.as_str()),
        Some("tcp:9090")
    );
    assert_eq!(
        base.android
            .debug
            .forward
            .get("tcp:5005")
            .map(|s| s.as_str()),
        Some("tcp:5005")
    );
    assert_eq!(base.macos.info.bundle_name.as_deref(), Some("BaseApp"));
    assert_eq!(base.macos.info.display_name.as_deref(), Some("OverrideApp"));
    assert_eq!(
        base.windows.manifest.identity.name.as_deref(),
        Some("OverrideIdentity")
    );
}

#[test]
fn test_toolchain_sdk_overrides_and_merge() {
    let toml_str = r#"
package = "test-toolchain"
name = "test_toolchain"

[toolchain.windows.sdk]
url = "https://custom.win.internal/Windows.sdk.tar.zst"
sha256 = "1111222233334444555566667777888899990000111122223333444455556666"

[toolchain.apple.macos_sdk]
url = "https://custom.mac.internal/MacOSX.sdk.tar.zst"

[toolchain.apple.ios_sdk]
url = "https://custom.ios.internal/iPhoneOS.sdk.tar.zst"
sha256 = "aaaabbbbccccddddeeeeffff0000111122223333444455556666777788889999"

[toolchain.android.ndk]
url = "https://custom.android.internal/Android.ndk.tar.zst"
"#;

    let config: ManifestConfig = toml::from_str(toml_str).unwrap();
    let toolchain = config.toolchain();

    let win_override = toolchain.get_windows_sdk_override().unwrap();
    assert_eq!(
        win_override.url,
        "https://custom.win.internal/Windows.sdk.tar.zst"
    );
    assert_eq!(
        win_override.sha256.as_deref(),
        Some("1111222233334444555566667777888899990000111122223333444455556666")
    );

    let mac_override = toolchain.get_macos_sdk_override().unwrap();
    assert_eq!(
        mac_override.url,
        "https://custom.mac.internal/MacOSX.sdk.tar.zst"
    );
    assert_eq!(mac_override.sha256, None);

    let ios_override = toolchain.get_ios_sdk_override().unwrap();
    assert_eq!(
        ios_override.url,
        "https://custom.ios.internal/iPhoneOS.sdk.tar.zst"
    );

    let ndk_override = toolchain.get_android_ndk_override().unwrap();
    assert_eq!(
        ndk_override.url,
        "https://custom.android.internal/Android.ndk.tar.zst"
    );

    // Test merge preserves and overrides properly
    let child_toml = r#"
[toolchain.windows.sdk]
url = "https://child.win.internal/Windows.sdk.tar.zst"
"#;
    let mut merged = config.clone();
    let child_config: ManifestConfig = toml::from_str(child_toml).unwrap();
    merged.merge(child_config);

    assert_eq!(
        merged.toolchain().get_windows_sdk_override().unwrap().url,
        "https://child.win.internal/Windows.sdk.tar.zst"
    );
    // Other fields in apple/android preserved
    assert_eq!(
        merged.toolchain().get_macos_sdk_override().unwrap().url,
        "https://custom.mac.internal/MacOSX.sdk.tar.zst"
    );
}

#[test]
fn test_wry_field_rejected_under_deny_unknown_fields() {
    let toml_with_wry = r#"
package = "com.example.rejection"
name = "rejection"
bin = "rejection"

[android]
wry = true
"#;
    let err = toml::from_str::<ManifestConfig>(toml_with_wry).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `wry`"),
        "Expected unknown field rejection for obsolete `wry` key, got: {err}"
    );
}
