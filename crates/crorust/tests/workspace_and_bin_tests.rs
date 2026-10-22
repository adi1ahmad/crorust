use clap::Parser;
use crorust::cargo::manifest::CargoPackageManifest;
use crorust::cargo::{CargoWorkspaceSession, CompilationCrateKind};
use crorust::config::{
    ManifestConfig, DEFAULT_AGP_VERSION, DEFAULT_IOS_MIN, DEFAULT_JVM_TARGET,
    DEFAULT_KOTLIN_VERSION, DEFAULT_MACOS_MIN, DEFAULT_WIN_SDK,
};
use crorust::{
    CargoPipelineArguments, TargetArch, TargetCompilationUnit, TargetOpt, TargetPlatform,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_virtual_workspace_default_member_resolution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["app", "lib-crate"]
default-members = ["app"]
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("app/src")).unwrap();
    fs::write(
        root.join("app/Cargo.toml"),
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("app/src/main.rs"), "fn main() {}").unwrap();

    fs::create_dir_all(root.join("lib-crate/src")).unwrap();
    fs::write(
        root.join("lib-crate/Cargo.toml"),
        r#"
[package]
name = "lib-crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("lib-crate/src/lib.rs"), "pub fn foo() {}").unwrap();

    let cargo = CargoWorkspaceSession::new(
        None,
        None,
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap();

    assert_eq!(cargo.package_identifier(), "app");
    assert_eq!(cargo.selected_bin(), Some("app"));
    assert_eq!(
        cargo.package_root_path(),
        dunce::canonicalize(root.join("app")).unwrap()
    );
}

#[test]
fn test_workspace_manifest_yaml_package_specification() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["desktop-client", "backend-server"]
"#,
    )
    .unwrap();

    fs::write(
        root.join("manifest.toml"),
        r#"
package = "desktop-client"
bin = "desktop-ui"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("desktop-client/src")).unwrap();
    fs::write(
        root.join("desktop-client/Cargo.toml"),
        r#"
[package]
name = "desktop-client"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "desktop-ui"
path = "src/main.rs"
"#,
    )
    .unwrap();
    fs::write(root.join("desktop-client/src/main.rs"), "fn main() {}").unwrap();

    fs::create_dir_all(root.join("backend-server/src")).unwrap();
    fs::write(
        root.join("backend-server/Cargo.toml"),
        r#"
[package]
name = "backend-server"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("backend-server/src/main.rs"), "fn main() {}").unwrap();

    let cargo = CargoWorkspaceSession::new(
        None,
        None,
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap();

    assert_eq!(cargo.package_identifier(), "desktop-client");
    assert_eq!(cargo.selected_bin(), Some("desktop-ui"));
}

#[test]
fn test_custom_bin_name_auto_detection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "game-core"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "supergame"
path = "src/main.rs"
"#,
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let cargo = CargoWorkspaceSession::new(
        None,
        None,
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap();

    assert_eq!(cargo.package_identifier(), "game-core");
    assert_eq!(cargo.selected_bin(), Some("supergame"));

    let host_platform = TargetPlatform::detect_host().unwrap();
    let host_arch = TargetArch::detect_host().unwrap();
    let target = TargetCompilationUnit::new(host_platform, host_arch, TargetOpt::Debug);

    let bin_filename = if cfg!(windows) {
        "supergame.exe"
    } else {
        "supergame"
    };

    let fake_target_dir = root.join("target");
    let host_opt_dir = fake_target_dir.join("debug");
    fs::create_dir_all(&host_opt_dir).unwrap();
    fs::write(host_opt_dir.join(bin_filename), "").unwrap();

    let bin_path = cargo
        .resolve_compiled_artifact_path(&fake_target_dir, target, None, CompilationCrateKind::Bin)
        .unwrap();
    assert_eq!(bin_path, host_opt_dir.join(bin_filename));
}

#[test]
fn test_cross_target_artifact_path_resolution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "cross-game"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "crossgame"
path = "src/main.rs"
"#,
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let cargo = CargoWorkspaceSession::new(
        None,
        None,
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap();

    // Use a target triple that is guaranteed to be cross-compiled (wasm32-unknown-unknown):
    let target = TargetCompilationUnit::new(TargetPlatform::Web, TargetArch::X64, TargetOpt::Debug);
    let fake_target_dir = root.join("target");
    let triple = target.rust_target_triple().unwrap();
    let cross_opt_dir = fake_target_dir.join(triple).join("debug");
    fs::create_dir_all(&cross_opt_dir).unwrap();
    fs::write(cross_opt_dir.join("crossgame.wasm"), "").unwrap();

    let bin_path = cargo
        .resolve_compiled_artifact_path(&fake_target_dir, target, None, CompilationCrateKind::Bin)
        .unwrap();
    assert_eq!(bin_path, cross_opt_dir.join("crossgame.wasm"));
}

