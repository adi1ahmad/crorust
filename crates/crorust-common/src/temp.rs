use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct TempPathGuard {
    path: PathBuf,
    active: bool,
}

impl TempPathGuard {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            active: true,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn disarm(&mut self) {
        self.active = false;
    }

    pub fn remove_now(&mut self) -> std::io::Result<()> {
        if self.active {
            self.active = false;
            std::fs::remove_file(&self.path)
        } else {
            Ok(())
        }
    }
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
