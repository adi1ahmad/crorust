use crate::devices::apple::AppleMobileDeviceBridge;
use crate::devices::host::LocalWorkstationHost;
use crate::{SessionExecutionContext, TargetArch, TargetPlatform};
use anyhow::{bail, Context, Result};
use std::path::Path;

pub mod adb;
pub mod apple;
pub mod host;

pub use adb::AndroidDebugBridgeController;
pub use apple::AppleSimulator;

/// Supported target device backends.
#[derive(Clone, Debug)]
pub enum DeviceBackend {
    /// Android physical phone or emulator connected via ADB.
    Android(AndroidDebugBridgeController),
    /// Physical Apple device (iPhone, iPad) connected over USB via `usbmuxd`.
    Apple(AppleMobileDeviceBridge),
    /// Current development workstation.
    Host(LocalWorkstationHost),
    /// Apple Silicon or Intel iOS Simulator managed via `simctl`.
    Simulator(AppleSimulator),
}

/// Represents an execution destination (physical phone, simulator, or local host).
#[derive(Clone, Debug)]
pub struct TargetDevice {
    pub(crate) id: String,
    pub(crate) backend: DeviceBackend,
}

impl std::str::FromStr for TargetDevice {
    type Err = anyhow::Error;

    fn from_str(selector: &str) -> Result<Self> {
        Self::parse_target_device(selector)
    }
}

impl std::fmt::Display for TargetDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.backend {
            DeviceBackend::Android(_) => write!(f, "android:{}", self.id),
            DeviceBackend::Host(_) => write!(f, "{}", self.id),
            DeviceBackend::Apple(_) => write!(f, "ios:{}", self.id),
            DeviceBackend::Simulator(_) => write!(f, "sim:{}", self.id),
        }
    }
}

impl TargetDevice {
    pub fn discover_available_devices() -> Result<Vec<Self>> {
        let mut targets = vec![Self::local_host_device()];

        if let Ok(adb) = AndroidDebugBridgeController::which() {
            let _ = adb.query_connected_devices(&mut targets);
        }
        if let Ok(apple) = AppleMobileDeviceBridge::which() {
            let _ = apple.query_connected_devices(&mut targets);
        }
        if AppleSimulator::is_supported() {
            if let Ok(simulators) = AppleSimulator::list() {
                for sim in simulators {
                    if sim.state == "Booted" {
                        targets.push(TargetDevice {
                            id: sim.udid.clone(),
                            backend: DeviceBackend::Simulator(sim),
                        });
                    }
                }
            }
        }

        Ok(targets)
    }

    pub fn list() -> Result<Vec<Self>> {
        Self::discover_available_devices()
    }

    pub fn list_connected_targets() -> Result<Vec<Self>> {
        let all = Self::list()?;
        Ok(all.into_iter().filter(|d| !d.is_host()).collect())
    }

    pub fn auto_select(platform_filter: Option<TargetPlatform>) -> Result<Self> {
        let connected = Self::list_connected_targets()?;

        if let Some(target_platform) = platform_filter {
            if target_platform == TargetPlatform::Android || target_platform == TargetPlatform::Ios
            {
                let matching = connected
                    .into_iter()
                    .find(|d| d.platform().map(|p| p == target_platform).unwrap_or(false));

                matching.ok_or_else(|| {
                    anyhow::anyhow!(
                        "No connected {} devices or emulators found.\n\
                         Troubleshooting:\n\
                         1. For Android: verify with `adb devices` or start an Android Virtual Device.\n\
                         2. For iOS: verify USB connection and trust pairing, or boot an iOS Simulator.\n\
                         3. Run `crorust devices` to list recognized devices.",
                        target_platform
                    )
                })
            } else {
                Ok(Self::host())
            }
        } else if let Some(first_target) = connected.into_iter().next() {
            Ok(first_target)
        } else {
            Ok(Self::host())
        }
    }

