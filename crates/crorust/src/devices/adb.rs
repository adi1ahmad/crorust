use crate::config::AndroidDebugIdentityConfig;
use crate::devices::{DeviceBackend, TargetDevice};
use crate::{SessionExecutionContext, TargetArch, TargetPlatform};
use anyhow::{bail, Context, Result};
use crorust_android::AndroidPackageContainer;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AndroidDebugBridgeController(PathBuf);

impl AndroidDebugBridgeController {
    pub fn locate_adb_executable() -> Result<Self> {
        Self::locate_with_optional_env(None)
    }

    pub fn which() -> Result<Self> {
        Self::locate_adb_executable()
    }

    pub fn locate_with_optional_env(env: Option<&SessionExecutionContext>) -> Result<Self> {
        if let Ok(adb_path) = which::which(crorust_common::host_executable_name("adb")) {
            return Ok(Self(adb_path));
        }

        if let Some(env) = env {
            let sdk_adb = env
                .locate_android_sdk()
                .join("platform-tools")
                .join(crorust_common::host_executable_name("adb"));
            if sdk_adb.exists() {
                return Ok(Self(sdk_adb));
            }

            let host_platform = crate::TargetPlatform::detect_host()?;
            if let Some(artifact) =
                crate::download::catalog::ToolCatalog::get_platform_tools(host_platform)
            {
                let tool_mgr = crate::download::manager::ToolManager::new(env)?;
                let tool_dir = tool_mgr.ensure_tool(&artifact)?;
                let managed_adb = tool_dir.join(crorust_common::host_executable_name("adb"));
                if managed_adb.exists() {
                    return Ok(Self(managed_adb));
                }
            }
        }

        bail!(
            "ADB executable not found on system PATH, in ANDROID_HOME, or in managed tools.\n\
             Ensure an Android device or emulator is connected, or run with internet to auto-provision."
        )
    }

    fn adb_cmd(&self, serial: &str) -> Command {
        let mut cmd = Command::new(&self.0);
        cmd.args(["-s", serial]);
        cmd
    }

    fn push_file(&self, serial: &str, local_path: &Path) -> Result<()> {
        let status = self
            .adb_cmd(serial)
            .args(["push", "--sync"])
            .arg(local_path)
            .arg(Path::new("/data/local/tmp"))
            .status()
            .with_context(|| {
                format!("Failed to push {} to device {serial}", local_path.display())
            })?;

        if !status.success() {
            bail!("ADB push failed with exit status {:?}", status.code());
        }
        Ok(())
    }

    fn shell_cmd(&self, serial: &str, run_as_package: Option<&str>) -> Command {
        let mut cmd = self.adb_cmd(serial);
        cmd.args(["shell"]);
        if let Some(pkg) = run_as_package {
            cmd.args(["run-as", pkg]);
        }
        cmd
    }

