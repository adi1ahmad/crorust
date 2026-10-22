use crorust::download::catalog::ToolCatalog;

#[test]
fn test_desugar_catalog_artifacts() {
    let libs = ToolCatalog::get_desugar_jdk_libs("2.1.4")
        .expect("desugar_jdk_libs 2.1.4 artifact must be present in catalog");
    assert_eq!(libs.version, "2.1.4");
    assert_eq!(libs.name, "desugar_jdk_libs");
    assert_eq!(
        libs.sha256,
        "d7604feeef3a771d5d616b68fcc2c42f516243311ea286cfd41e1962829a6d28"
    );
    assert!(libs.url.ends_with("desugar_jdk_libs-2.1.4.jar"));

    let config = ToolCatalog::get_desugar_jdk_libs_config("2.1.4")
        .expect("desugar_jdk_libs_configuration 2.1.4 artifact must be present in catalog");
    assert_eq!(config.version, "2.1.4");
    assert_eq!(config.name, "desugar_jdk_libs_configuration");
    assert_eq!(
        config.sha256,
        "5d770c1ba225c2cdf09086bf662954ae765ee481f901b02cd7b1dc3c8077c3f4"
    );
    assert!(config
        .url
        .ends_with("desugar_jdk_libs_configuration-2.1.4.jar"));

    let all_artifacts = ToolCatalog::all_known_artifacts();
    assert!(
        all_artifacts.iter().any(|a| a.name == "desugar_jdk_libs"),
        "desugar_jdk_libs must be included in all_known_artifacts()"
    );
    assert!(
        all_artifacts
            .iter()
            .any(|a| a.name == "desugar_jdk_libs_configuration"),
        "desugar_jdk_libs_configuration must be included in all_known_artifacts()"
    );

    let expected_libs_hash =
        ToolCatalog::expected_sha256_for_filename("desugar_jdk_libs-2.1.4.jar");
    assert_eq!(
        expected_libs_hash,
        Some("d7604feeef3a771d5d616b68fcc2c42f516243311ea286cfd41e1962829a6d28")
    );
}

#[test]
fn test_r8_catalog_contains_modern_compiler() {
    let r8_artifact = ToolCatalog::get_r8("9.5.26")
        .expect("r8 9.5.26 must be present in catalog for modern desugaring and metadata parsing");
    assert_eq!(r8_artifact.name, "r8");
    assert_eq!(r8_artifact.version, "9.5.26");
    assert!(r8_artifact.url.ends_with("r8-9.5.26.jar"));
}