#[test]
fn test_explicit_bin_flag_selection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "multi-tool"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "tool-a"
path = "src/a.rs"

[[bin]]
name = "tool-b"
path = "src/b.rs"
"#,
    )
    .unwrap();
    fs::write(root.join("src/a.rs"), "fn main() {}").unwrap();
    fs::write(root.join("src/b.rs"), "fn main() {}").unwrap();

    let cargo = CargoWorkspaceSession::new(
        None,
        Some("tool-b"),
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap();

    assert_eq!(cargo.package_identifier(), "multi-tool");
    assert_eq!(cargo.selected_bin(), Some("tool-b"));
}

#[test]
fn test_invalid_bin_flag_error_diagnostic() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "single-tool"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "real-bin"
path = "src/main.rs"
"#,
    )
    .unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let err = CargoWorkspaceSession::new(
        None,
        Some("nonexistent"),
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("binary `nonexistent` not found in package `single-tool`"),
        "Unexpected error: {msg}"
    );
    assert!(msg.contains("real-bin"), "Available bins missing in: {msg}");
}

#[test]
fn test_ambiguous_virtual_workspace_error_diagnostic() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["alpha", "beta"]
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("alpha/src")).unwrap();
    fs::write(
        root.join("alpha/Cargo.toml"),
        r#"
[package]
name = "alpha"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("alpha/src/main.rs"), "fn main() {}").unwrap();

    fs::create_dir_all(root.join("beta/src")).unwrap();
    fs::write(
        root.join("beta/Cargo.toml"),
        r#"
[package]
name = "beta"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(root.join("beta/src/main.rs"), "fn main() {}").unwrap();

    let err = CargoWorkspaceSession::new(
        None,
        None,
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )
    .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("Virtual workspace root detected"),
        "Unexpected error: {msg}"
    );
    assert!(
        msg.contains("alpha, beta") || (msg.contains("alpha") && msg.contains("beta")),
        "Packages missing in: {msg}"
    );
}

#[test]
fn test_external_manifest_path_outside_current_workspace_does_not_panic() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let root = dir.path();

    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "isolated-external-app"
version = "0.1.0"
edition = "2021"
"#,
    )?;
    fs::write(root.join("src/main.rs"), "fn main() {}")?;

    let cargo = CargoWorkspaceSession::new(
        None,
        None,
        vec![],
        false,
        Some(root.join("Cargo.toml")),
        None,
        true,
    )?;

    assert_eq!(cargo.package_identifier(), "isolated-external-app");
    assert_eq!(cargo.selected_bin(), Some("isolated-external-app"));
    Ok(())
}

#[test]
fn test_package_can_override_workspace_back_to_defaults() {
    let ws_toml = r#"
[toolchain]
offline = true

[android]
agp_version = "8.5.0"
kotlin_version = "2.0.0"
jvm_target = "21"
game_activity = true

[macos]
minimum_system_version = "13.0"

[ios]
minimum_os_version = "17.0"

[windows]
sdk_version = "10.0.19041.0"
"#;
    let mut ws_config: ManifestConfig = toml::from_str(ws_toml).unwrap();

    assert_eq!(ws_config.android().agp_version(), "8.5.0");
    assert_eq!(ws_config.android().kotlin_version(), "2.0.0");
    assert_eq!(ws_config.android().jvm_target(), "21");
    assert!(ws_config.android().game_activity());
    assert!(ws_config.toolchain().offline());
    assert_eq!(ws_config.macos().minimum_system_version(), "13.0");
    assert_eq!(ws_config.ios().minimum_os_version(), "17.0");
    assert_eq!(ws_config.windows().sdk_version(), "10.0.19041.0");

    let pkg_toml = format!(
        r#"
[toolchain]
offline = false

[android]
agp_version = "{DEFAULT_AGP_VERSION}"
kotlin_version = "{DEFAULT_KOTLIN_VERSION}"
jvm_target = "{DEFAULT_JVM_TARGET}"
game_activity = false

[macos]
minimum_system_version = "{DEFAULT_MACOS_MIN}"

[ios]
minimum_os_version = "{DEFAULT_IOS_MIN}"

[windows]
sdk_version = "{DEFAULT_WIN_SDK}"
"#
    );
    let pkg_config: ManifestConfig = toml::from_str(&pkg_toml).unwrap();

    ws_config.merge(pkg_config);

    assert_eq!(ws_config.android().agp_version(), DEFAULT_AGP_VERSION);
    assert_eq!(ws_config.android().kotlin_version(), DEFAULT_KOTLIN_VERSION);
    assert_eq!(ws_config.android().jvm_target(), DEFAULT_JVM_TARGET);
    assert!(!ws_config.android().game_activity());
    assert!(!ws_config.toolchain().offline());
    assert_eq!(
        ws_config.macos().minimum_system_version(),
        DEFAULT_MACOS_MIN
    );
    assert_eq!(ws_config.ios().minimum_os_version(), DEFAULT_IOS_MIN);
    assert_eq!(ws_config.windows().sdk_version(), DEFAULT_WIN_SDK);
}

