use crorust::download::catalog::{ArchiveKind, ExtractionPolicy, ToolArtifact, ToolCategory};
use crorust::download::manager::ToolManager;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_verify_installed_tool_fails_if_marker_missing() {
    let temp = TempDir::new().unwrap();
    let cache_dir = temp.path().join("cache");
    let mgr =
        ToolManager::with_cache_dir(cache_dir.clone(), "x86_64-unknown-linux-gnu", None, false)
            .unwrap();

    let artifact = ToolArtifact {
        name: "test-tool",
        category: ToolCategory::Jdk,
        version: "1.0.0",
        archive_kind: ArchiveKind::Zip,
        strip_components: 0,
        extraction_policy: ExtractionPolicy::default(),
        url: "https://example.com/test.zip",
        sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        binary_rel_paths: &["bin/test"],
    };

    let tool_dir = mgr.canonical_install_dir(&artifact);
    fs::create_dir_all(tool_dir.join("bin")).unwrap();
    let bin_path = tool_dir.join("bin").join("test");
    let mut file = File::create(&bin_path).unwrap();
    writeln!(file, "#!/bin/sh\necho ok").unwrap();

    // Missing .complete marker
    let err = mgr.verify_installed_tool(&artifact, &tool_dir);
    assert!(err.is_err(), "Must fail when .complete marker is missing");
    assert!(err
        .unwrap_err()
        .to_string()
        .contains("Missing `.complete` marker"));
}

#[test]
fn test_verify_installed_tool_fails_if_binary_missing() {
    let temp = TempDir::new().unwrap();
    let cache_dir = temp.path().join("cache");
    let mgr =
        ToolManager::with_cache_dir(cache_dir.clone(), "x86_64-unknown-linux-gnu", None, false)
            .unwrap();

    let artifact = ToolArtifact {
        name: "test-tool",
        category: ToolCategory::Jdk,
        version: "1.0.0",
        archive_kind: ArchiveKind::Zip,
        strip_components: 0,
        extraction_policy: ExtractionPolicy::default(),
        url: "https://example.com/test.zip",
        sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        binary_rel_paths: &["bin/missing-bin"],
    };

    let tool_dir = mgr.canonical_install_dir(&artifact);
    fs::create_dir_all(&tool_dir).unwrap();
    fs::write(tool_dir.join(".complete"), b"ok\n").unwrap();

    // Missing binary
    let err = mgr.verify_installed_tool(&artifact, &tool_dir);
    assert!(err.is_err(), "Must fail when expected binary is missing");
    assert!(err
        .unwrap_err()
        .to_string()
        .contains("Required tool binary `bin/missing-bin` is missing"));
}

#[test]
fn test_verify_installed_tool_succeeds_when_complete_and_binaries_present() {
    let temp = TempDir::new().unwrap();
    let cache_dir = temp.path().join("cache");
    let mgr =
        ToolManager::with_cache_dir(cache_dir.clone(), "x86_64-unknown-linux-gnu", None, false)
            .unwrap();

    let artifact = ToolArtifact {
        name: "test-tool",
        category: ToolCategory::Jdk,
        version: "1.0.0",
        archive_kind: ArchiveKind::Zip,
        strip_components: 0,
        extraction_policy: ExtractionPolicy::default(),
        url: "https://example.com/test.zip",
        sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        binary_rel_paths: &["bin/test"],
    };

    let tool_dir = mgr.canonical_install_dir(&artifact);
    fs::create_dir_all(tool_dir.join("bin")).unwrap();
    let bin_path = tool_dir.join("bin").join("test");
    let mut file = File::create(&bin_path).unwrap();
    writeln!(file, "#!/bin/sh\necho ok").unwrap();
    fs::write(tool_dir.join(".complete"), b"ok\n").unwrap();

    let res = mgr.verify_installed_tool(&artifact, &tool_dir);
    assert!(res.is_ok(), "Verification should succeed: {:?}", res.err());
}
