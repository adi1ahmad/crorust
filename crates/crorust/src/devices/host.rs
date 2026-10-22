use crate::{TargetArch, TargetPlatform};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalWorkstationHost;

impl LocalWorkstationHost {
    pub fn new() -> Self {
        Self
    }

    pub fn device_name(&self) -> Result<String> {
        if cfg!(target_os = "linux") {
            Ok("Linux Host Workstation".to_string())
        } else if cfg!(target_os = "macos") {
            Ok("macOS Host Workstation".to_string())
        } else if cfg!(target_os = "windows") {
            Ok("Windows Host Workstation".to_string())
        } else {
            Ok("Local Workstation".to_string())
        }
    }

    pub fn target_platform(&self) -> Result<TargetPlatform> {
        TargetPlatform::detect_host()
    }

    pub fn target_arch(&self) -> Result<TargetArch> {
        TargetArch::detect_host()
    }

    pub fn device_details(&self) -> Result<String> {
        if cfg!(target_os = "linux") {
            if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
                for line in content.lines() {
                    if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                        return Ok(val.trim_matches('"').to_string());
                    }
                }
            }
            Ok("Linux Host Generic".to_string())
        } else if cfg!(target_os = "macos") {
            Ok("Apple Darwin / macOS".to_string())
        } else if cfg!(target_os = "windows") {
            Ok("Microsoft Windows".to_string())
        } else {
            Ok(std::env::consts::OS.to_string())
        }
    }

    pub fn run(&self, binary: &Path, launch_args: &[String]) -> Result<()> {
        let status = Command::new(binary)
            .args(launch_args)
            .status()
            .with_context(|| format!("Failed to spawn executable: {}", binary.display()))?;
        if !status.success() {
            bail!("Process exited with status {:?}", status.code());
        }
        Ok(())
    }

    pub fn lldb(&self, executable: &Path) -> Result<()> {
        let status = Command::new("lldb")
            .arg(executable)
            .status()
            .with_context(|| "Failed to spawn LLDB debugger")?;
        if !status.success() {
            bail!("LLDB session finished with exit code {:?}", status.code());
        }
        Ok(())
    }
}
