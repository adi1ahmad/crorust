use crate::model::coordinate::{MavenCoordinates, SemverVersion};
use anyhow::{Context, Result};
use pubgrub::range::Range;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum DependencyScopeKind {
    Compile,
    Runtime,
    Provided,
    Test,
    System,
    Import,
}

impl DependencyScopeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compile => "compile",
            Self::Runtime => "runtime",
            Self::Provided => "provided",
            Self::Test => "test",
            Self::System => "system",
            Self::Import => "import",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename = "dependency")]
pub struct DependencyRequirement {
    #[serde(rename = "groupId")]
    pub group: Arc<str>,
    #[serde(rename = "artifactId")]
    pub name: Arc<str>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
}

impl DependencyRequirement {
    pub fn package(&self) -> MavenCoordinates {
        MavenCoordinates {
            group: self.group.clone(),
            name: self.name.clone(),
        }
    }

    pub fn coordinate(&self) -> MavenCoordinates {
        self.package()
    }

    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    pub fn is_optional(&self) -> bool {
        self.optional.unwrap_or(false)
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn range(&self) -> Result<Range<SemverVersion>> {
        match &self.version {
            Some(v) => crate::interval::parse_version_constraint(v),
            None => Ok(Range::any()),
        }
    }
}

impl FromStr for DependencyRequirement {
    type Err = anyhow::Error;

    fn from_str(dep: &str) -> Result<Self> {
        let (group, rest) = dep.split_once(':').context("invalid dep")?;
        let (name, version) = rest.split_once(':').context("invalid dep")?;
        anyhow::ensure!(
            !group.trim().is_empty() && !name.trim().is_empty() && !version.trim().is_empty(),
            "Dependency coordinate `{dep}` contains an empty group, artifact, or version"
        );
        let parsed = Self {
            group: Arc::from(group.trim()),
            name: Arc::from(name.trim()),
            version: Some(version.trim().into()),
            scope: None,
            optional: None,
        };
        let _ = parsed.range()?;
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_android_dependency_declaration() -> Result<()> {
        let pom_snippet = r#"
            <dependency>
                <groupId>androidx.activity</groupId>
                <artifactId>activity-ktx</artifactId>
                <version>1.9.0</version>
                <scope>compile</scope>
            </dependency>"#;

        let dep: DependencyRequirement = quick_xml::de::from_str(pom_snippet)?;
        assert_eq!(&*dep.package().group, "androidx.activity");
        assert_eq!(&*dep.package().name, "activity-ktx");
        assert_eq!(dep.version(), Some("1.9.0"));
        assert_eq!(dep.scope(), Some("compile"));
        assert!(!dep.is_optional());

        let parsed = DependencyRequirement::from_str("androidx.compose.ui:ui:1.6.7")?;
        assert_eq!(&*parsed.package().group, "androidx.compose.ui");
        Ok(())
    }
}
