use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "metadata")]
pub struct MavenMetadataDocument {
    #[serde(rename = "groupId", default)]
    pub group_id: Option<String>,
    #[serde(rename = "artifactId", default)]
    pub artifact_id: Option<String>,
    pub versioning: ReleaseTimeline,
}

impl MavenMetadataDocument {
    #[inline]
    pub fn latest(&self) -> &str {
        &self.versioning.latest
    }

    #[inline]
    pub fn release(&self) -> &str {
        &self.versioning.release
    }

    #[inline]
    pub fn versions(&self) -> &[String] {
        &self.versioning.versions.version
    }

    #[inline]
    pub fn last_updated(&self) -> Option<&str> {
        self.versioning.last_updated.as_deref()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReleaseTimeline {
    #[serde(default)]
    pub latest: String,
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub versions: VersionEnumeration,
    #[serde(rename = "lastUpdated", default)]
    pub last_updated: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VersionEnumeration {
    #[serde(default)]
    pub version: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_parse_google_android_metadata_document() -> Result<()> {
        let xml_payload = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>androidx.core</groupId>
  <artifactId>core</artifactId>
  <versioning>
    <latest>1.13.1</latest>
    <release>1.13.1</release>
    <versions>
      <version>1.11.0</version>
      <version>1.12.0</version>
      <version>1.13.0</version>
      <version>1.13.1</version>
    </versions>
    <lastUpdated>20240501120000</lastUpdated>
  </versioning>
</metadata>"#;

        let doc: MavenMetadataDocument = quick_xml::de::from_str(xml_payload)?;
        assert_eq!(doc.latest(), "1.13.1");
        assert_eq!(doc.release(), "1.13.1");
        assert_eq!(doc.versions().len(), 4);
        assert_eq!(doc.versions()[0], "1.11.0");
        assert_eq!(doc.versions()[3], "1.13.1");
        assert_eq!(doc.last_updated(), Some("20240501120000"));
        Ok(())
    }
}
