use anyhow::{Context, Result};
#[cfg(feature = "platform-apple")]
use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug)]
pub struct AppleSimulator {
    pub udid: String,
    pub name: String,
    pub state: String,
}

impl AppleSimulator {
    pub fn is_supported() -> bool {
        cfg!(feature = "platform-apple") && which::which("xcrun").is_ok()
    }

    pub fn parse_json(json_str: &str) -> Result<Vec<Self>> {
        #[cfg(feature = "platform-apple")]
        {
            let parsed: Value = serde_json::from_str(json_str)
                .context("Failed to parse JSON output from xcrun simctl")?;

            let mut simulators = Vec::new();
            if let Some(devices_by_runtime) = parsed.get("devices").and_then(|d| d.as_object()) {
                for (_runtime, device_list) in devices_by_runtime {
                    if let Some(devices) = device_list.as_array() {
                        for dev in devices {
                            let udid = dev.get("udid").and_then(|u| u.as_str()).unwrap_or_default();
                            let name = dev.get("name").and_then(|n| n.as_str()).unwrap_or_default();
                            let state = dev
                                .get("state")
                                .and_then(|s| s.as_str())
                                .unwrap_or_default();
                            let is_available = dev
                                .get("isAvailable")
                                .and_then(|a| a.as_bool())
                                .unwrap_or(true);

                            if is_available && !udid.is_empty() {
                                simulators.push(Self {
                                    udid: udid.to_string(),
                                    name: name.to_string(),
                                    state: state.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            Ok(simulators)
        }
        #[cfg(not(feature = "platform-apple"))]
        {
            let _ = json_str;
            Ok(Vec::new())
        }
    }

    pub fn list() -> Result<Vec<Self>> {
        if !Self::is_supported() {
            return Ok(vec![]);
        }

        let output = Command::new("xcrun")
            .args(["simctl", "list", "devices", "-j"])
            .output()
            .context("Failed to run `xcrun simctl list devices -j`")?;

        anyhow::ensure!(
            output.status.success(),
            "xcrun simctl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let json_str = std::str::from_utf8(&output.stdout)
            .context("Output of xcrun simctl list devices -j is not valid UTF-8")?;
        Self::parse_json(json_str)
    }

    pub fn boot(&self) -> Result<()> {
        if self.state != "Booted" {
            let status = Command::new("xcrun")
                .args(["simctl", "boot", &self.udid])
                .status()
                .context("Failed to boot simulator")?;
            anyhow::ensure!(status.success(), "Failed to boot simulator {}", self.udid);
        }
        Ok(())
    }

    pub fn install(&self, app_bundle_path: &Path) -> Result<()> {
        self.boot()?;
        let status = Command::new("xcrun")
            .args(["simctl", "install", &self.udid])
            .arg(app_bundle_path)
            .status()
            .context("Failed to install app on simulator")?;
        anyhow::ensure!(
            status.success(),
            "Failed to install app on simulator {}",
            self.udid
        );
        Ok(())
    }

    pub fn launch(&self, bundle_id: &str, launch_args: &[String]) -> Result<()> {
        self.boot()?;
        let mut cmd = Command::new("xcrun");
        cmd.args(["simctl", "launch", "--console-pty", &self.udid, bundle_id]);
        cmd.args(launch_args);
        let status = cmd.status().context("Failed to launch app on simulator")?;
        anyhow::ensure!(status.success(), "Failed to launch bundle {}", bundle_id);
        Ok(())
    }
}