    pub fn query_connected_devices(&self, devices: &mut Vec<TargetDevice>) -> Result<()> {
        let query_cmd = Command::new(&self.0)
            .args(["devices"])
            .output()
            .context("Failed running 'adb devices'")?;

        if !query_cmd.status.success() {
            let stderr_str = String::from_utf8_lossy(&query_cmd.stderr);
            bail!(
                "adb devices inquiry failed with code {:?}: {}",
                query_cmd.status.code(),
                stderr_str.trim()
            );
        }

        let stdout = String::from_utf8_lossy(&query_cmd.stdout);
        for line in stdout.lines().skip(1) {
            let mut cols = line.split_whitespace();
            if let (Some(serial), Some(state)) = (cols.next(), cols.next()) {
                if state == "device" {
                    devices.push(TargetDevice {
                        backend: DeviceBackend::Android(self.clone()),
                        id: serial.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    fn query_prop(&self, serial: &str, property_name: &str) -> Result<String> {
        let output = self
            .shell_cmd(serial, None)
            .args(["getprop", property_name])
            .output()
            .with_context(|| {
                format!("Failed to read property '{property_name}' on device {serial}")
            })?;

        if !output.status.success() {
            bail!(
                "getprop failed for '{property_name}': {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn install_package(&self, serial: &str, apk_path: &Path) -> Result<()> {
        let file_name = apk_path
            .file_name()
            .and_then(|f| f.to_str())
            .context("Invalid package file name")?;

        self.push_file(serial, apk_path)?;
        let remote_dest = format!("/data/local/tmp/{file_name}");

        let output = self
            .shell_cmd(serial, None)
            .args(["pm", "install", "-r", "-d", &remote_dest])
            .output()
            .context("Failed executing pm install on device")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() || stdout.contains("Failure [") {
            bail!(
                "ADB package installation failed: {}\n{}",
                stdout.trim(),
                stderr.trim()
            );
        }
        Ok(())
    }

    fn start_activity(
        &self,
        serial: &str,
        target_package: &str,
        target_activity: &str,
        cli_args: &[String],
    ) -> Result<()> {
        let mut cmd = self.shell_cmd(serial, None);
        cmd.args([
            "am",
            "start",
            "-a",
            "android.intent.action.MAIN",
            "-n",
            &format!("{target_package}/{target_activity}"),
        ]);

        let escaped_args = shlex::try_join(cli_args.iter().map(String::as_str))
            .context("Failed quoting launch arguments for ADB")?;
        if !escaped_args.is_empty() {
            cmd.arg(escaped_args);
        }

        let status = cmd.status().context("Failed to spawn am start on device")?;
        if !status.success() {
            bail!("am start exited with error code {:?}", status.code());
        }
        Ok(())
    }

    fn force_stop(&self, serial: &str, target_package: &str) -> Result<()> {
        let status = self
            .shell_cmd(serial, None)
            .args(["am", "force-stop", target_package])
            .status()
            .context("Failed running am force-stop")?;

        if !status.success() {
            bail!("am force-stop exited with code {:?}", status.code());
        }
        Ok(())
    }

    fn configure_port_forwards(
        &self,
        serial: &str,
        debug_cfg: &AndroidDebugIdentityConfig,
    ) -> Result<()> {
        for (local, remote) in &debug_cfg.forward {
            let status = self
                .adb_cmd(serial)
                .args(["forward", local, remote])
                .status()?;
            if !status.success() {
                bail!("adb forward failed for {local} -> {remote}");
            }
        }
        for (remote, local) in &debug_cfg.reverse {
            let status = self
                .adb_cmd(serial)
                .args(["reverse", remote, local])
                .status()?;
            if !status.success() {
                bail!("adb reverse failed for {remote} -> {local}");
            }
        }
        Ok(())
    }

    fn set_debug_app(&self, serial: &str, package: &str) -> Result<()> {
        let status = self
            .shell_cmd(serial, None)
            .args(["am", "set-debug-app", "-w", package])
            .status()?;
        if !status.success() {
            bail!("am set-debug-app failed");
        }
        Ok(())
    }

    fn clear_debug_app(&self, serial: &str) -> Result<()> {
        let status = self
            .shell_cmd(serial, None)
            .args(["am", "clear-debug-app"])
            .status()?;
        if !status.success() {
            bail!("am clear-debug-app failed");
        }
        Ok(())
    }

    fn query_device_timestamp(&self, serial: &str) -> Result<String> {
        let output = self
            .shell_cmd(serial, None)
            .args(["date", "'+%m-%d %T.000'"])
            .output()?;

        if !output.status.success() {
            bail!("Failed to query device timestamp via adb date");
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn resolve_package_uid(&self, serial: &str, package_id: &str) -> Result<u32> {
        let output = self
            .shell_cmd(serial, None)
            .args(["pm", "list", "package", "-U", package_id])
            .output()?;

        if !output.status.success() {
            bail!("pm list package failed for {package_id}");
        }

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some((pkg_part, uid_part)) = line.split_once(' ') {
                if pkg_part.strip_prefix("package:") == Some(package_id) {
                    if let Some(uid_str) = uid_part.strip_prefix("uid:") {
                        return Ok(uid_str.trim().parse::<u32>()?);
                    }
                }
            }
        }
        bail!("Could not resolve package UID for {package_id}")
    }

    fn stream_logcat(&self, serial: &str, uid: u32, since: &str) -> Result<AndroidLogcatStream> {
        let child = self
            .adb_cmd(serial)
            .args(["logcat", "-T", since, &format!("--uid={uid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to spawn logcat listener")?;

        Ok(AndroidLogcatStream::new(child))
    }

    pub fn forward_tcp_port(&self, serial: &str, remote_port: u16) -> Result<u16> {
        let output = self
            .adb_cmd(serial)
            .args(["forward", "tcp:0", &format!("tcp:{remote_port}")])
            .output()?;

        if !output.status.success() {
            bail!("adb forward failed");
        }
        let local_port = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u16>()?;
        Ok(local_port)
    }

    pub fn lldb(&self, serial: &str, executable: &Path, lldb_server: &Path) -> Result<()> {
        self.push_file(serial, lldb_server)?;
        let remote_server_bin = Path::new("./lldb-server");

        let mut server_process = self
            .shell_cmd(serial, None)
            .args([
                "cd",
                "/data/local/tmp",
                "&&",
                "chmod",
                "777",
                remote_server_bin.to_str().unwrap(),
                "&&",
                remote_server_bin.to_str().unwrap(),
                "platform",
                "--listen",
                "*:10086",
                "--server",
            ])
            .stdin(Stdio::null())
            .spawn()
            .context("Failed to launch remote lldb-server on device")?;

        std::thread::sleep(Duration::from_millis(150));
        self.forward_tcp_port(serial, 10086)?;

        let lldb_session = Command::new("lldb")
            .args([
                "-O",
                "platform select remote-android",
                "-O",
                &format!("platform connect connect://{serial}:10086"),
            ])
            .arg(executable)
            .status()
            .context("Failed executing host LLDB debugger")?;

        let _ = server_process.kill();
        if !lldb_session.success() {
            bail!(
                "LLDB debugging session exited with error code {:?}",
                lldb_session.code()
            );
        }
        Ok(())
    }

    pub fn run(
        &self,
        serial: &str,
        apk_path: &Path,
        launch_args: &[String],
        debug_cfg: &AndroidDebugIdentityConfig,
        is_debug: bool,
    ) -> Result<()> {
        let entry_point = AndroidPackageContainer::query_entry_point(apk_path)?;
        let target_package = &entry_point.package;
        let target_activity = &entry_point.activity;

        self.force_stop(serial, target_package)?;
        if is_debug {
            self.set_debug_app(serial, target_package)?;
        } else {
            self.clear_debug_app(serial)?;
        }

        self.install_package(serial, apk_path)?;
        self.configure_port_forwards(serial, debug_cfg)?;

        let timestamp = self.query_device_timestamp(serial)?;
        self.start_activity(serial, target_package, target_activity, launch_args)?;

        let uid = self.resolve_package_uid(serial, target_package)?;
        let logcat_stream = self.stream_logcat(serial, uid, &timestamp)?;

        for log_line in logcat_stream {
            println!("{log_line}");
        }
        Ok(())
    }

    pub fn query_device_name(&self, serial: &str) -> Result<String> {
        self.query_prop(serial, "ro.product.device")
    }

    pub fn query_device_platform(&self, _serial: &str) -> Result<TargetPlatform> {
        Ok(TargetPlatform::Android)
    }

    pub fn query_device_arch(&self, serial: &str) -> Result<TargetArch> {
        let abi = self.query_prop(serial, "ro.product.cpu.abi")?;
        match abi.as_str() {
            "arm64-v8a" => Ok(TargetArch::Arm64),
            "x86_64" => Ok(TargetArch::X64),
            unsupported => bail!("Unsupported Android CPU ABI: {unsupported}"),
        }
    }

    pub fn query_device_details(&self, serial: &str) -> Result<String> {
        let release = self.query_prop(serial, "ro.build.version.release")?;
        let sdk_api = self.query_prop(serial, "ro.build.version.sdk")?;
        Ok(format!("Android {release} (API {sdk_api})"))
    }
}

pub struct AndroidLogcatStream {
    stdout_stream_reader: BufReader<ChildStdout>,
    line_accumulator: String,
    logcat_subprocess: Child,
}

impl AndroidLogcatStream {
    fn new(mut process: Child) -> Self {
        let stdout = process
            .stdout
            .take()
            .expect("Child process missing stdout pipe");
        Self {
            logcat_subprocess: process,
            stdout_stream_reader: BufReader::new(stdout),
            line_accumulator: String::with_capacity(1024),
        }
    }
}

impl Iterator for AndroidLogcatStream {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.line_accumulator.clear();
        let read_res = self
            .stdout_stream_reader
            .read_line(&mut self.line_accumulator);
        match read_res {
            Ok(0) => None,
            Ok(_) => {
                let trimmed = self.line_accumulator.trim();
                if let Some((_date, rest)) = trimmed.split_once(' ') {
                    if let Some((_time, msg)) = rest.split_once(' ') {
                        return Some(msg.to_string());
                    }
                }
                Some(self.line_accumulator.clone())
            }
            Err(_) => None,
        }
    }
}

impl Drop for AndroidLogcatStream {
    fn drop(&mut self) {
        let _ = self.logcat_subprocess.kill();
    }
}
