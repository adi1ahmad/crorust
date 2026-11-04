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
        "2.0.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.5.2")),
        "2.0.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.7.0")),
        "3.0.5"
    );
    assert_eq!(resolve_recommended_games_activity_version(None), "3.0.5");
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
fn test_semver_forward_compatibility() {
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.9.0")),
        "3.0.5"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("1.0.0")),
        "3.0.5"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("2.1.0")),
        "3.0.5"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("0.6.5")),
        "2.0.2"
    );
    // Prefixes like ^, ~, >=, =
    assert_eq!(
        resolve_recommended_games_activity_version(Some("^0.6.0")),
        "2.0.2"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some("~0.7.0")),
        "3.0.5"
    );
    assert_eq!(
        resolve_recommended_games_activity_version(Some(">=1.0.0")),
        "3.0.5"
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
