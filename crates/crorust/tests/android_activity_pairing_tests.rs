use crorust::android::{
    parse_android_activity_from_lock, parse_android_activity_from_manifest,
    resolve_recommended_games_activity_version,
};
use crorust::config::AndroidPlatformConfig;

#[test]
fn test_game_activity_config_variants() {
    // 1. Boolean true
    let toml_bool = r#"
        game_activity = true
    "#;
    let cfg: AndroidPlatformConfig =
        toml::from_str(toml_bool).expect("Failed parsing boolean true");
    assert!(cfg.game_activity());
    assert_eq!(cfg.explicit_game_activity_version(), None);

    // 2. Boolean false
    let toml_bool_false = r#"
        game_activity = false
    "#;
    let cfg: AndroidPlatformConfig =
        toml::from_str(toml_bool_false).expect("Failed parsing boolean false");
    assert!(!cfg.game_activity());
    assert_eq!(cfg.explicit_game_activity_version(), None);

    // 3. Explicit version string
    let toml_ver = r#"
        game_activity = "2.0.2"
    "#;
    let cfg: AndroidPlatformConfig =
        toml::from_str(toml_ver).expect("Failed parsing version string");
    assert!(cfg.game_activity());
    assert_eq!(cfg.explicit_game_activity_version(), Some("2.0.2"));

    // 4. Structured activity table
    let toml_table = r#"
        [activity]
        type = "game"
        version = "2.0.2"
    "#;
    let cfg: AndroidPlatformConfig =
        toml::from_str(toml_table).expect("Failed parsing activity table");
    assert!(cfg.game_activity());
    assert_eq!(cfg.explicit_game_activity_version(), Some("2.0.2"));

    // 5. NativeActivity in table
    let toml_native = r#"
        [activity]
        type = "native"
    "#;
    let cfg: AndroidPlatformConfig =
        toml::from_str(toml_native).expect("Failed parsing native activity");
    assert!(!cfg.game_activity());
}

#[test]
fn test_parse_android_activity_lock_and_manifest() {
    let sample_lock = r#"
version = 4

[[package]]
name = "android-activity"
version = "0.6.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
    assert_eq!(
        parse_android_activity_from_lock(sample_lock),
        Some("0.6.0".into())
    );

    let sample_manifest = r#"
[target.'cfg(target_os = "android")'.dependencies]
android-activity = { version = "0.6.0", features = ["game-activity"] }
"#;
    assert_eq!(
        parse_android_activity_from_manifest(sample_manifest),
        Some("0.6.0".into())
    );
}

#[test]
fn test_version_pairing_logic() {
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.6.0")),
        "2.0.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.6.1")),
        "4.4.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.5.2")),
        "2.0.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.7.0")),
        "4.4.2"
    );
    assert_eq!(resolve_recommended_games_activity_version(None), "4.4.2");
}

#[test]
fn test_lockfile_version_order_insensitivity() {
    // Version serialized before name
    let lock_reversed = r#"
[[package]]
version = "0.7.2"
name = "android-activity"
"#;
    assert_eq!(
        parse_android_activity_from_lock(lock_reversed),
        Some("0.7.2".into())
    );
}

#[test]
fn test_manifest_table_and_sibling_exclusion() {
    let manifest_content = r#"
[dependencies]
android-activity-sys = "0.1.0"
android-activity-winit = { version = "0.2.0" }

[dependencies.android-activity]
version = "0.6.2"
features = ["game-activity"]
"#;
    assert_eq!(
        parse_android_activity_from_manifest(manifest_content),
        Some("0.6.2".into())
    );
}

#[test]
fn test_manifest_transitive_winit_inference() {
    let winit_30 = r#"
[target.'cfg(target_os = "android")'.dependencies]
winit = { version = "0.30", features = ["android-game-activity"] }
"#;
    assert_eq!(
        parse_android_activity_from_manifest(winit_30),
        Some("0.6.1".into())
    );

    let winit_29 = r#"
[dependencies]
winit = "0.29.10"
"#;
    assert_eq!(
        parse_android_activity_from_manifest(winit_29),
        Some("0.5.2".into())
    );

    let winit_10 = r#"
[dependencies]
winit = "1.0.0"
"#;
    assert_eq!(
        parse_android_activity_from_manifest(winit_10),
        Some("0.6.1".into())
    );
}

#[test]
fn test_semver_forward_compatibility() {
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.9.0")),
        "4.4.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("1.0.0")),
        "4.4.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("2.1.0")),
        "4.4.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.6.5")),
        "4.4.2"
    );
    // Unpinned "0.6" resolves to latest 0.6.x (0.6.4+) -> 4.4.2
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.6")),
        "4.4.2"
    );
    // Caret "^0.6.0" resolves to latest 0.6.x (0.6.4+) -> 4.4.2
    assert_eq!(
        resolve_recommended_games_activity_version(Some("^0.6.0")),
        "4.4.2"
    );
    // Exact "=0.6.0" pins to 0.6.0 -> 2.0.2
    assert_eq!(
        resolve_recommended_games_activity_version(Some("=0.6.0")),
        "2.0.2"
    );
    // Exact literal "0.6.0" -> 2.0.2
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.6.0")),
        "2.0.2"
    );
    // Older 0.5.x -> 2.0.2
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.5.2")),
        "2.0.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("~0.7.0")),
        "4.4.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some(">=1.0.0")),
        "4.4.2"
    );
}