    pub fn first_connected(platform_filter: Option<TargetPlatform>) -> Result<Self> {
        let targets = Self::list_connected_targets()?;

        if let Some(target_platform) = platform_filter {
            let matched = targets
                .into_iter()
                .find(|d| d.platform().map(|p| p == target_platform).unwrap_or(false));

            matched.ok_or_else(|| {
                anyhow::anyhow!(
                    "No connected {} hardware or virtual devices detected.",
                    target_platform
                )
            })
        } else {
            targets.into_iter().next().ok_or_else(|| {
                anyhow::anyhow!(
                    "No mobile or remote execution targets connected. Ensure USB debugging is enabled."
                )
            })
        }
    }

    pub fn parse_target_device(selector: &str) -> Result<Self> {
        let trimmed = selector.trim();
        match trimmed {
            "host" => Ok(Self::local_host_device()),
            "first" | "auto" | "connected" => Self::first_connected(None),
            "android" => Self::first_connected(Some(TargetPlatform::Android)),
            "ios" => Self::first_connected(Some(TargetPlatform::Ios)),
            _ => {
                if let Some((prefix, ident)) = trimmed.split_once(':') {
                    let backend = match prefix {
                        "adb" | "android" => {
                            DeviceBackend::Android(AndroidDebugBridgeController::which()?)
                        }
                        "imd" | "ios" | "apple" => {
                            DeviceBackend::Apple(AppleMobileDeviceBridge::which()?)
                        }
                        "sim" | "simulator" => {
                            let list = AppleSimulator::list()?;
                            let sim = list
                                .into_iter()
                                .find(|s| s.udid == ident)
                                .with_context(|| format!("Simulator '{ident}' not found"))?;
                            DeviceBackend::Simulator(sim)
                        }
                        "host" => DeviceBackend::Host(LocalWorkstationHost::new()),
                        other => bail!("Unknown device backend protocol '{other}'"),
                    };
                    return Ok(Self {
                        id: ident.to_string(),
                        backend,
                    });
                }

                let all_targets = Self::discover_available_devices()?;
                if let Some(found) = all_targets.into_iter().find(|d| d.id == trimmed) {
                    return Ok(found);
                }

                if let Ok(adb) = AndroidDebugBridgeController::which() {
                    return Ok(Self {
                        id: trimmed.to_string(),
                        backend: DeviceBackend::Android(adb),
                    });
                }

                bail!("Could not resolve device target '{trimmed}'")
            }
        }
    }

    pub fn resolve(selector: &str) -> Result<Self> {
        Self::parse_target_device(selector)
    }

    pub fn local_host_device() -> Self {
        Self {
            id: "host".to_string(),
            backend: DeviceBackend::Host(LocalWorkstationHost::new()),
        }
    }

    pub fn current_host() -> Self {
        Self::local_host_device()
    }

    pub fn host() -> Self {
        Self::local_host_device()
    }

    pub fn is_local_workstation(&self) -> bool {
        matches!(&self.backend, DeviceBackend::Host(_))
    }

    pub fn is_host(&self) -> bool {
        self.is_local_workstation()
    }

    pub fn targets_current_host(&self) -> bool {
        self.is_local_workstation()
    }

    pub fn device_name(&self) -> Result<String> {
        match &self.backend {
            DeviceBackend::Android(adb) => adb.query_device_name(&self.id),
            DeviceBackend::Host(h) => h.device_name(),
            DeviceBackend::Apple(apple) => apple.query_device_name(&self.id),
            DeviceBackend::Simulator(s) => Ok(s.name.clone()),
        }
    }

    pub fn name(&self) -> Result<String> {
        self.device_name()
    }

    pub fn target_platform(&self) -> Result<TargetPlatform> {
        match &self.backend {
            DeviceBackend::Android(adb) => adb.query_device_platform(&self.id),
            DeviceBackend::Host(h) => h.target_platform(),
            DeviceBackend::Apple(apple) => apple.query_device_platform(&self.id),
            DeviceBackend::Simulator(_) => Ok(TargetPlatform::Ios),
        }
    }

