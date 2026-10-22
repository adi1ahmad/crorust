use crate::model::coordinate::{MavenCoordinates, SemverVersion};
use pubgrub::error::PubGrubError;
use pubgrub::report::{DefaultStringReporter, Reporter};

pub fn format_resolution_conflict(error: &PubGrubError<MavenCoordinates, SemverVersion>) -> String {
    match error {
        PubGrubError::NoSolution(tree) => {
            let mut collapsed = tree.clone();
            collapsed.collapse_no_versions();
            DefaultStringReporter::report(&collapsed)
        }
        PubGrubError::ErrorRetrievingDependencies {
            package,
            version,
            source,
        } => {
            format!("Failed to retrieve dependencies for {package} {version}: {source}")
        }
        PubGrubError::DependencyOnTheEmptySet {
            package,
            version,
            dependent,
        } => {
            format!("{package} {version} depends on empty set for {dependent}")
        }
        PubGrubError::SelfDependency { package, version } => {
            format!("{package} {version} depends on itself")
        }
        PubGrubError::ErrorChoosingPackageVersion(err) => {
            format!("Failed to choose package version: {err}")
        }
        PubGrubError::ErrorInShouldCancel(err) => {
            format!("Error in resolution cancellation check: {err}")
        }
        PubGrubError::Failure(msg) => msg.clone(),
    }
}
