pub mod coordinate;
pub mod dependency;
pub mod pom;
pub mod pom_streaming;

pub use coordinate::{MavenCoordinates, RemoteArtifactRef, SemverVersion};
pub use dependency::{DependencyRequirement, DependencyScopeKind};
pub use pom::{DependencyContainer, ManagedDependencyCatalog, ProjectObjectModel};
pub use pom_streaming::StreamingPomReader;
