use crate::devices::{DeviceBackend, TargetDevice};
use crate::{SessionExecutionContext, TargetArch, TargetPlatform};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct AppleMobileDeviceBridge {
    device_id_tool: PathBuf,
    device_info_tool: Option<PathBuf>,
    image_mounter_tool: Option<PathBuf>,
    installer_tool: Option<PathBuf>,
    debug_tool: Option<PathBuf>,
    debug_server_proxy_tool: Option<PathBuf>,
}

impl AppleMobileDeviceBridge {
    pub fn discover_host_tools() -> Result<Self> {
        let device_id_tool = which::which(crorust_common::host_executable_name("idevice_id"))
            .context("idevice_id executable not found on host PATH")?;
        let device_info_tool =
            which::which(crorust_common::host_executable_name("ideviceinfo")).ok();
        let image_mounter_tool =
            which::which(crorust_common::host_executable_name("ideviceimagemounter")).ok();
        let installer_tool =
            which::which(crorust_common::host_executable_name("ideviceinstaller")).ok();
        let debug_tool = which::which(crorust_common::host_executable_name("idevicedebug")).ok();
        let debug_server_proxy_tool = which::which(crorust_common::host_executable_name(
            "idevicedebugserverproxy",
        ))
        .ok();

        Ok(Self {
            device_id_tool,
            device_info_tool,
            image_mounter_tool,
            installer_tool,
            debug_tool,
            debug_server_proxy_tool,
        })
    }

    pub fn which() -> Result<Self> {
        Self::discover_host_tools()
    }

