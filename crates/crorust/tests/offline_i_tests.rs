use crorust::commands::i::SystemIEngine;
use crorust::core::target::{TargetArch, TargetPlatform};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_i_offline_audit_and_sideload_generation() {
    let temp = TempDir::new().expect("Failed to create tempdir");
    let fake_cache = temp.path().join(".cache/crorust");
    fs::create_dir_all(&fake_cache).expect("Failed to create fake cache dir");

    let sideload_script_path = temp.path().join("sideload_test.sh");

    let engine = SystemIEngine::default();
    let cache_roots = vec![fake_cache];

    let is_ready = engine
        .audit_offline_readiness(
            &cache_roots,
            TargetPlatform::Linux,
            TargetArch::X64,
            Some(21),
            true,
            Some(&sideload_script_path),
        )
        .expect("Offline audit should execute without I/O error");

    assert!(
        !is_ready,
        "Empty cache must not be ready for offline compilation"
    );
    assert!(
        sideload_script_path.exists(),
        "Sideload bash script must have been generated"
    );

    let script_content =
        fs::read_to_string(&sideload_script_path).expect("Failed to read generated script");
    assert!(script_content.contains("#!/usr/bin/env bash"));
    assert!(script_content.contains("curl -Lo"));
    assert!(script_content.contains("sha256sum -c -"));
    assert!(script_content.contains("desugar_jdk_libs"));
    assert!(script_content.contains("desugar_jdk_libs_configuration"));
    assert!(
        script_content.contains("d7604feeef3a771d5d616b68fcc2c42f516243311ea286cfd41e1962829a6d28")
    );

    let ps1_script_path = sideload_script_path.with_extension("ps1");
    assert!(
        ps1_script_path.exists(),
        "Sideload PowerShell script (.ps1) must have been generated"
    );
    let ps1_content = fs::read_to_string(&ps1_script_path).unwrap();
    assert!(ps1_content.contains("Invoke-WebRequest"));
    assert!(ps1_content.contains("Get-FileHash -Path"));
    assert!(ps1_content.contains("-Algorithm SHA256"));
}

#[test]
fn test_offline_i_reports_ready_when_cache_is_complete() {
    let temp = TempDir::new().unwrap();
    let cache_dir = temp.path().join(".cache").join("crorust");
    fs::create_dir_all(&cache_dir).unwrap();

    let engine = SystemIEngine::default();

    let stage_tool = |cat: &str, name: &str, ver: &str| {
        let p = cache_dir.join(cat).join(name).join(ver);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join(".complete"), b"ok").unwrap();
    };

    stage_tool("tools", "jdk", "17.0.10_7");
    stage_tool("tools", "r8", "9.5.26");
    stage_tool("tools", "kotlinc", "2.1.0");
    stage_tool("tools", "platform-tools", "35.0.0");
    stage_tool("tools", "ndk", "26.3.11579264");
    stage_tool("tools", "desugar_jdk_libs", "2.1.4");
    stage_tool("tools", "desugar_jdk_libs_configuration", "2.1.4");

    let is_ready = engine
        .audit_offline_readiness(
            std::slice::from_ref(&cache_dir),
            TargetPlatform::Linux,
            TargetArch::X64,
            Some(21),
            false,
            None,
        )
        .unwrap();

    assert!(is_ready, "Fully staged cache must report 100% READY");
}
