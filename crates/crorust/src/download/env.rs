use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct EnvSynthesizer {
    prepended_paths: Vec<PathBuf>,
    env_vars: Vec<(OsString, OsString)>,
}

impl EnvSynthesizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepend_path<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.prepended_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn set_var<K: Into<OsString>, V: Into<OsString>>(&mut self, key: K, val: V) -> &mut Self {
        self.env_vars.push((key.into(), val.into()));
        self
    }

    pub fn build_path(&self) -> Result<OsString> {
        let mut all_paths = self.prepended_paths.clone();
        if let Some(existing_path) = std::env::var_os("PATH") {
            all_paths.extend(std::env::split_paths(&existing_path));
        }
        std::env::join_paths(all_paths).context("Failed to join synthesized PATH paths")
    }

    pub fn apply(&self, cmd: &mut Command) -> Result<()> {
        let synthesized_path = self.build_path()?;
        cmd.env("PATH", synthesized_path);

        for (k, v) in &self.env_vars {
            cmd.env(k, v);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinuxDistro {
    StandardFhs,
    NixOs {
        has_nix_ld: bool,
        in_nix_shell: bool,
    },
}

impl LinuxDistro {
    pub fn is_nixos(&self) -> bool {
        matches!(self, Self::NixOs { .. })
    }

    pub fn can_run_precompiled_binaries(&self) -> bool {
        match self {
            Self::StandardFhs => true,
            Self::NixOs { has_nix_ld, .. } => *has_nix_ld,
        }
    }
}

pub fn detect_linux_environment() -> LinuxDistro {
    let is_nixos = Path::new("/etc/NIXOS").exists() || Path::new("/etc/nixos").exists();
    if !is_nixos {
        return LinuxDistro::StandardFhs;
    }

    let has_nix_ld = Path::new("/lib64/ld-linux-x86-64.so.2").exists()
        || Path::new("/run/current-system/sw/share/nix-ld").exists();

    let in_nix_shell =
        std::env::var_os("IN_NIX_SHELL").is_some() || std::env::var_os("NIX_BUILD_TOP").is_some();

    LinuxDistro::NixOs {
        has_nix_ld,
        in_nix_shell,
    }
}

pub fn is_nix_store_path<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().starts_with("/nix/store")
}

pub fn format_nixos_tool_error(tool_name: &str, nix_pkg: &str) -> String {
    format!(
        "\n======================== [crorust: NixOS Detected] ========================\n\
         The tool `{tool_name}` is required for this build step, but cannot run directly.\n\
         Precompiled Linux ELF binaries require an FHS emulator (nix-ld) on NixOS.\n\n\
         To fix this, choose one of the following:\n\n\
         Option A: Add `{nix_pkg}` to your devShell packages in flake.nix:\n\
             devShells.default = pkgs.mkShell {{\n\
               packages = [ pkgs.crorust pkgs.{nix_pkg} ];\n\
             }};\n\n\
         Option B: Enable nix-ld globally in /etc/nixos/configuration.nix:\n\
             programs.nix-ld.enable = true;\n\
         =========================================================================="
    )
}

pub fn check_nixos_tool_execution_for_distro(
    env: LinuxDistro,
    tool_name: &str,
    nix_pkg: &str,
) -> anyhow::Result<()> {
    if let LinuxDistro::NixOs {
        has_nix_ld: false, ..
    } = env
    {
        anyhow::bail!("{}", format_nixos_tool_error(tool_name, nix_pkg));
    }
    Ok(())
}

pub fn check_nixos_tool_execution(tool_name: &str, nix_pkg: &str) -> anyhow::Result<()> {
    check_nixos_tool_execution_for_distro(detect_linux_environment(), tool_name, nix_pkg)
}
