use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct MavenCoordinates {
    #[serde(rename = "$unflatten=groupId")]
    pub group: Arc<str>,
    #[serde(rename = "$unflatten=artifactId")]
    pub name: Arc<str>,
}

impl MavenCoordinates {
    #[inline]
    pub fn new(group: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        Self {
            group: Arc::from(group.as_ref()),
            name: Arc::from(name.as_ref()),
        }
    }

    pub fn file_name(&self) -> String {
        format!("{}-{}.metadata.xml", self.group, self.name)
    }

    pub fn url(&self, repository_base_url: &str) -> String {
        let base = repository_base_url.trim_end_matches('/');
        let group_path = self.group.replace('.', "/");
        format!(
            "{base}/{group_path}/{name}/maven-metadata.xml",
            name = self.name
        )
    }

    #[inline]
    pub fn group_id(&self) -> &str {
        &self.group
    }

    #[inline]
    pub fn artifact_id(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for MavenCoordinates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.group, self.name)
    }
}

#[derive(Clone, Debug)]
pub struct SemverVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub suffix: Option<String>,
    pub raw: Option<String>,
}

impl SemverVersion {
    pub fn is_stable_release(&self) -> bool {
        match &self.suffix {
            None => true,
            Some(sfx) => {
                let tag = sfx.to_ascii_lowercase();
                !tag.contains("alpha")
                    && !tag.contains("beta")
                    && !tag.contains("rc")
                    && !tag.contains("snapshot")
                    && !tag.contains("preview")
                    && !tag.contains("dev")
                    && !tag.contains("empty")
            }
        }
    }
}

impl PartialEq for SemverVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.suffix == other.suffix
    }
}

impl Eq for SemverVersion {}

impl std::hash::Hash for SemverVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.major.hash(state);
        self.minor.hash(state);
        self.patch.hash(state);
        self.suffix.hash(state);
    }
}

impl PartialOrd for SemverVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemverVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match (&self.suffix, &other.suffix) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(s_a), Some(s_b)) => s_a.cmp(s_b),
        }
    }
}

impl pubgrub::version::Version for SemverVersion {
    fn lowest() -> Self {
        Self {
            major: 0,
            minor: 0,
            patch: 0,
            suffix: Some("\0".to_string()),
            raw: None,
        }
    }

    fn bump(&self) -> Self {
        if let Some(ref sfx) = self.suffix {
            Self {
                major: self.major,
                minor: self.minor,
                patch: self.patch,
                suffix: Some(format!("{sfx}\0")),
                raw: None,
            }
        } else {
            Self {
                major: self.major,
                minor: self.minor,
                patch: self.patch + 1,
                suffix: Some("\0".to_string()),
                raw: None,
            }
        }
    }
}

impl FromStr for SemverVersion {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        ensure!(!trimmed.is_empty(), "Maven version cannot be empty");

        let raw_copy = trimmed.to_string();
        let (numeric_stem, dash_qualifier) = match trimmed.split_once('-') {
            Some((num, qual)) => (num, Some(qual.to_string())),
            None => (trimmed, None),
        };

        let mut segments = numeric_stem.split('.');
        let major = segments
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<u32>())
            .transpose()?
            .unwrap_or(0);
        let minor = segments
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<u32>())
            .transpose()?
            .unwrap_or(0);
        let patch = segments
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<u32>())
            .transpose()?
            .unwrap_or(0);

        let remainder: Vec<&str> = segments.collect();
        let suffix = match (remainder.is_empty(), dash_qualifier) {
            (true, q) => q,
            (false, None) => Some(format!(".{}", remainder.join("."))),
            (false, Some(q)) => Some(format!(".{}-{}", remainder.join("."), q)),
        };

        Ok(Self {
            major,
            minor,
            patch,
            suffix,
            raw: Some(raw_copy),
        })
    }
}

impl fmt::Display for SemverVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref original) = self.raw {
            write!(f, "{original}")
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
            if let Some(ref sfx) = self.suffix {
                write!(f, "-{sfx}")?;
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RemoteArtifactRef<'a> {
    pub package: &'a MavenCoordinates,
    pub version: &'a SemverVersion,
}

impl RemoteArtifactRef<'_> {
    pub fn file_name(&self, extension: &str) -> String {
        format!(
            "{}-{}-{}.{}",
            self.package.group, self.package.name, self.version, extension
        )
    }

    pub fn url(&self, repository_base_url: &str, extension: &str) -> String {
        let base = repository_base_url.trim_end_matches('/');
        let group_dir = self.package.group.replace('.', "/");
        format!(
            "{base}/{group_dir}/{name}/{ver}/{name}-{ver}.{extension}",
            name = self.package.name,
            ver = self.version,
        )
    }
}

impl fmt::Display for RemoteArtifactRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.package.group, self.package.name, self.version
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pubgrub::version::Version as PubGrubVersion;
    use std::str::FromStr;

    #[test]
    fn test_semver_hierarchy_and_qualifier_ordering() -> Result<()> {
        let alpha = SemverVersion::from_str("2.1.0-alpha01")?;
        let beta = SemverVersion::from_str("2.1.0-beta01")?;
        let rc = SemverVersion::from_str("2.1.0-rc01")?;
        let ga = SemverVersion::from_str("2.1.0")?;

        assert!(alpha < beta);
        assert!(beta < rc);
        assert!(rc < ga);
        assert!(!alpha.is_stable_release());
        assert!(!beta.is_stable_release());
        assert!(!rc.is_stable_release());
        assert!(ga.is_stable_release());

        let next_bump = ga.bump();
        assert!(ga < next_bump);
        assert_eq!(next_bump.major, 2);
        assert_eq!(next_bump.minor, 1);
        Ok(())
    }
}
