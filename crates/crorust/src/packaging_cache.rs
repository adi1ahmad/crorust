use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncrementalDecision {
    SkipAll,

    FastPathBinaryOnly,

    FullRebuild,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadataStamp {
    pub size: u64,
    pub mtime_nanos: u64,
    pub sha256: String,
}

impl FileMetadataStamp {
    pub fn from_metadata_and_hash(meta: &std::fs::Metadata, hash: String) -> Self {
        let size = meta.len();
        let mtime_nanos = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos().min(u64::MAX as u128) as u64)
            .unwrap_or(0);
        Self {
            size,
            mtime_nanos,
            sha256: hash,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackagingStamp {
    pub config_hash: String,

    pub icon_hash: Option<String>,

    pub assets: BTreeMap<String, FileMetadataStamp>,

    pub binaries: BTreeMap<String, FileMetadataStamp>,

    pub android_dex_hash: Option<String>,
}

impl PackagingStamp {
    pub fn compute(
        config_bytes: &[u8],
        icon_path: Option<&Path>,
        asset_roots: &[PathBuf],
        binary_paths: &[PathBuf],
        android_dex_hash: Option<String>,
    ) -> Result<Self> {
        Self::compute_incremental(
            config_bytes,
            icon_path,
            asset_roots,
            binary_paths,
            android_dex_hash,
            None,
        )
    }

    pub fn compute_incremental(
        config_bytes: &[u8],
        icon_path: Option<&Path>,
        asset_roots: &[PathBuf],
        binary_paths: &[PathBuf],
        android_dex_hash: Option<String>,
        previous_stamp: Option<&PackagingStamp>,
    ) -> Result<Self> {
        let config_hash = hex::encode(Sha256::digest(config_bytes));
        let icon_hash = match icon_path {
            Some(p) if p.is_file() => {
                let prev_meta = previous_stamp.and_then(|s| s.icon_meta(p));
                Some(hash_file_entry(p, prev_meta)?.sha256)
            }
            _ => None,
        };

        let mut candidate_entries = Vec::new();
        for root in asset_roots {
            collect_candidate_files(root, root, &mut candidate_entries);
        }

        let hashed_assets: Vec<Result<(String, FileMetadataStamp)>> = candidate_entries
            .into_par_iter()
            .map(|(key, path)| {
                let prev = previous_stamp.and_then(|s| s.assets.get(&key));
                let stamp = hash_file_entry(&path, prev)?;
                Ok((key, stamp))
            })
            .collect();

        let mut assets = BTreeMap::new();
        for res in hashed_assets {
            let (key, stamp) = res?;
            assets.insert(key, stamp);
        }

        let mut binaries = BTreeMap::new();
        for bin in binary_paths {
            if bin.is_file() {
                let key = bin.to_string_lossy().to_string();
                let prev_meta = previous_stamp.and_then(|s| s.binaries.get(&key));
                let stamp = hash_file_entry(bin, prev_meta)?;
                binaries.insert(key, stamp);
            }
        }

        Ok(Self {
            config_hash,
            icon_hash,
            assets,
            binaries,
            android_dex_hash,
        })
    }

    fn icon_meta(&self, path: &Path) -> Option<&FileMetadataStamp> {
        self.assets.get(&path.to_string_lossy().to_string())
    }

    pub const FILE_NAME: &'static str = ".packaging_stamp.toml";

    pub fn load(stamp_path: &Path) -> Option<Self> {
        if stamp_path.exists() {
            if let Ok(content) = std::fs::read_to_string(stamp_path) {
                if let Ok(stamp) = toml::from_str(&content) {
                    return Some(stamp);
                }
            }
        }

        None
    }

    pub fn save(&self, stamp_path: &Path) -> Result<()> {
        if let Some(parent) = stamp_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(stamp_path, toml_str)?;
        Ok(())
    }

    pub fn evaluate(&self, previous: Option<&Self>, output_artifact: &Path) -> IncrementalDecision {
        if !output_artifact.exists() {
            return IncrementalDecision::FullRebuild;
        }
        let Some(prev) = previous else {
            return IncrementalDecision::FullRebuild;
        };

        let non_binary_equal = self.config_hash == prev.config_hash
            && self.icon_hash == prev.icon_hash
            && self.android_dex_hash == prev.android_dex_hash
            && self.assets.len() == prev.assets.len()
            && self.assets.iter().all(|(k, v)| {
                prev.assets
                    .get(k)
                    .map(|pv| pv.sha256 == v.sha256)
                    .unwrap_or(false)
            });

        if !non_binary_equal {
            return IncrementalDecision::FullRebuild;
        }

        let binaries_equal = self.binaries.len() == prev.binaries.len()
            && self.binaries.iter().all(|(k, v)| {
                prev.binaries
                    .get(k)
                    .map(|pv| pv.sha256 == v.sha256)
                    .unwrap_or(false)
            });

        if binaries_equal {
            IncrementalDecision::SkipAll
        } else {
            IncrementalDecision::FastPathBinaryOnly
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidBytecodeStamp {
    pub sources: BTreeMap<String, String>,
    pub maven_deps: Vec<String>,
    pub r_symbols_hash: String,
    pub min_sdk: u32,
    pub compile_sdk: u32,
    #[serde(default)]
    pub desugar_enabled: bool,
    #[serde(default)]
    pub desugar_rules_hash: Option<String>,
}

impl AndroidBytecodeStamp {
    pub fn compute(
        source_dirs: &[PathBuf],
        maven_deps: &[String],
        r_symbols_text: &str,
        min_sdk: u32,
        compile_sdk: u32,
        desugar_enabled: bool,
        desugar_rules_hash: Option<String>,
    ) -> Result<Self> {
        let mut sources = BTreeMap::new();
        for dir in source_dirs {
            if dir.exists() {
                collect_tree_digests(dir, dir, &mut sources)?;
            }
        }
        let mut deps = maven_deps.to_vec();
        deps.sort();
        let r_symbols_hash = hex::encode(Sha256::digest(r_symbols_text.as_bytes()));
        Ok(Self {
            sources,
            maven_deps: deps,
            r_symbols_hash,
            min_sdk,
            compile_sdk,
            desugar_enabled,
            desugar_rules_hash,
        })
    }

    pub fn fingerprint(&self) -> String {
        let toml_str = toml::to_string(self).unwrap_or_default();
        hex::encode(Sha256::digest(toml_str.as_bytes()))
    }
}

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 131072]; // 128 KiB buffer matches modern NVMe storage cluster sizes and halves read syscalls
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn hash_file_entry(path: &Path, prev: Option<&FileMetadataStamp>) -> Result<FileMetadataStamp> {
    let meta = std::fs::symlink_metadata(path)?;
    let size = meta.len();
    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0);

    if let Some(prev) = prev {
        if prev.size == size && prev.mtime_nanos == mtime_nanos && !prev.sha256.is_empty() {
            return Ok(FileMetadataStamp {
                size,
                mtime_nanos,
                sha256: prev.sha256.clone(),
            });
        }
    }

    let hash = hash_file(path)?;
    Ok(FileMetadataStamp {
        size,
        mtime_nanos,
        sha256: hash,
    })
}

fn collect_candidate_files(base: &Path, current: &Path, candidates: &mut Vec<(String, PathBuf)>) {
    if !current.exists() {
        return;
    }
    if current.is_file() {
        let key = current.to_string_lossy().to_string();
        candidates.push((key, current.to_path_buf()));
        return;
    }
    if current.is_dir() {
        if let Ok(entries) = std::fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_candidate_files(base, &path, candidates);
                } else if path.is_file() {
                    let rel = path
                        .strip_prefix(base)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let key = format!("{}:{rel}", base.display());
                    candidates.push((key, path));
                }
            }
        }
    }
}

fn collect_tree_digests(
    base: &Path,
    current: &Path,
    out: &mut BTreeMap<String, String>,
) -> Result<()> {
    if !current.exists() {
        return Ok(());
    }
    if current.is_file() {
        let key = current.to_string_lossy().to_string();
        out.insert(key, hash_file(current)?);
        return Ok(());
    }
    if current.is_dir() {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_tree_digests(base, &path, out)?;
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let key = format!("{}:{rel}", base.display());
                out.insert(key, hash_file(&path)?);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_packaging_stamp_skip_fastpath_and_full_rebuild_decisions() {
        let temp = TempDir::new().unwrap();
        let icon = temp.path().join("icon.png");
        let asset_dir = temp.path().join("assets");
        let bin = temp.path().join("libapp.so");
        let output_apk = temp.path().join("app.apk");

        std::fs::create_dir_all(&asset_dir).unwrap();
        std::fs::write(&icon, b"icon-v1").unwrap();
        std::fs::write(asset_dir.join("data.txt"), b"asset-v1").unwrap();
        std::fs::write(&bin, b"binary-v1").unwrap();

        let stamp1 = PackagingStamp::compute(
            b"manifest-v1",
            Some(&icon),
            std::slice::from_ref(&asset_dir),
            std::slice::from_ref(&bin),
            Some("dex-v1".into()),
        )
        .unwrap();

        assert_eq!(
            stamp1.evaluate(Some(&stamp1), &output_apk),
            IncrementalDecision::FullRebuild
        );

        std::fs::write(&output_apk, b"packaged-apk").unwrap();

        assert_eq!(
            stamp1.evaluate(Some(&stamp1), &output_apk),
            IncrementalDecision::SkipAll
        );

        std::fs::write(&bin, b"binary-v2-recompiled").unwrap();
        let stamp2 = PackagingStamp::compute(
            b"manifest-v1",
            Some(&icon),
            std::slice::from_ref(&asset_dir),
            std::slice::from_ref(&bin),
            Some("dex-v1".into()),
        )
        .unwrap();
        assert_eq!(
            stamp2.evaluate(Some(&stamp1), &output_apk),
            IncrementalDecision::FastPathBinaryOnly
        );

        std::fs::write(asset_dir.join("data.txt"), b"asset-v2-modified").unwrap();
        let stamp3 = PackagingStamp::compute(
            b"manifest-v1",
            Some(&icon),
            std::slice::from_ref(&asset_dir),
            std::slice::from_ref(&bin),
            Some("dex-v1".into()),
        )
        .unwrap();
        assert_eq!(
            stamp3.evaluate(Some(&stamp2), &output_apk),
            IncrementalDecision::FullRebuild
        );
    }

    #[test]
    fn test_two_tier_stat_caching_reuses_hash() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("asset.dat");
        std::fs::write(&file_path, b"initial payload").unwrap();

        let initial_stamp = hash_file_entry(&file_path, None).unwrap();
        assert_eq!(initial_stamp.size, 15);
        assert!(!initial_stamp.sha256.is_empty());

        let cached_stamp = hash_file_entry(&file_path, Some(&initial_stamp)).unwrap();
        assert_eq!(cached_stamp, initial_stamp);

        std::fs::write(&file_path, b"initial payload extended").unwrap();
        let updated_stamp = hash_file_entry(&file_path, Some(&initial_stamp)).unwrap();
        assert_ne!(updated_stamp.size, initial_stamp.size);
        assert_ne!(updated_stamp.sha256, initial_stamp.sha256);
    }

    #[test]
    fn test_packaging_stamp_toml_roundtrip() {
        let temp = TempDir::new().unwrap();
        let stamp_path = temp.path().join(PackagingStamp::FILE_NAME);

        let mut stamp = PackagingStamp {
            config_hash: "test_config_hash_12345".to_string(),
            icon_hash: Some("test_icon_hash_67890".to_string()),
            ..Default::default()
        };
        stamp.binaries.insert(
            "libtest.so".to_string(),
            FileMetadataStamp {
                size: 1024,
                mtime_nanos: 1713456789000,
                sha256: "hash1".to_string(),
            },
        );

        // Test TOML save & load
        stamp.save(&stamp_path).unwrap();
        assert!(stamp_path.exists());
        let content = std::fs::read_to_string(&stamp_path).unwrap();
        assert!(content.contains("test_config_hash_12345"));

        let loaded = PackagingStamp::load(&stamp_path).expect("Should load TOML stamp");
        assert_eq!(loaded, stamp);
    }

    #[test]
    fn test_hash_file_across_sizes() {
        let temp = TempDir::new().unwrap();

        // Test 0 bytes
        let empty_file = temp.path().join("empty.bin");
        std::fs::write(&empty_file, b"").unwrap();
        assert_eq!(
            hash_file(&empty_file).unwrap(),
            hex::encode(Sha256::digest(b""))
        );

        // Test 1 KiB
        let small_file = temp.path().join("small.bin");
        let small_data = vec![0xABu8; 1024];
        std::fs::write(&small_file, &small_data).unwrap();
        assert_eq!(
            hash_file(&small_file).unwrap(),
            hex::encode(Sha256::digest(&small_data))
        );

        // Test exactly 128 KiB (matching single buffer boundary)
        let exact_file = temp.path().join("exact.bin");
        let exact_data = vec![0x55u8; 131072];
        std::fs::write(&exact_file, &exact_data).unwrap();
        assert_eq!(
            hash_file(&exact_file).unwrap(),
            hex::encode(Sha256::digest(&exact_data))
        );

        // Test multi-buffer boundary (300 KiB)
        let large_file = temp.path().join("large.bin");
        let large_data = vec![0xEFu8; 300 * 1024];
        std::fs::write(&large_file, &large_data).unwrap();
        assert_eq!(
            hash_file(&large_file).unwrap(),
            hex::encode(Sha256::digest(&large_data))
        );
    }
}
