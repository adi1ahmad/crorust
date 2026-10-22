//! Dynamic resolution for Android SDK platform archives from Google's repository manifest.

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const GOOGLE_REPO_XML_URL: &str = "https://dl.google.com/android/repository/repository2-1.xml";
pub const GOOGLE_REPO_BASE_URL: &str = "https://dl.google.com/android/repository/";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DynamicPlatformResolution {
    pub api_level: u32,
    pub revision: u32,
    pub url: String,
    pub checksum: String,
    pub checksum_type: String, // "sha1" or "sha256"
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformResolutionCache {
    #[serde(default)]
    pub platforms: BTreeMap<String, DynamicPlatformResolution>,
}

impl PlatformResolutionCache {
    pub fn load(path: &Path) -> Self {
        if let Ok(content) = fs::read_to_string(path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

pub struct GoogleSdkRepositoryClient<'a> {
    client: &'a Client,
    cache_file: PathBuf,
    mirror_base: Option<&'a str>,
}

impl<'a> GoogleSdkRepositoryClient<'a> {
    pub fn new(client: &'a Client, cache_dir: &Path, mirror_base: Option<&'a str>) -> Self {
        Self {
            client,
            cache_file: cache_dir.join("android_platform_cache.toml"),
            mirror_base,
        }
    }

    pub fn resolve_platform(&self, api_level: u32) -> Result<DynamicPlatformResolution> {
        let key = api_level.to_string();
        let mut cache = PlatformResolutionCache::load(&self.cache_file);

        if let Some(res) = cache.platforms.get(&key) {
            return Ok(res.clone());
        }

        let repo_url = if let Some(mirror) = self.mirror_base {
            format!("{}/repository2-1.xml", mirror.trim_end_matches('/'))
        } else {
            GOOGLE_REPO_XML_URL.to_string()
        };

        eprintln!(
            "\x1b[33m⚠️  [crorust] Resolving Android platform {} dynamically from Google SDK repository ({})\x1b[0m",
            api_level, repo_url
        );
        eprintln!(
            "\x1b[33m    Notice: Dynamic upstream manifest resolution reduces offline reproducibility and pinned supply chain guarantees.\x1b[0m"
        );

        let response = self.client.get(&repo_url).send().with_context(|| {
            format!(
                "Failed to fetch Android SDK repository manifest from {}",
                repo_url
            )
        })?;

        let xml_text = response.text().with_context(|| {
            format!(
                "Failed to read repository manifest payload from {}",
                repo_url
            )
        })?;
        let resolution = parse_platform_from_xml(&xml_text, api_level, self.mirror_base)?;

        cache.platforms.insert(key, resolution.clone());
        let _ = cache.save(&self.cache_file);

        Ok(resolution)
    }
}

pub fn parse_platform_from_xml(
    xml: &str,
    target_api: u32,
    mirror_base: Option<&str>,
) -> Result<DynamicPlatformResolution> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let target_path_exact = format!("platforms;android-{}", target_api);
    let mut buf = Vec::new();

    let mut in_target_package = false;
    let mut current_tag = String::new();

    let mut found_revision: Option<u32> = None;
    let mut found_url: Option<String> = None;
    let mut found_checksum: Option<String> = None;
    let mut found_checksum_type = "sha1".to_string();

    let mut best_candidate: Option<DynamicPlatformResolution> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "remotePackage" {
                    let mut is_target = false;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"path" {
                            let val = String::from_utf8_lossy(&attr.value);
                            if val == target_path_exact {
                                is_target = true;
                            }
                        }
                    }
                    if is_target {
                        in_target_package = true;
                        found_revision = None;
                        found_url = None;
                        found_checksum = None;
                        found_checksum_type = "sha1".to_string();
                    }
                } else if in_target_package {
                    current_tag = name;
                    if current_tag == "checksum" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"type" {
                                found_checksum_type =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_target_package {
                    let text = e.unescape()?.to_string();
                    match current_tag.as_str() {
                        "major" => {
                            if found_revision.is_none() {
                                found_revision = text.parse().ok();
                            }
                        }
                        "url" => {
                            if found_url.is_none() && text.ends_with(".zip") {
                                found_url = Some(text);
                            }
                        }
                        "checksum" if found_checksum.is_none() => {
                            found_checksum = Some(text);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "remotePackage" && in_target_package {
                    in_target_package = false;
                    if let (Some(url_rel), Some(chk)) = (found_url.take(), found_checksum.take()) {
                        let base = mirror_base.unwrap_or(GOOGLE_REPO_BASE_URL);
                        let full_url =
                            if url_rel.starts_with("http://") || url_rel.starts_with("https://") {
                                url_rel
                            } else {
                                format!(
                                    "{}{}",
                                    base.trim_end_matches('/'),
                                    if url_rel.starts_with('/') { "" } else { "/" }
                                ) + &url_rel
                            };
                        let c_type = if chk.len() == 64 || found_checksum_type == "sha256" {
                            "sha256".to_string()
                        } else {
                            "sha1".to_string()
                        };
                        let cand = DynamicPlatformResolution {
                            api_level: target_api,
                            revision: found_revision.unwrap_or(1),
                            url: full_url,
                            checksum: chk,
                            checksum_type: c_type,
                        };
                        if let Some(ref best) = best_candidate {
                            if cand.revision > best.revision {
                                best_candidate = Some(cand);
                            }
                        } else {
                            best_candidate = Some(cand);
                        }
                    }
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => anyhow::bail!(
                "XML parsing error while reading repository manifest: {:?}",
                e
            ),
            _ => {}
        }
        buf.clear();
    }

    best_candidate.ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find Android platform package for API level {} in Google repository manifest",
            target_api
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sdk:sdk-repository xmlns:sdk="http://schemas.android.com/sdk/android/repository/11">
  <remotePackage path="platforms;android-34">
    <type-details>
      <api-level>34</api-level>
    </type-details>
    <revision>
      <major>3</major>
    </revision>
    <display-name>Android SDK Platform 34</display-name>
    <archives>
      <archive>
        <complete>
          <size>63180081</size>
          <checksum>1f2e9478d6a7601425ceaa553311dc43191f103d</checksum>
          <url>platform-34-ext7_r03.zip</url>
        </complete>
      </archive>
    </archives>
  </remotePackage>
  <remotePackage path="platforms;android-35">
    <type-details>
      <api-level>35</api-level>
    </type-details>
    <revision>
      <major>2</major>
    </revision>
    <display-name>Android SDK Platform 35</display-name>
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

    #[test]
    fn test_parse_platform_from_xml() {
        let res35 = parse_platform_from_xml(SAMPLE_XML, 35, None).expect("Should find API 35");
        assert_eq!(res35.api_level, 35);
        assert_eq!(res35.revision, 2);
        assert_eq!(
            res35.url,
            "https://dl.google.com/android/repository/platform-35_r02.zip"
        );
        assert_eq!(res35.checksum, "0bb560a90a7a2cbd0dd8348224d518b638fe7949");
        assert_eq!(res35.checksum_type, "sha1");

        let res34 = parse_platform_from_xml(SAMPLE_XML, 34, None).expect("Should find API 34");
        assert_eq!(res34.api_level, 34);
        assert_eq!(res34.revision, 3);
        assert_eq!(
            res34.url,
            "https://dl.google.com/android/repository/platform-34-ext7_r03.zip"
        );
        assert_eq!(res34.checksum, "1f2e9478d6a7601425ceaa553311dc43191f103d");

        assert!(parse_platform_from_xml(SAMPLE_XML, 99, None).is_err());
    }

    #[test]
    fn test_platform_resolution_cache_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_path = temp_dir.path().join("platform_cache.toml");

        let mut cache = PlatformResolutionCache::default();
        cache.platforms.insert(
            "35".to_string(),
            DynamicPlatformResolution {
                api_level: 35,
                revision: 2,
                url: "https://dl.google.com/android/repository/platform-35_r02.zip".to_string(),
                checksum: "0bb560a90a7a2cbd0dd8348224d518b638fe7949".to_string(),
                checksum_type: "sha1".to_string(),
            },
        );

        cache.save(&cache_path).unwrap();
        let loaded = PlatformResolutionCache::load(&cache_path);
        assert_eq!(loaded, cache);
    }
}
