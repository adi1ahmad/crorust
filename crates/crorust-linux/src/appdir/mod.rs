use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct AppDirFilesystemLayout {
    pub root: PathBuf,
}

impl AppDirFilesystemLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("usr").join("bin")
    }

    pub fn lib_dir(&self) -> PathBuf {
        self.root.join("usr").join("lib")
    }

    pub fn share_dir(&self) -> PathBuf {
        self.root.join("usr").join("share")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.bin_dir()).with_context(|| {
            format!("Failed creating bin dir in AppDir: {}", self.root.display())
        })?;
        std::fs::create_dir_all(self.lib_dir()).with_context(|| {
            format!("Failed creating lib dir in AppDir: {}", self.root.display())
        })?;
        std::fs::create_dir_all(self.share_dir()).with_context(|| {
            format!(
                "Failed creating share dir in AppDir: {}",
                self.root.display()
            )
        })?;
        Ok(())
    }
}
