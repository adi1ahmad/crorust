use crorust::commands::template::{ProjectNames, ScaffoldEngine};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_project_names_transformations() {
    let names = ProjectNames::new("my-cool-game").expect("valid name");
    assert_eq!(names.raw, "my-cool-game");
    assert_eq!(names.kebab, "my-cool-game");
    assert_eq!(names.snake, "my_cool_game");
    assert_eq!(names.pascal, "MyCoolGame");
    assert_eq!(names.title, "My Cool Game");
    assert_eq!(names.package_id, "com.example.my_cool_game");
    assert_eq!(names.jni_symbol_prefix, "com_example_my_1cool_1game");
}

#[test]
fn test_invalid_and_reserved_names_rejected() {
    assert!(ProjectNames::new("").is_err());
    assert!(ProjectNames::new("   ").is_err());

    assert!(ProjectNames::new("123app").is_err());

    assert!(ProjectNames::new("app$name").is_err());
    assert!(ProjectNames::new("my app").is_err());

    assert!(ProjectNames::new("fn").is_err());
    assert!(ProjectNames::new("struct").is_err());
    assert!(ProjectNames::new("match").is_err());
    assert!(ProjectNames::new("crate").is_err());
    assert!(ProjectNames::new("super").is_err());

    assert!(ProjectNames::new("con").is_err());
    assert!(ProjectNames::new("prn").is_err());
    assert!(ProjectNames::new("aux").is_err());
    assert!(ProjectNames::new("nul").is_err());
    assert!(ProjectNames::new("com1").is_err());
}

#[test]
fn test_scaffold_generation_from_template() {
    let temp = tempdir().expect("create temp dir");
    let project_dir = temp.path().join("sandbox_app");

    let names = ProjectNames::new("sandbox_app").unwrap();
    ScaffoldEngine::new(&names, &project_dir).execute().unwrap();

    let cargo_str = fs::read_to_string(project_dir.join("Cargo.toml")).unwrap();
    let cargo_val: toml::Value = toml::from_str(&cargo_str).expect("Cargo.toml must be valid TOML");
    assert_eq!(
        cargo_val["package"]["name"].as_str().unwrap(),
        "sandbox-app"
    );

    let manifest_str = fs::read_to_string(project_dir.join("manifest.toml")).unwrap();
    let manifest_val: toml::Value =
        toml::from_str(&manifest_str).expect("manifest.toml must be valid TOML");
    assert_eq!(manifest_val["package"].as_str().unwrap(), "sandbox-app");
    assert_eq!(manifest_val["name"].as_str().unwrap(), "sandbox_app");
    assert_eq!(manifest_val["bin"].as_str().unwrap(), "sandbox_app");
    assert_eq!(
        manifest_val["android"]["manifest"]["application"]["label"]
            .as_str()
            .unwrap(),
        "Sandbox App"
    );
    assert_eq!(
        manifest_val["android"]["manifest"]["package"]
            .as_str()
            .unwrap(),
        "com.example.sandbox_app"
    );

    let kt_str = fs::read_to_string(project_dir.join("kotlin/MainActivity.kt")).unwrap();
    assert!(kt_str.contains("package com.example.sandbox_app"));
    assert!(kt_str.contains("System.loadLibrary(\"sandbox_app\")"));

    let icon_path = project_dir.join("assets/icon_512.png");
    assert!(icon_path.exists(), "icon_512.png must exist");
    let png_bytes = fs::read(&icon_path).unwrap();
    assert!(
        png_bytes.len() > 100,
        "icon_512.png must be non-empty valid image asset"
    );
}

#[test]
fn test_scaffold_preserves_executable_permissions() {
    let temp = tempdir().expect("create temp dir");
    let project_dir = temp.path().join("perm_test_app");

    let names = ProjectNames::new("perm_test_app").unwrap();
    ScaffoldEngine::new(&names, &project_dir).execute().unwrap();

    let ci_script = project_dir.join("nix/scripts/ci.sh");
    assert!(
        ci_script.exists(),
        "nix/scripts/ci.sh must exist in scaffolded output"
    );

    let snapshot_script = project_dir.join("nix/scripts/check-snapshots.sh");
    assert!(
        snapshot_script.exists(),
        "check-snapshots.sh must exist in scaffolded output"
    );

    let cargo_toml = project_dir.join("Cargo.toml");
    assert!(
        cargo_toml.exists(),
        "Cargo.toml must exist in scaffolded output"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let ci_meta = fs::metadata(&ci_script).unwrap();
        let ci_mode = ci_meta.permissions().mode();
        assert_eq!(
            ci_mode & 0o111,
            0o111,
            "ci.sh should have executable bits set (mode: {:o})",
            ci_mode
        );

        let snap_meta = fs::metadata(&snapshot_script).unwrap();
        let snap_mode = snap_meta.permissions().mode();
        assert_eq!(
            snap_mode & 0o111,
            0o111,
            "check-snapshots.sh should have executable bits set (mode: {:o})",
            snap_mode
        );

        let toml_meta = fs::metadata(&cargo_toml).unwrap();
        let toml_mode = toml_meta.permissions().mode();
        assert_eq!(
            toml_mode & 0o111,
            0,
            "Cargo.toml must not have executable bits set (mode: {:o})",
            toml_mode
        );
    }
}

#[test]
fn test_template_init_entrypoint() {
    let temp = tempdir().expect("create temp dir");
    let project_dir = temp.path().join("template_test_app");

    crorust::commands::template::init(project_dir.to_str().unwrap())
        .expect("crorust::commands::template::init must succeed");

    assert!(
        project_dir.join("Cargo.toml").exists(),
        "Cargo.toml must be generated"
    );
    assert!(
        project_dir.join("manifest.toml").exists(),
        "manifest.toml must be generated"
    );
    assert!(
        project_dir.join("src/main.rs").exists(),
        "src/main.rs must be generated"
    );
    assert!(
        project_dir.join(".gitignore").exists(),
        ".gitignore must be generated"
    );
    assert!(
        project_dir.join(".vscode/settings.json").exists(),
        ".vscode/settings.json must be generated"
    );
    assert!(
        project_dir.join("sw.js").exists(),
        "sw.js PWA service worker must be generated"
    );
    assert!(
        project_dir.join("assets/manifest.webmanifest").exists(),
        "assets/manifest.webmanifest must be generated"
    );
}

#[test]
fn test_crlf_line_ending_scaffold_substitution() {
    use std::path::Path;

    let temp = tempdir().expect("create temp dir");
    let project_dir = temp.path().join("crlf_app");

    let names = ProjectNames::new("crlf_app").unwrap();
    let engine = ScaffoldEngine::new(&names, &project_dir);

    let mut crlf_cargo = "[package]\r\nname = \"ruxt\"\r\nversion = \"0.1.0\"\r\n".to_string();
    engine.apply_token_substitutions(Path::new("Cargo.toml"), &mut crlf_cargo);

    assert!(
        crlf_cargo.contains("name = \"crlf-app\""),
        "CRLF Cargo.toml must substitute 'name = \"crlf-app\"', got:\n{}",
        crlf_cargo
    );
}
