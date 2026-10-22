use crorust::download::catalog::{ArchiveKind, ExtractionPolicy, ToolArtifact, ToolCategory};

#[test]
fn test_tool_artifact_resolved_url_without_mirror() {
    let artifact = ToolArtifact {
        category: ToolCategory::PlatformTools,
        name: "platform-tools",
        version: "35.0.1",
        url: "https://dl.google.com/android/repository/platform-tools_r35.0.1-linux.zip",
        sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        archive_kind: ArchiveKind::Zip,
        strip_components: 1,
        binary_rel_paths: &["adb"],
        extraction_policy: ExtractionPolicy::default(),
    };

    let resolved = artifact.resolved_url(None);
    assert_eq!(
        resolved, "https://dl.google.com/android/repository/platform-tools_r35.0.1-linux.zip",
        "Without mirror, URL must match upstream official URL exactly"
    );
}

#[test]
fn test_tool_artifact_resolved_url_with_mirror_handles_trailing_slash() {
    let artifact = ToolArtifact {
        category: ToolCategory::Gradle,
        name: "gradle",
        version: "8.7",
        url: "https://services.gradle.org/distributions/gradle-8.7-bin.zip",
        sha256: "544c35d6bd849ae8a5ed0bced3134f37803e1cbcae4348a27bc52b2f689725f1",
        archive_kind: ArchiveKind::Zip,
        strip_components: 1,
        binary_rel_paths: &["bin/gradle"],
        extraction_policy: ExtractionPolicy::default(),
    };

    let mirror_with_slash = "https://artifactory.corp.internal/dist/";
    assert_eq!(
        artifact.resolved_url(Some(mirror_with_slash)),
        "https://artifactory.corp.internal/dist/gradle-8.7-bin.zip",
        "Must correctly strip trailing slash and append filename"
    );

    let mirror_no_slash = "https://artifactory.corp.internal/dist";
    assert_eq!(
        artifact.resolved_url(Some(mirror_no_slash)),
        "https://artifactory.corp.internal/dist/gradle-8.7-bin.zip",
        "Must append filename correctly without trailing slash"
    );
}

#[test]
fn test_offline_mode_error_message_is_actionable() {
    let dir = tempfile::tempdir().unwrap();
    let tm = crorust::download::manager::ToolManager::with_cache_dir(
        dir.path().to_path_buf(),
        "x86_64-unknown-linux-gnu",
        Some("https://mirror.corp.internal/toolchains".to_string()),
        true,
    )
    .unwrap();
    let artifact = crorust::download::catalog::ToolCatalog::get_gradle("8.8").unwrap();

    let err = tm
        .ensure_tool(&artifact)
        .expect_err("Offline ToolManager must fail when tool is not installed in cache");
    let err_msg = format!("{err:#}");

    assert!(err_msg.contains("Offline build failed"));
    assert!(err_msg.contains("gradle"));
    assert!(err_msg.contains("8.8"));
    assert!(err_msg.contains(&dir.path().display().to_string()));
}

#[test]
fn test_ndk_version_parsing_and_compatibility() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ndk_root = temp_dir.path().join("android-ndk-r29");
    std::fs::create_dir_all(&ndk_root).unwrap();

    let props = r#"Pkg.Desc = Android NDK
Pkg.Revision = 29.0.14206865
Pkg.BaseRevision = 29.0.14206865
Pkg.ReleaseName = r29
"#;
    std::fs::write(ndk_root.join("source.properties"), props).unwrap();

    let info = crorust::download::catalog::read_ndk_version(&ndk_root).unwrap();
    assert_eq!(info.revision.as_deref(), Some("29.0.14206865"));
    assert_eq!(info.release_name.as_deref(), Some("r29"));

    assert!(crorust::download::catalog::is_ndk_version_compatible(
        &ndk_root,
        Some("29")
    ));
    assert!(crorust::download::catalog::is_ndk_version_compatible(
        &ndk_root,
        Some("r29")
    ));
    assert!(crorust::download::catalog::is_ndk_version_compatible(
        &ndk_root,
        Some("29.0.14206865")
    ));
    assert!(crorust::download::catalog::is_ndk_version_compatible(
        &ndk_root, None
    ));

    assert!(!crorust::download::catalog::is_ndk_version_compatible(
        &ndk_root,
        Some("26.3.11579264")
    ));
    assert!(!crorust::download::catalog::is_ndk_version_compatible(
        &ndk_root,
        Some("r26")
    ));
}

#[test]
fn test_google_ndk_sysroot_normalization() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ndk_root = temp_dir.path().join("google-ndk");

    let sysroot_usr_lib = ndk_root
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join("linux-x86_64")
        .join("sysroot")
        .join("usr")
        .join("lib");
    std::fs::create_dir_all(&sysroot_usr_lib).unwrap();

    let resolved = crorust::resolve_ndk_sysroot(&ndk_root);
    let expected = ndk_root
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join("linux-x86_64")
        .join("sysroot");
    assert_eq!(resolved, expected);

    let standalone_root = temp_dir.path().join("standalone-ndk");
    std::fs::create_dir_all(standalone_root.join("usr").join("lib")).unwrap();

    let resolved_standalone = crorust::resolve_ndk_sysroot(&standalone_root);
    assert_eq!(resolved_standalone, standalone_root);
}

#[test]
fn test_jdk_and_platform_tools_version_parsing() {
    let temp_dir = tempfile::tempdir().unwrap();

    let jdk_root = temp_dir.path().join("jdk-17");
    std::fs::create_dir_all(&jdk_root).unwrap();
    let jdk_release = r#"IMPLEMENTOR="Adoptium"
JAVA_RUNTIME_VERSION="17.0.10+7"
JAVA_VERSION="17.0.10"
OS_ARCH="x86_64"
"#;
    std::fs::write(jdk_root.join("release"), jdk_release).unwrap();
    assert_eq!(
        crorust::download::catalog::read_jdk_version(&jdk_root).as_deref(),
        Some("17.0.10")
    );

    let pt_root = temp_dir.path().join("platform-tools");
    std::fs::create_dir_all(&pt_root).unwrap();
    let pt_props = r#"Pkg.UserSrc=false
Pkg.Revision=35.0.1
"#;
    std::fs::write(pt_root.join("source.properties"), pt_props).unwrap();
    assert_eq!(
        crorust::download::catalog::read_platform_tools_version(&pt_root).as_deref(),
        Some("35.0.1")
    );
}
