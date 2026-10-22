use anyhow::Result;
use crorust_mvn::{
    DependencyRequirement, Download, MavenCoordinates, RepositoryClient, SemverVersion,
};
use std::path::Path;
use std::str::FromStr;

struct OfflineFixtureDownloader;

impl Download for OfflineFixtureDownloader {
    fn download(&self, url: &str, _dest: &Path) -> Result<()> {
        anyhow::bail!("Unexpected network call in offline test: {url}")
    }
}

fn write_metadata(dir: &Path, group: &str, artifact: &str, versions: &[&str]) -> Result<()> {
    let latest = versions.last().copied().unwrap_or("1.0.0");
    let vers_xml: String = versions
        .iter()
        .map(|v| format!("<version>{v}</version>"))
        .collect();
    let xml = format!(
        "<metadata><groupId>{group}</groupId><artifactId>{artifact}</artifactId>\
         <versioning><latest>{latest}</latest><release>{latest}</release>\
         <versions>{vers_xml}</versions></versioning></metadata>"
    );
    std::fs::write(dir.join(format!("{group}-{artifact}.metadata.xml")), xml)?;
    Ok(())
}

fn write_pom_and_jar(
    dir: &Path,
    group: &str,
    artifact: &str,
    version: &str,
    deps: &[(&str, &str, &str)],
) -> Result<()> {
    let deps_xml: String = deps
        .iter()
        .map(|(g, a, v)| {
            format!("<dependency><groupId>{g}</groupId><artifactId>{a}</artifactId><version>{v}</version></dependency>")
        })
        .collect();
    let pom = format!(
        "<project><groupId>{group}</groupId><artifactId>{artifact}</artifactId>\
         <version>{version}</version><packaging>jar</packaging>\
         <dependencies>{deps_xml}</dependencies></project>"
    );
    std::fs::write(dir.join(format!("{group}-{artifact}-{version}.pom")), pom)?;
    std::fs::write(
        dir.join(format!("{group}-{artifact}-{version}.jar")),
        b"PK\x03\x04dummy_jar",
    )?;
    Ok(())
}

#[test]
fn test_pubgrub_diamond_dependency_resolution() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let cache = temp_dir.path();

    write_metadata(cache, "com.example", "lib-a", &["1.0.0"])?;
    write_pom_and_jar(
        cache,
        "com.example",
        "lib-a",
        "1.0.0",
        &[("com.example", "common", "[1.0.0,2.0.0)")],
    )?;

    write_metadata(cache, "com.example", "lib-b", &["1.0.0"])?;
    write_pom_and_jar(
        cache,
        "com.example",
        "lib-b",
        "1.0.0",
        &[("com.example", "common", "[1.5.0,1.8.0]")],
    )?;

    write_metadata(
        cache,
        "com.example",
        "common",
        &["1.0.0", "1.5.0", "1.7.0", "1.9.0"],
    )?;
    for ver in ["1.0.0", "1.5.0", "1.7.0", "1.9.0"] {
        write_pom_and_jar(cache, "com.example", "common", ver, &[])?;
    }

    let mut client = RepositoryClient::new(cache.to_path_buf(), OfflineFixtureDownloader)?;
    client.add_default_repositories();

    let pkg_root = MavenCoordinates::new("com.example", "root");
    let v1_0 = SemverVersion::from_str("1.0.0")?;
    client.add_package(
        pkg_root.clone(),
        v1_0.clone(),
        vec![
            DependencyRequirement::from_str("com.example:lib-a:1.0.0")?,
            DependencyRequirement::from_str("com.example:lib-b:1.0.0")?,
        ],
    )?;

    let resolved = client.resolve(pkg_root, v1_0)?;
    let file_names: Vec<String> = resolved
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();

    assert_eq!(file_names.len(), 3);
    assert!(file_names.contains(&"com.example-lib-a-1.0.0.jar".to_string()));
    assert!(file_names.contains(&"com.example-lib-b-1.0.0.jar".to_string()));
    assert!(file_names.contains(&"com.example-common-1.7.0.jar".to_string()));

    Ok(())
}

fn write_pom_and_aar(
    dir: &Path,
    group: &str,
    artifact: &str,
    version: &str,
    deps: &[(&str, &str, &str)],
    has_classes_jar: bool,
) -> Result<()> {
    let deps_xml: String = deps
        .iter()
        .map(|(g, a, v)| {
            format!("<dependency><groupId>{g}</groupId><artifactId>{a}</artifactId><version>{v}</version></dependency>")
        })
        .collect();
    let pom = format!(
        "<project><groupId>{group}</groupId><artifactId>{artifact}</artifactId>\
         <version>{version}</version><packaging>aar</packaging>\
         <dependencies>{deps_xml}</dependencies></project>"
    );
    std::fs::write(dir.join(format!("{group}-{artifact}-{version}.pom")), pom)?;

    let aar_path = dir.join(format!("{group}-{artifact}-{version}.aar"));
    let mut zip = crorust_common::BoundaryZipArchive::new(&aar_path, false)?;
    zip.create_file(
        Path::new("AndroidManifest.xml"),
        crorust_common::ArchiveEntryOptions::Unaligned,
        b"<manifest package=\"com.example\" />",
    )?;

    if has_classes_jar {
        zip.create_file(
            Path::new("classes.jar"),
            crorust_common::ArchiveEntryOptions::Unaligned,
            b"PK\x03\x04dummy_classes",
        )?;
    }

    Ok(())
}

