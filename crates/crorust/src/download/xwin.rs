//! Automatic acquisition of Microsoft Windows SDK & MSVC CRT via xwin.
//!
//! Downloads official Microsoft packages directly from Microsoft CDN,
//! resolves case-sensitivity for Linux/macOS filesystems, and organizes
//! the sysroot layout required by Clang and LLD.

use crate::core::BuildEnv;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct XWinManager {
    target_dir: PathBuf,
    cache_download_dir: PathBuf,
    version: String,
    is_offline: bool,
}

impl XWinManager {
    pub fn new(env: &BuildEnv) -> Self {
        let windows_cfg = env.manifest_config().windows_platform_config();
        let version = windows_cfg.sdk_version().to_string();
        let is_offline = env.manifest_config().toolchain().is_offline_enforced();
        Self::with_cache_dir(env.cache_directory(), &version, is_offline)
    }

    pub fn with_cache_dir(cache_dir: &Path, version: &str, is_offline: bool) -> Self {
        let target_dir = cache_dir.join("sysroots").join("Windows.sdk").join(version);
        let cache_download_dir = cache_dir.join("download").join("xwin");
        Self {
            target_dir,
            cache_download_dir,
            version: version.to_string(),
            is_offline,
        }
    }

    pub fn target_directory(&self) -> &Path {
        &self.target_dir
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn is_installed(&self) -> bool {
        self.target_dir.join(".complete").exists()
            && self.target_dir.join("crt").join("include").exists()
            && self.target_dir.join("sdk").join("include").exists()
    }

    /// Ensures the Windows SDK sysroot is acquired and extracted.
    pub fn ensure_sysroot(&self) -> Result<PathBuf> {
        if self.is_installed() {
            return Ok(self.target_dir.clone());
        }

        if self.is_offline {
            bail!(
                "Windows SDK sysroot is missing at `{}` and offline mode is enabled. \
                 Run without offline mode or install via xwin.",
                self.target_dir.display()
            );
        }

        println!("==> Acquiring Windows SDK & MSVC CRT via xwin (official Microsoft CDN)...");
        let xwin_bin = self.ensure_xwin_binary()?;

        let staging = self.target_dir.with_extension("staging");
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        std::fs::create_dir_all(&staging).with_context(|| {
            format!("Failed to create staging directory: {}", staging.display())
        })?;

        std::fs::create_dir_all(&self.cache_download_dir)?;

        let mut cmd = Command::new(&xwin_bin);
        cmd.arg("--accept-license")
            .arg("--cache-dir")
            .arg(&self.cache_download_dir)
            .arg("splat")
            .arg("--output")
            .arg(&staging);

        let status = cmd.status().with_context(|| {
            format!(
                "Failed to execute `{} --accept-license splat`",
                xwin_bin.display()
            )
        })?;

        if !status.success() {
            bail!("`xwin splat` failed with status code: {}", status);
        }

        // Verify required layout was produced
        let crt_include = staging.join("crt").join("include");
        let sdk_include = staging.join("sdk").join("include");
        if !crt_include.exists() || !sdk_include.exists() {
            bail!(
                "`xwin splat` completed but required directories (crt/include, sdk/include) were not found in `{}`",
                staging.display()
            );
        }

        // Write completion marker
        std::fs::write(staging.join(".complete"), b"ok\n")?;

        if self.target_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.target_dir);
        }
        if let Some(parent) = self.target_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&staging, &self.target_dir).with_context(|| {
            format!(
                "Failed to move staged Windows SDK from `{}` to `{}`",
                staging.display(),
                self.target_dir.display()
            )
        })?;

        println!(
            "==> Windows SDK sysroot ready at `{}`",
            self.target_dir.display()
        );
        Ok(self.target_dir.clone())
    }

    fn ensure_xwin_binary(&self) -> Result<PathBuf> {
        if let Ok(bin) = which::which("xwin") {
            return Ok(bin);
        }

        if let Some(home) = dirs::home_dir() {
            let cargo_xwin = home.join(".cargo").join("bin").join("xwin");
            if cargo_xwin.exists() {
                return Ok(cargo_xwin);
            }
        }

        println!("==> `xwin` not found on PATH. Automatically installing via cargo...");
        let status = Command::new("cargo")
            .args(["install", "xwin", "--locked", "--version", "0.10.0"])
            .status()
            .context("Failed to invoke `cargo install xwin`")?;

        if !status.success() {
            bail!("Failed to automatically install `xwin` via cargo.");
        }

        if let Ok(bin) = which::which("xwin") {
            return Ok(bin);
        }

        if let Some(home) = dirs::home_dir() {
            let cargo_xwin = home.join(".cargo").join("bin").join("xwin");
            if cargo_xwin.exists() {
                return Ok(cargo_xwin);
            }
        }

        bail!("`cargo install xwin` finished but `xwin` executable could not be located.");
    }
}