#[test]
fn test_lockfile_with_inline_comments_does_not_abort_parser() {
    let lock_with_comments = r#"
version = 4

[[package]]
name = "preceding-crate"
version = "1.2.3" # custom vendor patch

[[package]]
name = "android-activity"
version = "0.6.4"
"#;
    assert_eq!(
        parse_android_activity_from_lock(lock_with_comments),
        Some("0.6.4".into())
    );
}

#[test]
fn test_game_activity_abi_validation() {
    use crorust::android::abi::{
        detect_class_game_activity_abi, detect_dex_file_game_activity_abi,
        detect_native_game_activity_abi, validate_game_activity_abi, GameActivityJniAbi,
        SIG_17_ARG, SIG_2_ARG,
    };
    use std::io::Write;
    use tempfile::NamedTempFile;

    // 1. Mock SO with 17-arg signature
    let mut so_17 = NamedTempFile::new().unwrap();
    so_17.write_all(b"\x7fELFfakeheader...").unwrap();
    so_17.write_all(SIG_17_ARG).unwrap();
    so_17.flush().unwrap();

    assert_eq!(
        detect_native_game_activity_abi(so_17.path()).unwrap(),
        Some(GameActivityJniAbi::SeventeenArg)
    );

    // 2. Mock SO with 2-arg signature
    let mut so_2 = NamedTempFile::new().unwrap();
    so_2.write_all(b"\x7fELFfakeheader...").unwrap();
    so_2.write_all(SIG_2_ARG).unwrap();
    so_2.flush().unwrap();

    assert_eq!(
        detect_native_game_activity_abi(so_2.path()).unwrap(),
        Some(GameActivityJniAbi::TwoArg)
    );

    // 3. Mock class/dex bytes
    let class_17 = [
        b"com/google/androidgamesdk/GameActivity".as_slice(),
        SIG_17_ARG,
    ]
    .concat();
    let class_2 = [
        b"com/google/androidgamesdk/GameActivity".as_slice(),
        SIG_2_ARG,
    ]
    .concat();

    assert_eq!(
        detect_class_game_activity_abi(&class_17),
        Some(GameActivityJniAbi::SeventeenArg)
    );
    assert_eq!(
        detect_class_game_activity_abi(&class_2),
        Some(GameActivityJniAbi::TwoArg)
    );

    // 4. Parity validations
    assert!(validate_game_activity_abi(so_17.path(), Some(&class_17), None).is_ok());
    assert!(validate_game_activity_abi(so_2.path(), Some(&class_2), None).is_ok());

    // 5. Mismatch validations
    let err_17_native_2_class =
        validate_game_activity_abi(so_17.path(), Some(&class_2), Some("2.0.2"));
    assert!(err_17_native_2_class.is_err());
    let err_msg = err_17_native_2_class.unwrap_err().to_string();
    assert!(err_msg.contains("Native JNI ABI Mismatch"));
    assert!(err_msg.contains("4.4.2"));

    let err_2_native_17_class =
        validate_game_activity_abi(so_2.path(), Some(&class_17), Some("4.4.2"));
    assert!(err_2_native_17_class.is_err());
    let err_msg2 = err_2_native_17_class.unwrap_err().to_string();
    assert!(err_msg2.contains("Native JNI ABI Mismatch"));
    assert!(err_msg2.contains("2.0.2"));

    // 6. Explicit version fallback parity when DEX bytes are absent:
    // Native SO 17-arg should pass against explicit version "4.4.2" without DEX bytes
    assert!(validate_game_activity_abi(so_17.path(), None, Some("4.4.2")).is_ok());
    // Native SO 2-arg should pass against explicit version "2.0.2" without DEX bytes
    assert!(validate_game_activity_abi(so_2.path(), None, Some("2.0.2")).is_ok());

    // Native SO 17-arg must FAIL against explicit version "2.0.2" even if DEX bytes are None
    let err_explicit_mismatch = validate_game_activity_abi(so_17.path(), None, Some("2.0.2"));
    assert!(err_explicit_mismatch.is_err());
    let err_explicit_msg = err_explicit_mismatch.unwrap_err().to_string();
    assert!(err_explicit_msg.contains("Native JNI ABI Mismatch"));
    assert!(err_explicit_msg.contains("4.4.2"));

    // 7. Streaming chunk boundary test: needle placed exactly across 64KB boundary
    let mut boundary_so = NamedTempFile::new().unwrap();
    let padding = vec![0xAAu8; 64 * 1024 - 10]; // 10 bytes before boundary
    boundary_so.write_all(&padding).unwrap();
    boundary_so.write_all(SIG_17_ARG).unwrap(); // Crosses the 64KB boundary
    boundary_so.flush().unwrap();

    assert_eq!(
        detect_native_game_activity_abi(boundary_so.path()).unwrap(),
        Some(GameActivityJniAbi::SeventeenArg)
    );

    // 8. Streaming DEX file test:
    let mut dex_file_17 = NamedTempFile::new().unwrap();
    dex_file_17.write_all(b"dex\n035\0fake_dex_header").unwrap();
    dex_file_17.write_all(SIG_17_ARG).unwrap();
    dex_file_17.flush().unwrap();

    assert_eq!(
        detect_dex_file_game_activity_abi(dex_file_17.path()).unwrap(),
        Some(GameActivityJniAbi::SeventeenArg)
    );

    let mut dex_file_2 = NamedTempFile::new().unwrap();
    dex_file_2.write_all(b"dex\n035\0fake_dex_header").unwrap();
    dex_file_2.write_all(SIG_2_ARG).unwrap();
    dex_file_2.flush().unwrap();

    assert_eq!(
        detect_dex_file_game_activity_abi(dex_file_2.path()).unwrap(),
        Some(GameActivityJniAbi::TwoArg)
    );
}
