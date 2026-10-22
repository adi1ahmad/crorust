use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
pub struct CaseFoldCache {
    cache: Arc<Mutex<HashMap<PathBuf, Vec<String>>>>,
}

impl CaseFoldCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve(&self, base_dir: &Path, relative: &Path) -> Option<PathBuf> {
        let mut current = base_dir.to_path_buf();

        let normalized = relative.to_string_lossy().replace('\\', "/");
        let rel_path = Path::new(&normalized);

        for component in rel_path.components() {
            match component {
                Component::Normal(segment) => {
                    let segment_str = segment.to_str()?;
                    let matched_child = self.find_case_insensitive_child(&current, segment_str)?;
                    current.push(matched_child);
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    current.pop();
                }
                _ => return None,
            }
        }

        if current.exists() {
            Some(current)
        } else {
            None
        }
    }

    fn find_case_insensitive_child(&self, dir: &Path, target: &str) -> Option<String> {
        let mut cache = self.cache.lock().ok()?;

        if !cache.contains_key(dir) {
            let mut entries = Vec::new();
            if let Ok(read_dir) = std::fs::read_dir(dir) {
                for entry in read_dir.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        entries.push(name);
                    }
                }
            }
            cache.insert(dir.to_path_buf(), entries);
        }

        let entries = cache.get(dir)?;

        if let Some(exact) = entries.iter().find(|e| *e == target) {
            return Some(exact.clone());
        }

        if let Some(matched) = entries.iter().find(|e| e.eq_ignore_ascii_case(target)) {
            return Some(matched.clone());
        }

        entries
            .iter()
            .find(|e| e.to_lowercase() == target.to_lowercase())
            .cloned()
    }
}

pub fn case_fold_lookup(base_dir: &Path, relative: &Path) -> Option<PathBuf> {
    let cache = CaseFoldCache::new();
    cache.resolve(base_dir, relative)
}

pub fn generate_clang_vfs_overlay(
    out_yaml_path: &Path,
    case_insensitive_dirs: &[PathBuf],
) -> anyhow::Result<()> {
    let mut yaml = String::from("{\n  'version': 0,\n  'case-sensitive': 'false',\n  'roots': [\n");

    for (i, dir) in case_insensitive_dirs.iter().enumerate() {
        let clean_path = crate::path::sanitize_path(dir);
        yaml.push_str(&format!(
            "    {{\n      'type': 'directory',\n      'name': '{}',\n      'contents': [\n        {{\n          'type': 'directory-remap',\n          'name': '.',\n          'external-contents': '{}'\n        }}\n      ]\n    }}",
            clean_path, clean_path
        ));
        if i + 1 < case_insensitive_dirs.len() {
            yaml.push(',');
        }
        yaml.push('\n');
    }

    yaml.push_str("  ]\n}\n");

    if let Some(parent) = out_yaml_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_yaml_path, yaml)?;
    Ok(())
}

/// Atomically replaces destination with source. On Windows, if background
/// antivirus or indexers hold transient file locks, retries with exponential backoff.
pub fn atomic_replace_file(src: &Path, dst: &Path) -> anyhow::Result<()> {
    use anyhow::Context;

    if cfg!(target_os = "windows") {
        for attempt in 0..5 {
            match std::fs::rename(src, dst) {
                Ok(()) => return Ok(()),
                Err(_) if attempt < 4 => {
                    std::thread::sleep(std::time::Duration::from_millis(25 * (1 << attempt)));
                }
                Err(err) => {
                    if std::fs::copy(src, dst).is_ok() {
                        let _ = std::fs::remove_file(src);
                        return Ok(());
                    }
                    return Err(err).with_context(|| {
                        format!(
                            "Failed atomic rename `{}` -> `{}` after 5 attempts",
                            src.display(),
                            dst.display()
                        )
                    });
                }
            }
        }
        Ok(())
    } else {
        std::fs::rename(src, dst)
            .or_else(|_| {
                std::fs::copy(src, dst)?;
                let _ = std::fs::remove_file(src);
                Ok::<(), std::io::Error>(())
            })
            .with_context(|| {
                format!(
                    "Failed atomic rename `{}` -> `{}`",
                    src.display(),
                    dst.display()
                )
            })
    }
}

/// Streams a file in 128 KiB chunks to compute its hex SHA-256 digest and total length,
/// requiring strictly 128 KiB of RAM regardless of file size.
pub fn hash_file_streaming(path: &Path) -> Result<(String, u64)> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed opening file for streaming hash: {}", path.display()))?;
    let len = file.metadata()?.len();
    let mut reader = std::io::BufReader::with_capacity(131072, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 131072];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok((hex::encode(hasher.finalize()), len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_replace_file() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let file_a = dir.path().join("a.txt");
        let file_b = dir.path().join("b.txt");

        std::fs::write(&file_a, b"hello a")?;
        atomic_replace_file(&file_a, &file_b)?;

        assert!(!file_a.exists());
        assert_eq!(std::fs::read(&file_b)?, b"hello a");

        // Test overwrite
        let file_c = dir.path().join("c.txt");
        std::fs::write(&file_c, b"hello c")?;
        atomic_replace_file(&file_c, &file_b)?;

        assert!(!file_c.exists());
        assert_eq!(std::fs::read(&file_b)?, b"hello c");
        Ok(())
    }

    #[test]
    fn test_hash_file_streaming() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("payload.bin");

        // Test multi-chunk buffer (300 KiB across 128 KiB boundaries)
        let data = vec![0x42u8; 300 * 1024];
        std::fs::write(&file_path, &data)?;

        let (hash, len) = hash_file_streaming(&file_path)?;
        assert_eq!(len, 300 * 1024);
        assert_eq!(hash, hex::encode(Sha256::digest(&data)));

        // Test empty file
        let empty_path = dir.path().join("empty.bin");
        std::fs::write(&empty_path, b"")?;
        let (empty_hash, empty_len) = hash_file_streaming(&empty_path)?;
        assert_eq!(empty_len, 0);
        assert_eq!(empty_hash, hex::encode(Sha256::digest(b"")));

        Ok(())
    }
}
