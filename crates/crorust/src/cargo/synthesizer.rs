use crate::core::contracts::{
    BuildEnvironmentSynthesizer, SynthesizedBuildEnvironment, ToolchainProvider,
};
use anyhow::{Context, Result};
use std::collections::HashMap;

/// Synthesizes the required compiler flags, linkers, and environment overrides
/// for cross-compiling with Cargo and LLVM.
///
/// Rather than requiring host-specific build scripts or proprietary IDEs, this
/// synthesizer queries the [`ToolchainProvider`] for staged SDK headers and link
/// libraries, generating exact `CARGO_TARGET_<TRIPLE>_LINKER` and `-C link-arg=--sysroot`
/// arguments.
#[derive(Clone, Copy, Debug, Default)]
pub struct CargoEnvironmentSynthesizer;

impl BuildEnvironmentSynthesizer for CargoEnvironmentSynthesizer {
    /// Computes the synthesized environment and compiler flags for a given target triple.
    ///
    /// For Android targets, this locates the NDK Clang binary and injects the `--sysroot`
    /// argument pointing to Bionic C headers and libraries.
    /// For Windows targets built on non-Windows hosts, it resolves the Windows SDK sysroot.
    /// For Apple Darwin/iOS targets built on non-macOS hosts, it resolves the Apple SDK.
    fn synthesize(
        &self,
        target_triple: &str,
        toolchains: &dyn ToolchainProvider,
    ) -> Result<SynthesizedBuildEnvironment> {
        let mut flags = Vec::new();
        let mut envs = HashMap::new();
        let mut sysroot_dir = None;

        if target_triple.contains("android") {
            let ndk = toolchains
                .resolve_android_ndk()
                .context("Android NDK resolution failed during environment synthesis")?;

            let (sysroot, clang_bin) = {
                if ndk.join("usr").join("lib").exists() {
                    let parent = ndk.parent().unwrap_or(&ndk);
                    let clang = if cfg!(target_os = "windows") {
                        parent.join("bin").join("clang.exe")
                    } else {
                        parent.join("bin").join("clang")
                    };
                    (ndk.clone(), clang)
                } else {
                    let prebuilt = ndk.join("toolchains").join("llvm").join("prebuilt");
                    let mut found = None;
                    if let Ok(entries) = std::fs::read_dir(&prebuilt) {
                        for entry in entries.flatten() {
                            let candidate_sysroot = entry.path().join("sysroot");
                            if candidate_sysroot.join("usr").join("lib").exists() {
                                let clang = if cfg!(target_os = "windows") {
                                    entry.path().join("bin").join("clang.exe")
                                } else {
                                    entry.path().join("bin").join("clang")
                                };
                                found = Some((candidate_sysroot, clang));
                                break;
                            }
                        }
                    }
                    found.unwrap_or_else(|| {
                        let host_tag = if cfg!(target_os = "windows") {
                            "windows-x86_64"
                        } else if cfg!(target_os = "macos") {
                            "darwin-x86_64"
                        } else {
                            "linux-x86_64"
                        };
                        let clang_name = if cfg!(target_os = "windows") {
                            "clang.exe"
                        } else {
                            "clang"
                        };
                        let fallback_sysroot =
                            ndk.join(format!("toolchains/llvm/prebuilt/{host_tag}/sysroot"));
                        let fallback_clang = ndk.join(format!(
                            "toolchains/llvm/prebuilt/{host_tag}/bin/{clang_name}"
                        ));
                        (fallback_sysroot, fallback_clang)
                    })
                }
            };

            let clean_sysroot = crorust_common::sanitize_path(&sysroot);
            let clean_clang = crorust_common::sanitize_path(&clang_bin);

            flags.push(format!("-C link-arg=--sysroot={clean_sysroot}"));
            envs.insert(
                format!(
                    "CARGO_TARGET_{}_LINKER",
                    target_triple.to_uppercase().replace('-', "_")
                ),
                clean_clang,
            );
            sysroot_dir = Some(sysroot);
        } else if target_triple.contains("windows") && !cfg!(target_os = "windows") {
            if let Ok(win_sdk) = toolchains.resolve_windows_sdk() {
                sysroot_dir = Some(win_sdk);
            }
        } else if (target_triple.contains("darwin") || target_triple.contains("ios"))
            && !cfg!(target_os = "macos")
        {
            if target_triple.contains("ios") {
                if let Ok(sdk) = toolchains.resolve_ios_sdk() {
                    sysroot_dir = Some(sdk);
                }
            } else if let Ok(sdk) = toolchains.resolve_macos_sdk() {
                sysroot_dir = Some(sdk);
            }
        }

        Ok(SynthesizedBuildEnvironment {
            cargo_flags: flags,
            environment_overrides: envs,
            sysroot_directory: sysroot_dir,
            target_triple: target_triple.to_string(),
        })
    }
}
