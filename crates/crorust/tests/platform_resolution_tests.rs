use clap::Parser;
use crorust::config::ManifestConfig;
use crorust::download::catalog::ToolCatalog;
use crorust::download::dynamic_platform::{parse_platform_from_xml, PlatformResolutionCache};
use crorust::ProjectBuildArguments;

#[test]
fn test_pinned_platform_catalog_integrity() {
    let p34 = ToolCatalog::get_android_platform(34).expect("API 34 must be in catalog");
    assert_eq!(p34.archive_filename(), "platform-34-ext7_r03.zip");
    assert_eq!(
        p34.sha256,
        "16fdb74c55e59ae3ef52def135aec713508467bd56d7dabcd8c9be31fa8b20f3"
    );
    assert_eq!(p34.strip_components, 1);
    assert!(p34.binary_rel_paths.contains(&"android.jar"));

    let p35 = ToolCatalog::get_android_platform(35).expect("API 35 must be in catalog");
    assert_eq!(p35.archive_filename(), "platform-35_r02.zip");
    assert_eq!(
        p35.sha256,
        "0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0"
    );
    assert_eq!(p35.strip_components, 1);
    assert!(p35.binary_rel_paths.contains(&"android.jar"));

    // Expected sha256 lookups
    assert_eq!(
        ToolCatalog::expected_sha256_for_filename("platform-35_r02.zip"),
        Some("0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0")
    );
    assert_eq!(
        ToolCatalog::expected_sha256_for_filename("platform-35_r01.zip"),
        Some("ec20a0a65704e1b2554f8e3eebf588ef260569673c848c01aff2e29c28734cf4")
    );
}

#[test]
fn test_option_a_manifest_override_parsing() {
    let toml = r#"
[toolchain.android.platform.35]
url = "https://custom.internal.repo/platform-35.zip"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[toolchain.android.platform.36]
url = "https://custom.internal.repo/platform-36.zip"
"#;

    let cfg: ManifestConfig = toml::from_str(toml).expect("Must parse manifest overrides");
    let o35 = cfg
        .toolchain
        .get_platform_override(35)
        .expect("Override 35 must exist");
    assert_eq!(o35.url, "https://custom.internal.repo/platform-35.zip");
    assert_eq!(
        o35.sha256.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );

    let o36 = cfg
        .toolchain
        .get_platform_override(36)
        .expect("Override 36 must exist");
    assert_eq!(o36.url, "https://custom.internal.repo/platform-36.zip");
    assert_eq!(o36.sha256, None);
}

#[test]
fn test_option_a_cli_skip_verification_flag() {
    let args = ProjectBuildArguments::try_parse_from([
        "crorust",
        "-p",
        "android",
        "--skip-artifact-verification",
    ])
    .expect("Must parse CLI flags");

    assert!(args.cargo.skip_artifact_verification);
}

#[test]
fn test_option_b_dynamic_xml_resolution_and_caching() {
    let sample_xml = r#"
<sdk:sdk-repository xmlns:sdk="http://schemas.android.com/sdk/android/repository/11">
  <remotePackage path="platforms;android-35">
    <type-details><api-level>35</api-level></type-details>
    <revision><major>2</major></revision>
    <archives>
      <archive>
        <complete>
          <size>64273788</size>
          <checksum>0bb560a90a7a2cbd0dd8348224d518b638fe7949</checksum>
          <url>platform-35_r02.zip</url>
        </complete>
      </archive>
    </archives>
  </remotePackage>
</sdk:sdk-repository>
"#;

    let res = parse_platform_from_xml(sample_xml, 35, None).expect("Should parse API 35");
    assert_eq!(res.api_level, 35);
    assert_eq!(res.revision, 2);
    assert_eq!(res.checksum, "0bb560a90a7a2cbd0dd8348224d518b638fe7949");
    assert_eq!(res.checksum_type, "sha1");
    assert_eq!(
        res.url,
        "https://dl.google.com/android/repository/platform-35_r02.zip"
    );

    // Test local resolution cache serialization
    let temp_dir = tempfile::tempdir().unwrap();
    let cache_file = temp_dir.path().join("android_platform_cache.toml");

    let mut cache = PlatformResolutionCache::default();
    cache.platforms.insert("35".to_string(), res.clone());
    cache.save(&cache_file).unwrap();

    let loaded = PlatformResolutionCache::load(&cache_file);
    assert_eq!(loaded.platforms.get("35"), Some(&res));
}
