use anyhow::Result;
use crorust_mvn::interval::parse_version_constraint;
use crorust_mvn::model::coordinate::{MavenCoordinates, SemverVersion};
use crorust_mvn::model::pom::ProjectObjectModel;
use crorust_mvn::registry::cache::{CachedMetadataEntry, PersistentMavenCache};
use crorust_mvn::registry::client::{Download, RepositoryClient};
use std::path::Path;
use std::str::FromStr;
use tempfile::tempdir;

struct InMemoryMockDownloader;

impl Download for InMemoryMockDownloader {
    fn download(&self, url: &str, dest: &Path) -> Result<()> {
        if url.ends_with("maven-metadata.xml") {
            let xml = r#"<metadata>
                <groupId>androidx.annotation</groupId>
                <artifactId>annotation</artifactId>
                <versioning>
                    <latest>1.8.0</latest>
                    <release>1.8.0</release>
                    <versions>
                        <version>1.7.0</version>
                        <version>1.8.0</version>
                    </versions>
                </versioning>
            </metadata>"#;
            std::fs::write(dest, xml)?;
            return Ok(());
        }
        if url.ends_with(".pom") {
            let pom = r#"<project>
                <groupId>androidx.annotation</groupId>
                <artifactId>annotation</artifactId>
                <version>1.8.0</version>
                <packaging>jar</packaging>
            </project>"#;
            std::fs::write(dest, pom)?;
            return Ok(());
        }
        if url.ends_with(".jar") {
            std::fs::write(dest, b"PK0304mockjar")?;
            return Ok(());
        }
        anyhow::bail!("404 Not Found: {url}")
    }
}

#[test]
fn test_decoupled_coordinate_and_version_ordering() -> Result<()> {
    let coord1 = MavenCoordinates::new("com.google.guava", "guava");
    let coord2 = MavenCoordinates::new("com.google.guava", "guava");
    let coord3 = MavenCoordinates::new("androidx.core", "core");

    assert_eq!(coord1, coord2);
    assert_ne!(coord1, coord3);
    assert_eq!(coord1.to_string(), "com.google.guava:guava");

    let v1 = SemverVersion::from_str("1.0.0")?;
    let v2 = SemverVersion::from_str("1.0.1")?;
    let v_alpha = SemverVersion::from_str("1.0.1-alpha01")?;
    let v_beta = SemverVersion::from_str("1.0.1-beta01")?;

    assert!(v1 < v2);
    assert!(v1 < v_alpha);
    assert!(v_alpha < v_beta);
    assert!(v_beta < v2);

    Ok(())
}

#[test]
fn test_version_constraint_range_evaluations() -> Result<()> {
    let r1 = parse_version_constraint("[1.0.0, 2.0.0)")?;
    assert!(r1.contains(&SemverVersion::from_str("1.0.0")?));
    assert!(r1.contains(&SemverVersion::from_str("1.5.0")?));
    assert!(!r1.contains(&SemverVersion::from_str("2.0.0")?));
    assert!(!r1.contains(&SemverVersion::from_str("0.9.0")?));

    let r_any = parse_version_constraint("1.2.0")?;
    assert!(r_any.contains(&SemverVersion::from_str("1.2.0")?));
    assert!(r_any.contains(&SemverVersion::from_str("1.3.0")?));
    assert!(!r_any.contains(&SemverVersion::from_str("1.1.9")?));

    Ok(())
}

#[test]
fn test_pom_model_deserialization_and_dependencies() -> Result<()> {
    let raw_pom = r#"<project>
        <groupId>com.example</groupId>
        <artifactId>my-app</artifactId>
        <version>3.0.0</version>
        <packaging>aar</packaging>
        <dependencies>
            <dependency>
                <groupId>androidx.core</groupId>
                <artifactId>core</artifactId>
                <version>1.12.0</version>
                <scope>compile</scope>
            </dependency>
            <dependency>
                <groupId>junit</groupId>
                <artifactId>junit</artifactId>
                <version>4.13.2</version>
                <scope>test</scope>
            </dependency>
            <dependency>
                <groupId>com.optional</groupId>
                <artifactId>lib</artifactId>
                <version>1.0.0</version>
                <optional>true</optional>
            </dependency>
        </dependencies>
    </project>"#;

    let pom: ProjectObjectModel = quick_xml::de::from_str(raw_pom)?;
    assert_eq!(pom.packaging(), "aar");

    let deps = pom.dependencies();
    assert_eq!(deps.len(), 2);
    assert_eq!(&*deps[0].group, "androidx.core");
    assert_eq!(&*deps[0].name, "core");
    assert_eq!(deps[0].version(), Some("1.12.0"));
    assert_eq!(deps[0].scope(), Some("compile"));

    Ok(())
}

