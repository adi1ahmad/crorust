use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
#[ignore = "Requires nightly toolchain installed; covered by Tier 3 E2E CI"]
fn test_template_scaffold_and_compile() {
    let temp = tempdir().expect("Failed to create temporary directory");
    let project_dir = temp.path().join("dogfood_test_app");

    // 1. Run crorust template init
    crorust::commands::template::init(project_dir.to_str().expect("valid path"))
        .expect("crorust template init must succeed");

    // 2. Verify Cargo.toml was customized
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let cargo_toml_content =
        fs::read_to_string(&cargo_toml_path).expect("Failed to read scaffolded Cargo.toml");
    assert!(
        cargo_toml_content.contains("name = \"dogfood-test-app\""),
        "Cargo.toml must define package name 'dogfood-test-app', got:\n{cargo_toml_content}"
    );
    assert!(
        cargo_toml_content.contains("name = \"dogfood_test_app\""),
        "Cargo.toml must define lib/bin name 'dogfood_test_app', got:\n{cargo_toml_content}"
    );

    // 3. Verify newly scaffolded project passes cargo check
    let mut check_cmd = Command::new("cargo");
    check_cmd.arg("check").current_dir(&project_dir);

    // Isolate child cargo from parent cargo test harness env variables
    for (k, _) in std::env::vars() {
        if k.starts_with("CARGO") || k.starts_with("RUST") {
            check_cmd.env_remove(&k);
        }
    }

    let check_status = check_cmd
        .status()
        .expect("Failed to execute cargo check on scaffolded project");

    assert!(
        check_status.success(),
        "Scaffolded project failed cargo check"
    );
}