#[test]
fn test_aar_resolution_with_and_without_classes_jar() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let cache = temp_dir.path();

    write_metadata(cache, "androidx.appcompat", "appcompat", &["1.6.1"])?;
    write_pom_and_aar(cache, "androidx.appcompat", "appcompat", "1.6.1", &[], true)?;

    write_metadata(
        cache,
        "androidx.lifecycle",
        "lifecycle-runtime-ktx",
        &["2.11.0"],
    )?;
    write_pom_and_aar(
        cache,
        "androidx.lifecycle",
        "lifecycle-runtime-ktx",
        "2.11.0",
        &[],
        false,
    )?;

    let mut client = RepositoryClient::new(cache.to_path_buf(), OfflineFixtureDownloader)?;
    client.add_default_repositories();

    let root_pkg = MavenCoordinates::new("com.example", "app");
    let root_ver = SemverVersion::from_str("1.0.0")?;
    client.add_package(
        root_pkg.clone(),
        root_ver.clone(),
        vec![
            DependencyRequirement::from_str("androidx.appcompat:appcompat:1.6.1")?,
            DependencyRequirement::from_str("androidx.lifecycle:lifecycle-runtime-ktx:2.11.0")?,
        ],
    )?;

    let resolved = client.resolve(root_pkg, root_ver)?;
    let file_names: Vec<String> = resolved
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();

    assert_eq!(file_names.len(), 2);
    assert!(file_names.contains(&"androidx.appcompat-appcompat-1.6.1.aar".to_string()));
    assert!(file_names.contains(&"androidx.lifecycle-lifecycle-runtime-ktx-2.11.0.aar".to_string()));

    assert!(cache
        .join("androidx.appcompat-appcompat-1.6.1.jar")
        .exists());

    assert!(!cache
        .join("androidx.lifecycle-lifecycle-runtime-ktx-2.11.0.jar")
        .exists());

    Ok(())
}

struct MockBatchDownloader {
    pub batch_invocations: std::sync::atomic::AtomicUsize,
}

impl Download for MockBatchDownloader {
    fn download(&self, url: &str, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if url.ends_with(".pom") {
            let pom_xml = "<project><groupId>test</groupId><artifactId>lib</artifactId><version>1.0.0</version><packaging>aar</packaging></project>";
            std::fs::write(dest, pom_xml)?;
        } else if url.ends_with(".aar") {
            let mut zip = crorust_common::BoundaryZipArchive::new(dest, false)?;
            zip.create_file(
                Path::new("classes.jar"),
                crorust_common::ArchiveEntryOptions::Unaligned,
                b"Mock Dalvik Bytecode",
            )?;
        } else {
            std::fs::write(dest, b"Mock jar bytes")?;
        }
        Ok(())
    }

    fn download_batch(&self, items: &[crorust_mvn::DownloadBatchItem]) -> Result<()> {
        self.batch_invocations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        for item in items {
            self.download(&item.url, &item.dest)?;
        }
        Ok(())
    }
}

#[test]
fn test_parallel_batch_artifact_download_and_aar_jar_extraction() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let cache_dir = tmp.path().join("cache");
    std::fs::create_dir_all(&cache_dir)?;

    // Write metadata fixture so PubGrub finds version 1.0.0
    let meta_xml = "<metadata><groupId>test</groupId><artifactId>lib</artifactId><versioning><release>1.0.0</release><versions><version>1.0.0</version></versions></versioning></metadata>";
    std::fs::write(cache_dir.join("test-lib.metadata.xml"), meta_xml)?;

    let downloader = MockBatchDownloader {
        batch_invocations: std::sync::atomic::AtomicUsize::new(0),
    };
    let mut client = RepositoryClient::new(cache_dir.clone(), downloader)?;
    client.add_repository("https://repo1.maven.org/maven2");

    let root_pkg = MavenCoordinates::new("test", "root");
    let root_ver = SemverVersion::from_str("1.0.0")?;
    let dep_req = DependencyRequirement::from_str("test:lib:1.0.0")?;
    client.add_package(root_pkg.clone(), root_ver.clone(), vec![dep_req])?;

    // Execute parallel resolution
    let resolved_files = client.resolve(root_pkg, root_ver)?;
    assert!(
        !resolved_files.is_empty(),
        "Must resolve at least one artifact"
    );

    // Verify AAR artifact was retrieved
    let expected_aar = cache_dir.join("test-lib-1.0.0.aar");
    assert!(
        expected_aar.exists(),
        "AAR file must exist in cache directory"
    );

    // Verify embedded classes.jar was extracted in parallel
    let expected_classes_jar = cache_dir.join("test-lib-1.0.0.jar");
    assert!(
        expected_classes_jar.exists(),
        "Embedded classes.jar must be extracted from AAR"
    );
    let extracted_content = std::fs::read_to_string(&expected_classes_jar)?;
    assert_eq!(extracted_content, "Mock Dalvik Bytecode");

    Ok(())
}