    fn query_device_key(&self, udid: &str, property_key: &str) -> Result<String> {
        let tool = self
            .device_info_tool
            .as_ref()
            .context("ideviceinfo executable not found on host PATH")?;
        let output = Command::new(tool)
            .args(["--udid", udid, "--key", property_key])
            .output()
            .with_context(|| {
                format!("Failed to invoke ideviceinfo for UDID {udid} and key {property_key}")
            })?;

        if !output.status.success() {
            bail!(
                "ideviceinfo query failed for key '{}': {}",
                property_key,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn install_app_bundle(&self, udid: &str, app_path: &Path) -> Result<()> {
        let tool = self
            .installer_tool
            .as_ref()
            .context("ideviceinstaller executable not found on host PATH")?;
        let status = Command::new(tool)
            .args(["--udid", udid, "--install"])
            .arg(app_path)
            .status()
            .with_context(|| {
                format!(
                    "Failed to execute ideviceinstaller for {}",
                    app_path.display()
                )
            })?;

        if !status.success() {
            bail!("ideviceinstaller failed with status {:?}", status.code());
        }
        Ok(())
    }

    fn launch_app(&self, udid: &str, bundle_id: &str, args: &[String]) -> Result<()> {
        let tool = self
            .debug_tool
            .as_ref()
            .context("idevicedebug executable not found on host PATH")?;
        let status = Command::new(tool)
            .args(["--udid", udid, "run", bundle_id])
            .args(args)
            .status()
            .with_context(|| format!("Failed to launch bundle {bundle_id} via idevicedebug"))?;

        if !status.success() {
            bail!(
                "idevicedebug launch failed with exit status {:?}",
                status.code()
            );
        }
        Ok(())
    }

    fn is_disk_image_mounted(&self, udid: &str) -> Result<bool> {
        let tool = self
            .image_mounter_tool
            .as_ref()
            .context("ideviceimagemounter executable not found on host PATH")?;
        let output = Command::new(tool)
            .args(["--udid", udid, "-l"])
            .output()
            .context("Failed to query mounted disk images via ideviceimagemounter")?;

        if !output.status.success() {
            bail!("ideviceimagemounter listing failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some((_, rest)) = stdout.split_once('[') {
            if let Some((num_str, _)) = rest.split_once(']') {
                if let Ok(count) = num_str.trim().parse::<u32>() {
                    return Ok(count > 0);
                }
            }
        }
        Ok(false)
    }

    pub fn mount_disk_image(&self, env: &SessionExecutionContext, udid: &str) -> Result<()> {
        if self.is_disk_image_mounted(udid)? {
            return Ok(());
        }
        let (major, minor) = self.product_version(udid)?;
        let image_path = env.resolve_developer_disk_image(major, minor);
        let tool = self
            .image_mounter_tool
            .as_ref()
            .context("ideviceimagemounter executable not found on host PATH")?;
        let status = Command::new(tool)
            .args(["--udid", udid])
            .arg(image_path)
            .status()
            .context("Failed to mount developer disk image")?;

        if !status.success() {
            bail!(
                "ideviceimagemounter mount operation failed with status {:?}",
                status.code()
            );
        }
        Ok(())
    }

    pub fn run(
        &self,
        env: &SessionExecutionContext,
        udid: &str,
        path: &Path,
        launch_args: &[String],
    ) -> Result<()> {
        let bundle_identifier = crorust_apple::extract_bundle_identifier_from_package(path)?;
        self.mount_disk_image(env, udid)?;
        self.install_app_bundle(udid, path)?;
        self.launch_app(udid, &bundle_identifier, launch_args)?;
        Ok(())
    }

    pub fn query_connected_devices(&self, devices: &mut Vec<TargetDevice>) -> Result<()> {
        let output = Command::new(&self.device_id_tool)
            .args(["-l", "-d"])
            .output()
            .context("Failed to query connected iOS devices via idevice_id")?;

        if !output.status.success() {
            bail!("idevice_id failed with status {:?}", output.status.code());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let udid = line.trim();
            if !udid.is_empty() {
                devices.push(TargetDevice {
                    backend: DeviceBackend::Apple(self.clone()),
                    id: udid.to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn query_device_name(&self, udid: &str) -> Result<String> {
        self.query_device_key(udid, "DeviceName")
    }

    pub fn query_device_platform(&self, _udid: &str) -> Result<TargetPlatform> {
        Ok(TargetPlatform::Ios)
    }

    pub fn query_device_arch(&self, udid: &str) -> Result<TargetArch> {
        let arch = self.query_device_key(udid, "CPUArchitecture")?;
        match arch.as_str() {
            "arm64" | "arm64e" => Ok(TargetArch::Arm64),
            other => bail!("Unsupported iOS CPU architecture: {other}"),
        }
    }

    pub fn parse_product_version_str(raw: &str) -> Result<(u32, u32)> {
        let parts: Vec<&str> = raw.trim().split('.').collect();
        if parts.is_empty() {
            bail!("Empty product version string");
        }
        let major = parts[0]
            .parse::<u32>()
            .context("Invalid iOS major version")?;
        let minor = if parts.len() > 1 {
            let minor_cleaned = parts[1].split('-').next().unwrap_or(parts[1]);
            minor_cleaned.parse::<u32>().unwrap_or(0)
        } else {
            0
        };
        Ok((major, minor))
    }

    pub fn product_version(&self, udid: &str) -> Result<(u32, u32)> {
        let raw = self.query_device_key(udid, "ProductVersion")?;
        Self::parse_product_version_str(&raw)
    }

    pub fn query_device_details(&self, udid: &str) -> Result<String> {
        let model = self
            .query_device_key(udid, "ProductName")
            .unwrap_or_else(|_| "iOS Device".to_string());
        let version = self
            .query_device_key(udid, "ProductVersion")
            .unwrap_or_default();
        Ok(format!("{model} ({version})"))
    }

    pub fn query_installed_bundle_path(&self, udid: &str, bundle_id: &str) -> Result<PathBuf> {
        let tool = self
            .installer_tool
            .as_ref()
            .context("ideviceinstaller executable not found on host PATH")?;
        let output = Command::new(tool)
            .args(["--udid", udid, "-l", "-o", "xml"])
            .output()
            .context("Failed listing installed applications on iOS device")?;

        if !output.status.success() {
            bail!("ideviceinstaller list command failed");
        }

        let plist_val: plist::Value = plist::from_reader_xml(&*output.stdout)
            .context("Failed to parse device installed apps plist")?;

        if let Some(apps) = plist_val.as_array() {
            for app in apps {
                if let Some(dict) = app.as_dictionary() {
                    let id_match = dict.get("CFBundleIdentifier").and_then(|v| v.as_string());
                    if id_match == Some(bundle_id) {
                        if let Some(path_str) = dict.get("Path").and_then(|v| v.as_string()) {
                            return Ok(PathBuf::from(path_str));
                        }
                    }
                }
            }
        }
        bail!("Installed application bundle '{bundle_id}' not found on device {udid}")
    }

    pub fn start_debug_server_proxy(&self, udid: &str, port: u16) -> Result<()> {
        let tool = self
            .debug_server_proxy_tool
            .as_ref()
            .context("idevicedebugserverproxy executable not found on host PATH")?;
        let mut cmd = Command::new(tool);
        cmd.args(["--udid", udid, "--lldb", &port.to_string()]);
        std::thread::spawn(move || {
            let _ = cmd.status();
        });
        Ok(())
    }

    pub fn lldb(&self, env: &SessionExecutionContext, udid: &str, path: &Path) -> Result<()> {
        let bundle_id = crorust_apple::extract_bundle_identifier_from_package(path)?;
        self.mount_disk_image(env, udid)?;
        self.install_app_bundle(udid, path)?;

        let debug_port: u16 = 1234;
        let device_app_path = self.query_installed_bundle_path(udid, &bundle_id)?;
        let work_dir = path
            .parent()
            .context("App bundle has no valid parent directory")?;

        let sdk_path = env.resolve_ios_sdk();
        let sdk_str = sdk_path
            .to_str()
            .context("iOS SDK path contains invalid UTF-8")?;

        let disk_app = path.join(env.app_name());
        let disk_app_str = disk_app
            .to_str()
            .context("Local app path contains invalid UTF-8")?;
        let dev_app_str = device_app_path
            .to_str()
            .context("Device app path contains invalid UTF-8")?;

        let py_script_path = work_dir.join("crorust_lldb.py");
        let py_str = py_script_path
            .to_str()
            .context("LLDB python script path contains invalid UTF-8")?;

        let lldb_commands = format!(
            "settings set target.process.stop-on-exec false\n\
             platform select remote-ios --sysroot {}\n\
             target create \"{}\"\n\
             command script import \"{}\"\n\
             script session = crorust_lldb.AppleDebugSession(lldb.debugger, \"connect://127.0.0.1:{}\", \"{}\")\n\
             script session.connect_and_run()\n\
             script session.monitor_loop()\n\
             quit\n",
            sdk_str, disk_app_str, py_str, debug_port, dev_app_str
        );

        std::fs::write(work_dir.join("crorust_lldb.cmd"), lldb_commands)?;
        std::fs::write(&py_script_path, include_str!("../../../scripts/lldb.py"))?;

        self.start_debug_server_proxy(udid, debug_port)?;

        let status = Command::new("lldb")
            .current_dir(work_dir)
            .args(["-s", "crorust_lldb.cmd"])
            .status()
            .context("Failed executing LLDB interactive debugger")?;

        if !status.success() {
            bail!("LLDB session exited with status {:?}", status.code());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ios_product_version_parsing() -> Result<()> {
        assert_eq!(
            AppleMobileDeviceBridge::parse_product_version_str("17.0")?,
            (17, 0)
        );
        assert_eq!(
            AppleMobileDeviceBridge::parse_product_version_str("17.4.1")?,
            (17, 4)
        );
        assert_eq!(
            AppleMobileDeviceBridge::parse_product_version_str("18.1.0-beta")?,
            (18, 1)
        );
        assert!(AppleMobileDeviceBridge::parse_product_version_str("invalid").is_err());
        Ok(())
    }
}