#[test]
fn test_persistent_maven_cache_serde() -> Result<()> {
    let dir = tempdir()?;
    let cache_file = dir.path().join(PersistentMavenCache::FILE_NAME);

    let mut cache = PersistentMavenCache::default();
    cache.metadata_cache.insert(
        "https://repo1.maven.org/maven2/pkg/meta.xml".to_string(),
        CachedMetadataEntry {
            etag: Some("etag-12345".to_string()),
            last_modified: Some("Wed, 21 Oct 2025 07:28:00 GMT".to_string()),
            fetched_timestamp_utc: 1761000000,
            xml_payload: "<metadata/>".to_string(),
        },
    );
    cache.solved_graphs.insert(
        "androidx.core:core:1.12.0".to_string(),
        vec!["androidx.annotation:annotation:1.8.0".to_string()],
    );

    cache.save(&cache_file)?;
    assert!(cache_file.exists());

    let loaded = PersistentMavenCache::load_or_create(&cache_file);
    assert_eq!(loaded.metadata_cache.len(), 1);
    assert_eq!(
        loaded.metadata_cache["https://repo1.maven.org/maven2/pkg/meta.xml"]
            .etag
            .as_deref(),
        Some("etag-12345")
    );
    assert_eq!(
        loaded.solved_graphs["androidx.core:core:1.12.0"],
        vec!["androidx.annotation:annotation:1.8.0"]
    );

    Ok(())
}

#[test]
fn test_repository_client_solve_pipeline() -> Result<()> {
    let dir = tempdir()?;
    let mut client = RepositoryClient::new(dir.path().to_path_buf(), InMemoryMockDownloader)?;
    client.add_repository("https://maven.google.com");

    let pkg = MavenCoordinates::new("androidx.annotation", "annotation");
    let ver = SemverVersion::from_str("1.8.0")?;

    let resolved = client.resolve(pkg, ver)?;
    assert_eq!(resolved.len(), 1);
    let resolved_name = resolved[0].file_name().unwrap().to_str().unwrap();
    assert_eq!(resolved_name, "androidx.annotation-annotation-1.8.0.jar");
    assert!(resolved[0].exists());

    Ok(())
}

#[test]
fn test_repository_affinity_avoids_google_404_for_central_libraries() -> Result<()> {
    let dir = tempdir()?;
    let mut client = RepositoryClient::new(dir.path().to_path_buf(), InMemoryMockDownloader)?;
    // Repositories in order: Google Maven (idx 0), Maven Central (idx 1)
    client.add_repository("https://maven.google.com");
    client.add_repository("https://repo1.maven.org/maven2");

    // 1. Static routing checks
    let repos_kotlin = client.prioritized_repositories("org.jetbrains.kotlin");
    assert_eq!(repos_kotlin.len(), 2);
    assert_eq!(repos_kotlin[0].0, 1);
    assert!(repos_kotlin[0].1.url.contains("repo1.maven.org"));

    let repos_android = client.prioritized_repositories("androidx.core");
    assert_eq!(repos_android.len(), 2);
    assert_eq!(repos_android[0].0, 0);
    assert!(repos_android[0].1.url.contains("maven.google.com"));

    // 2. Dynamic affinity learning
    let repos_custom = client.prioritized_repositories("com.custom.library");
    assert_eq!(repos_custom[0].0, 0);

    // Record that com.custom.library was discovered in Maven Central (idx 1)
    client.record_affinity("com.custom.library", 1);
    let repos_custom_after = client.prioritized_repositories("com.custom.library");
    assert_eq!(repos_custom_after[0].0, 1);
    assert_eq!(repos_custom_after[1].0, 0);

    // 3. Exact pairing for artifact resolution
    let pkg = MavenCoordinates::new("com.custom.library", "lib");
    let ver = SemverVersion::from_str("2.0.0")?;
    assert_eq!(client.get_artifact_repository(&pkg, &ver), None);
    client.record_artifact_repository(&pkg, &ver, 1);
    assert_eq!(client.get_artifact_repository(&pkg, &ver), Some(1));

    Ok(())
}
