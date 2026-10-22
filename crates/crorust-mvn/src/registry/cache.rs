use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistentMavenCache {
    pub metadata_cache: BTreeMap<String, CachedMetadataEntry>,

    pub solved_graphs: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedMetadataEntry {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_timestamp_utc: i64,
    pub xml_payload: String,
}

impl PersistentMavenCache {
    pub const FILE_NAME: &'static str = "cache.toml";

    pub fn load_or_create(cache_path: &Path) -> Self {
        if let Ok(content) = std::fs::read_to_string(cache_path) {
            if let Ok(cache) = toml::from_str(&content) {
                return cache;
            }
        }

        Self::default()
    }

    pub fn save(&self, cache_path: &Path) -> Result<()> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)
            .context("Failed serializing PersistentMavenCache to TOML")?;
        std::fs::write(cache_path, toml_str)?;
        Ok(())
    }
}
