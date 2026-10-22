use clap::Parser;
use crorust::{ProjectBuildArguments, SessionExecutionContext};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_manifest_android_sdk_override_relative_path() {
    let dir = tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "sdk-test-app"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let custom_sdk = root.join("custom_sdk");
    fs::create_dir_all(&custom_sdk).unwrap();

    fs::write(
        root.join("manifest.toml"),
        r#"
[android]
sdk = "custom_sdk"
"#,
    )
    .unwrap();

    let args = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "android",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();

    let env = SessionExecutionContext::new(args).unwrap();
    let sdk_path = env.locate_android_sdk();
    assert_eq!(
        sdk_path,
        dunce::simplified(&root.join("custom_sdk")).to_path_buf()
    );
}

#[test]
fn test_path_sanitization_for_local_properties() {
    let win_path = PathBuf::from(r"C:\Users\Developer\AppData\Local\Android\Sdk");
    let escaped = crorust::sanitize_path(&win_path);
    assert_eq!(escaped, "C:/Users/Developer/AppData/Local/Android/Sdk");

    let unc_path = PathBuf::from(r"\\?\C:\Users\Developer\Android\Sdk");
    let sanitized_unc = crorust::sanitize_path(&unc_path);
    assert!(!sanitized_unc.starts_with("//?/"));
    assert_eq!(sanitized_unc, "C:/Users/Developer/Android/Sdk");

    let unix_path = PathBuf::from("/home/user/Android/Sdk");
    let escaped_unix = crorust::sanitize_path(&unix_path);
    assert_eq!(escaped_unix, "/home/user/Android/Sdk");
}

#[test]
fn test_platform_toolchain_sdk_overrides() {
    let dir = tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "toolchain-test-app"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let custom_ndk = root.join("custom_ndk");
    let custom_win_sdk = root.join("custom_win_sdk");
    let custom_macos_sdk = root.join("custom_macos_sdk");
    let custom_ios_sdk = root.join("custom_ios_sdk");
    fs::create_dir_all(&custom_ndk).unwrap();
    fs::create_dir_all(&custom_win_sdk).unwrap();
    fs::create_dir_all(&custom_macos_sdk).unwrap();
    fs::create_dir_all(&custom_ios_sdk).unwrap();

    fs::write(
        root.join("manifest.toml"),
        r#"
[android]
ndk = "custom_ndk"

[windows]
sdk = "custom_win_sdk"

[macos]
sdk = "custom_macos_sdk"

[ios]
sdk = "custom_ios_sdk"
"#,
    )
    .unwrap();

    let args_android = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "android",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_android = SessionExecutionContext::new(args_android).unwrap();
    assert_eq!(
        env_android.resolve_android_ndk(),
        dunce::simplified(&custom_ndk).to_path_buf()
    );

    let args_win = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "windows",
        "--arch",
        "x64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_win = SessionExecutionContext::new(args_win).unwrap();
    assert_eq!(
        env_win.resolve_windows_sdk(),
        dunce::simplified(&custom_win_sdk).to_path_buf()
    );

    let args_mac = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "macos",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_mac = SessionExecutionContext::new(args_mac).unwrap();
    assert_eq!(
        env_mac.resolve_macos_sdk(),
        dunce::simplified(&custom_macos_sdk).to_path_buf()
    );

    let args_ios = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "ios",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_ios = SessionExecutionContext::new(args_ios).unwrap();
    assert_eq!(
        env_ios.resolve_ios_sdk(),
        dunce::simplified(&custom_ios_sdk).to_path_buf()
    );
    assert_eq!(
        env_ios.resolve_ios_simulator_sdk(),
        dunce::simplified(&custom_ios_sdk).to_path_buf()
    );
}

#[test]
fn test_android_sdk_version_hierarchies() {
    let dir = tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "sdk-version-app"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    // Default configuration (no explicit SDK versions set)
    let args_default = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "android",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_default = SessionExecutionContext::new(args_default).unwrap();
    assert_eq!(env_default.android_compile_sdk_version(), 35);
    assert_eq!(env_default.android_target_sdk_version(), 34);
    assert_eq!(env_default.android_min_sdk_version(), 21);

    // Explicit top-level Android configuration
    fs::write(
        root.join("manifest.toml"),
        r#"
[android]
compile_sdk = 35
target_sdk = 34
min_sdk = 23
"#,
    )
    .unwrap();
    let args_explicit = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "android",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_explicit = SessionExecutionContext::new(args_explicit).unwrap();
    assert_eq!(env_explicit.android_compile_sdk_version(), 35);
    assert_eq!(env_explicit.android_target_sdk_version(), 34);
    assert_eq!(env_explicit.android_min_sdk_version(), 23);
    assert!(env_explicit
        .resolve_android_jar()
        .ends_with("platforms/android-35/android.jar"));

    // Nested manifest SDK configuration fallback
    fs::write(
        root.join("manifest.toml"),
        r#"
[android]
compile_sdk = 35

[android.manifest.sdk]
target_sdk_version = 33
min_sdk_version = 24
"#,
    )
    .unwrap();
    let args_nested = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "android",
        "--arch",
        "arm64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();
    let env_nested = SessionExecutionContext::new(args_nested).unwrap();
    assert_eq!(env_nested.android_compile_sdk_version(), 35);
    assert_eq!(env_nested.android_target_sdk_version(), 33);
    assert_eq!(env_nested.android_min_sdk_version(), 24);
}

#[test]
fn test_windows_sdk_versioned_sysroot_resolution() {
    let _guard = TEST_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev_cache = std::env::var("CRORUST_CACHE_DIR").ok();
    std::env::remove_var("CRORUST_CACHE_DIR");

    let dir = tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "win-test-app"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let cache_dir = root.join("cache");
    let versioned_sdk = cache_dir
        .join("sysroots")
        .join("Windows.sdk")
        .join("10.0.22621.0");
    fs::create_dir_all(versioned_sdk.join("crt")).unwrap();

    fs::write(
        root.join("manifest.toml"),
        format!(
            r#"
[toolchain]
cache_dir = "{}"

[windows]
sdk_version = "10.0.22621.0"
"#,
            cache_dir.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();

    let args = ProjectBuildArguments::try_parse_from([
        "crorust",
        "--platform",
        "windows",
        "--arch",
        "x64",
        "--manifest-path",
        root.join("Cargo.toml").to_str().unwrap(),
    ])
    .unwrap();

    let env = SessionExecutionContext::new(args).unwrap();
    let resolved_sdk = env.resolve_windows_sdk();

    if let Some(val) = prev_cache {
        std::env::set_var("CRORUST_CACHE_DIR", val);
    }

    assert_eq!(
        resolved_sdk,
        dunce::simplified(&versioned_sdk).to_path_buf(),
        "resolve_windows_sdk must resolve the versioned subdirectory containing crt"
    );
}