#[test]
fn test_unspecified_package_fields_inherit_workspace_overrides() {
    let ws_toml = r#"
[android]
agp_version = "8.9.0"
target_sdk = 35
"#;
    let mut ws_config: ManifestConfig = toml::from_str(ws_toml).unwrap();

    let pkg_toml = r#"
[android]
min_sdk = 26
"#;
    let pkg_config: ManifestConfig = toml::from_str(pkg_toml).unwrap();

    ws_config.merge(pkg_config);

    assert_eq!(ws_config.android().agp_version(), "8.9.0");
    assert_eq!(ws_config.android().target_sdk, Some(35));
    assert_eq!(ws_config.android().min_sdk, Some(26));
    assert_eq!(ws_config.android().jvm_target(), DEFAULT_JVM_TARGET);
}

#[test]
fn test_workspace_exclusion_with_globs() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let workspace_root = tmp.path();

    let root_cargo = r#"
[workspace]
members = ["crates/*"]
exclude = ["templates/*", "examples"]
"#;
    fs::write(workspace_root.join("Cargo.toml"), root_cargo)?;

    let template_dir = workspace_root.join("templates").join("my_app");
    fs::create_dir_all(&template_dir)?;
    let template_cargo = r#"
[package]
name = "my_app"
version = "0.1.0"
edition = "2024"
"#;
    fs::write(template_dir.join("Cargo.toml"), template_cargo)?;

    let member_dir = workspace_root.join("crates").join("my_lib");
    fs::create_dir_all(&member_dir)?;
    let member_cargo = r#"
[package]
name = "my_lib"
version = "0.1.0"
edition = "2024"
"#;
    fs::write(member_dir.join("Cargo.toml"), member_cargo)?;

    let manifest: CargoPackageManifest = toml::from_str(root_cargo)?;
    assert!(
        manifest.is_excluded(workspace_root, &template_dir)?,
        "templates/my_app must be recognized as excluded"
    );
    assert!(
        !manifest.is_excluded(workspace_root, &member_dir)?,
        "crates/my_lib must not be recognized as excluded"
    );

    Ok(())
}

#[test]
fn test_template_packages_strictly_omitted_from_workspace_members() -> anyhow::Result<()> {
    let manifest_path = format!("{}/../../Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.manifest_path(manifest_path);

    let metadata = cmd.exec()?;
    for package_id in &metadata.workspace_members {
        let pkg = &metadata[package_id];
        assert!(
            !pkg.manifest_path.to_string().contains("/templates/"),
            "Package '{}' at {} must NOT be a member of the root workspace!",
            pkg.name,
            pkg.manifest_path
        );
    }
    Ok(())
}

#[test]
fn test_project_flag_resolves_directory_manifest() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let root = dunce::canonicalize(tmp.path())?;
    let project_dir = root.join("sample_app");
    fs::create_dir_all(&project_dir)?;
    fs::write(
        project_dir.join("Cargo.toml"),
        "[package]\nname = \"sample_app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;

    let args =
        CargoPipelineArguments::parse_from(["test", "--project", project_dir.to_str().unwrap()]);

    let cargo = args.cargo()?;
    assert_eq!(
        cargo.cargo_manifest_file_path(),
        project_dir.join("Cargo.toml")
    );

    Ok(())
}

#[test]
fn test_project_flag_resolves_file_manifest() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let root = dunce::canonicalize(tmp.path())?;
    let cargo_file = root.join("Cargo.toml");
    fs::write(
        &cargo_file,
        "[package]\nname = \"sample_app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;

    let args =
        CargoPipelineArguments::parse_from(["test", "--project", cargo_file.to_str().unwrap()]);

    let cargo = args.cargo()?;
    assert_eq!(cargo.cargo_manifest_file_path(), cargo_file);

    Ok(())
}

#[test]
fn test_project_flag_rejects_missing_directory() {
    let args = CargoPipelineArguments::parse_from([
        "test",
        "--project",
        "/path/that/definitely/does/not/exist_12345",
    ]);
    assert!(args.cargo().is_err());
}

#[test]
fn test_project_flag_conflicts_with_manifest_path() {
    let result = CargoPipelineArguments::try_parse_from([
        "test",
        "--project",
        "some/dir",
        "--manifest-path",
        "other/Cargo.toml",
    ]);
    assert!(
        result.is_err(),
        "--project and --manifest-path must be mutually exclusive"
    );
}
