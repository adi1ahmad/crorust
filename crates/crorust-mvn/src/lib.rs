//! # crorust-mvn: Pure-Rust Maven Dependency Resolution Engine
//!
//! Resolves Android and Java Maven dependencies using the PubGrub algorithm to download
//! `.aar` and `.jar` libraries without requiring the JVM, Gradle, or Maven.
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Inputs["Dependency Requirements"]
//!         Req["androidx.core:core-ktx:1.10.1"]
//!     end
//!
//!     subgraph Engine["Resolution Engine"]
//!         Client["RepositoryClient<br/><i>Google Maven / Central</i>"]
//!         POM["StreamingPomReader<br/><i>Fast XML parsing</i>"]
//!         Solver["PubGrub Version Solver<br/><i>Conflict-driven clause learning</i>"]
//!         Client --> POM --> Solver
//!     end
//!
//!     subgraph Artifacts["Downloaded Libraries"]
//!         Solver --> AAR["ParallelAarUnpacker<br/><i>classes.jar & resources</i>"]
//!     end
//!
//!     Req --> Engine --> Artifacts
//! ```
//!
//! ## Core Modules
//! - [`extract`]: Multi-threaded `.aar` unpacking for `classes.jar` and Android resources.
//! - [`interval`]: Mathematical version interval parsing (`[1.0, 2.0)`).
//! - [`model`]: Streaming XML Project Object Model (`pom.xml`) reader and dependency models.
//! - [`registry`]: Remote artifact repository client (Google Maven, Maven Central) with caching.
//! - [`solver`]: PubGrub version-solving algorithm adaptation.

pub mod extract;
pub mod interval;
pub mod model;
pub mod registry;
pub mod solver;

pub use extract::ParallelAarUnpacker;

pub use interval::{
    parse_version_constraint, IntervalBoundEndpoint, IntervalLexer, IntervalParser, IntervalToken,
    VersionConstraint,
};

pub use model::{
    DependencyContainer, DependencyRequirement, DependencyScopeKind, ManagedDependencyCatalog,
    MavenCoordinates, ProjectObjectModel, RemoteArtifactRef, SemverVersion, StreamingPomReader,
};

pub use registry::{
    CachedMetadataEntry, Download, DownloadBatchItem, MavenMetadataDocument, PersistentMavenCache,
    ReleaseTimeline, RemoteArtifactRepository, RepositoryClient, RepositoryHttpCredentials,
    VersionEnumeration, GOOGLE_MAVEN, GRADLE_PLUGIN_PORTAL, MAVEN_CENTRAL,
};

pub use solver::format_resolution_conflict;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::path::Path;

    struct MockDownload;

    impl Download for MockDownload {
        fn download(&self, _url: &str, _dest: &Path) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_repository_registration_and_url_normalization() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut resolver = RepositoryClient::new(temp.path().to_path_buf(), MockDownload)?;

        resolver.add_repository("https://maven.google.com///");
        assert_eq!(resolver.repositories()[0], "https://maven.google.com");

        let dynamic_url = format!("https://nexus.internal.corp/{}/maven", "team-a");
        resolver.add_repository(dynamic_url.clone());
        assert_eq!(resolver.repositories()[1], dynamic_url);

        let pkg = MavenCoordinates::new("androidx.core", "core");
        assert_eq!(
            pkg.url(&resolver.repositories()[0]),
            "https://maven.google.com/androidx/core/core/maven-metadata.xml"
        );

        let mut default_resolver = RepositoryClient::new(temp.path().to_path_buf(), MockDownload)?;
        default_resolver.add_default_repositories();
        assert_eq!(default_resolver.repositories().len(), 2);
        assert_eq!(default_resolver.repositories()[0], GOOGLE_MAVEN);
        assert_eq!(default_resolver.repositories()[1], MAVEN_CENTRAL);

        let mut auth_resolver = RepositoryClient::new(temp.path().to_path_buf(), MockDownload)?;
        auth_resolver.add_repository_with_auth(
            "https://nexus.corp/maven",
            RepositoryHttpCredentials::Basic {
                username: "user".into(),
                password: "pass".into(),
            },
        );
        assert_eq!(auth_resolver.repositories().len(), 1);
        assert_eq!(auth_resolver.repositories()[0], "https://nexus.corp/maven");

        Ok(())
    }
}