    pub fn platform(&self) -> Result<TargetPlatform> {
        self.target_platform()
    }

    pub fn target_arch(&self) -> Result<TargetArch> {
        match &self.backend {
            DeviceBackend::Android(adb) => adb.query_device_arch(&self.id),
            DeviceBackend::Host(h) => h.target_arch(),
            DeviceBackend::Apple(apple) => apple.query_device_arch(&self.id),
            DeviceBackend::Simulator(_) => TargetArch::detect_host(),
        }
    }

    pub fn arch(&self) -> Result<TargetArch> {
        self.target_arch()
    }

    pub fn device_details(&self) -> Result<String> {
        match &self.backend {
            DeviceBackend::Android(adb) => adb.query_device_details(&self.id),
            DeviceBackend::Host(h) => h.device_details(),
            DeviceBackend::Apple(apple) => apple.query_device_details(&self.id),
            DeviceBackend::Simulator(s) => Ok(format!("iOS Simulator ({}) - {}", s.state, s.name)),
        }
    }

    pub fn details(&self) -> Result<String> {
        self.device_details()
    }

    pub fn run(
        &self,
        env: &SessionExecutionContext,
        path: &Path,
        launch_args: &[String],
    ) -> Result<()> {
        match &self.backend {
            DeviceBackend::Android(adb) => adb.run(
                &self.id,
                path,
                launch_args,
                &env.manifest_config().android_platform_config().debug,
                false,
            ),
            DeviceBackend::Host(h) => h.run(path, launch_args),
            DeviceBackend::Apple(apple) => apple.run(env, &self.id, path, launch_args),
            DeviceBackend::Simulator(sim) => {
                sim.install(path)?;
                let bundle_id = crorust_apple::extract_bundle_identifier_from_package(path)?;
                sim.launch(&bundle_id, launch_args)?;
                Ok(())
            }
        }
    }

    pub fn lldb(
        &self,
        env: &SessionExecutionContext,
        executable: &Path,
        lldb_server: Option<&Path>,
    ) -> Result<()> {
        match &self.backend {
            DeviceBackend::Android(adb) => {
                if let Some(server) = lldb_server {
                    adb.lldb(&self.id, executable, server)
                } else {
                    bail!("lldb-server binary path required for remote Android debugging");
                }
            }
            DeviceBackend::Host(h) => h.lldb(executable),
            DeviceBackend::Apple(apple) => apple.lldb(env, &self.id, executable),
            DeviceBackend::Simulator(_) => {
                bail!("LLDB debugging on iOS simulator is not currently supported")
            }
        }
    }

    pub fn target_ios_product_version(&self) -> Result<(u32, u32)> {
        match &self.backend {
            DeviceBackend::Apple(apple) => apple.product_version(&self.id),
            DeviceBackend::Simulator(_) => Ok((17, 0)),
            _ => bail!("Target is not an iOS device"),
        }
    }

    pub fn ios_product_version(&self) -> Result<(u32, u32)> {
        self.target_ios_product_version()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_resolve_host() {
        let dev = TargetDevice::resolve("host").unwrap();
        assert!(dev.targets_current_host());
        assert_eq!(dev.id, "host");
    }

    #[test]
    fn test_device_resolve_explicit_adb() {
        if AndroidDebugBridgeController::which().is_err() {
            return;
        }
        let dev = TargetDevice::resolve("adb:emulator-5554").unwrap();
        assert!(!dev.targets_current_host());
        assert_eq!(dev.id, "emulator-5554");
    }

    #[test]
    fn test_device_auto_select_fallback_to_host() {
        let dev = TargetDevice::auto_select(None).unwrap();
        if TargetDevice::list_connected_targets().unwrap().is_empty() {
            assert!(dev.targets_current_host());
        }
    }
}
