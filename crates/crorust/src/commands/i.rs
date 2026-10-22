use crate::core::target::{TargetArch, TargetCompilationUnit, TargetOpt, TargetPlatform};
use crate::download::catalog::{ToolArtifact, ToolCatalog, ToolCategory};
use crate::download::manager::ToolManager;
use anyhow::{Context, Result};
use clap::Parser;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Clone, Debug, Default)]
pub struct IArgs {
    #[clap(long)]
    pub fix: bool,

    /// With --fix: exit successfully even if some required tools could not be provisioned
    #[clap(long)]
    pub allow_partial: bool,

    #[clap(long)]
    pub clean_cache: bool,

    /// Run host system package manager (apt, dnf, pacman, brew, winget) to install missing system tools
    #[clap(long)]
    pub system: bool,

    /// Explicitly override cache directory destination
    #[clap(long)]
    pub cache_dir: Option<PathBuf>,

    /// Force global cache (~/.cache/crorust/) instead of default project-local .cache
    #[clap(long)]
    pub global_cache: bool,

    /// Force project-local cache (`<project>`/.cache/crorust/)
    #[clap(long)]
    pub project_cache: bool,

    #[clap(short = 'C', long)]
    pub project: Option<PathBuf>,

    #[clap(long)]
    pub offline: bool,

    #[clap(long)]
    pub emit_sideload_manifest: bool,

    #[clap(long)]
    pub sideload_out: Option<PathBuf>,
}

pub struct ToolStatus {
    pub name: String,
    pub system_version: Option<String>,
    pub managed_version: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SystemIEngine {
    audit_groups: Vec<DiagnosticCategory>,
}

impl SystemIEngine {
    pub fn check_managed_tools(&self, cache_roots: &[PathBuf]) -> Vec<ToolStatus> {
        let mut statuses = vec![];

        let has_complete = |subpath: &Path| -> Option<String> {
            for root in cache_roots {
                if root.join(subpath).join(".complete").exists() {
                    return Some(format!("Cached ({})", root.display()));
                }
                let tools_dir = root.join("tools");
                if tools_dir.join(subpath).join(".complete").exists() {
                    return Some(format!("Cached ({})", root.display()));
                }
                if let Ok(entries) = std::fs::read_dir(&tools_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() && p.join(subpath).join(".complete").exists() {
                            return Some(format!("Cached ({})", root.display()));
                        }
                    }
                }
                let sysroots_dir = root.join("sysroots");
                if sysroots_dir.join(subpath).join(".complete").exists() {
                    return Some(format!("Cached ({})", root.display()));
                }
                if root.join(subpath).is_dir() {
                    return Some(format!("Cached ({})", root.display()));
                }
            }
            None
        };

        let jdk_loc = has_complete(&Path::new("jdk").join("17.0.10_7"))
            .or_else(|| has_complete(&Path::new("jdk").join("17.0.10+7")));
        let gradle_loc = has_complete(&Path::new("gradle").join("8.7"));
        let adb_loc = has_complete(&Path::new("platform-tools").join("35.0.0"))
            .or_else(|| has_complete(&Path::new("platform-tools").join("35.0.1")));
        let kotlin_loc = has_complete(&Path::new("kotlinc").join("2.1.0"))
            .or_else(|| has_complete(&Path::new("kotlin").join("1.9.22")));
        let r8_loc = has_complete(&Path::new("r8").join("9.5.26"))
            .or_else(|| has_complete(&Path::new("d8").join("34.0.0")));
        let ndk_loc = has_complete(&Path::new("ndk").join("26.3.11579264"))
            .or_else(|| has_complete(Path::new("Android.ndk")));

        let win_sdk_loc = has_complete(&Path::new("Windows.sdk").join("10.0.22621.0"))
            .or_else(|| has_complete(Path::new("Windows.sdk")));
        let mac_sdk_loc = has_complete(&Path::new("MacOSX.sdk").join("14.0"))
            .or_else(|| has_complete(Path::new("MacOSX.sdk")));
        let ios_sdk_loc = has_complete(&Path::new("iPhoneOS.sdk").join("17.0"))
            .or_else(|| has_complete(Path::new("iPhoneOS.sdk")));
        let sim_sdk_loc = has_complete(&Path::new("iPhoneSimulator.sdk").join("17.0"))
            .or_else(|| has_complete(Path::new("iPhoneSimulator.sdk")));
        let lin_x64_loc = has_complete(&Path::new("Linux.sysroot-x86_64").join("2.31"))
            .or_else(|| has_complete(&Path::new("Linux.sysroot-x86_64").join("2.35")))
            .or_else(|| has_complete(Path::new("Linux.sysroot-x86_64")));
        let lin_arm_loc = has_complete(&Path::new("Linux.sysroot-aarch64").join("2.31"))
            .or_else(|| has_complete(&Path::new("Linux.sysroot-aarch64").join("2.35")))
            .or_else(|| has_complete(Path::new("Linux.sysroot-aarch64")));

        statuses.push(ToolStatus {
            name: "JDK 17".into(),
            system_version: which::which("java").ok().map(|_| "System Java".into()),
            managed_version: jdk_loc.map(|loc| format!("17.0.10_7 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Kotlin Compiler (kotlinc)".into(),
            system_version: which::which("kotlinc")
                .ok()
                .map(|_| "System kotlinc".into()),
            managed_version: kotlin_loc.map(|loc| format!("2.1.0 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Android D8/R8 Dexer".into(),
            system_version: which::which("d8").ok().map(|_| "System d8".into()),
            managed_version: r8_loc.map(|loc| format!("9.5.26 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Android NDK".into(),
            system_version: std::env::var_os("ANDROID_NDK_ROOT")
                .or_else(|| std::env::var_os("ANDROID_NDK_HOME"))
                .map(|_| "Env ANDROID_NDK".into()),
            managed_version: ndk_loc.map(|loc| format!("26.3.11579264 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "ADB (Platform Tools)".into(),
            system_version: which::which(crorust_common::host_executable_name("adb"))
                .ok()
                .map(|_| "System ADB".into()),
            managed_version: adb_loc.map(|loc| format!("35.0.0 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Gradle (Legacy Fallback)".into(),
            system_version: which::which("gradle").ok().map(|_| "System Gradle".into()),
            managed_version: gradle_loc.map(|loc| format!("8.7 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Windows SDK (Sysroot)".into(),
            system_version: None,
            managed_version: win_sdk_loc.map(|loc| format!("10.0.22621.0 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "macOS SDK (Sysroot)".into(),
            system_version: None,
            managed_version: mac_sdk_loc.map(|loc| format!("14.0 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "iOS Device SDK (Sysroot)".into(),
            system_version: None,
            managed_version: ios_sdk_loc.map(|loc| format!("17.0 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "iOS Simulator SDK (Sysroot)".into(),
            system_version: None,
            managed_version: sim_sdk_loc.map(|loc| format!("17.0 [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Linux Sysroot x86_64".into(),
            system_version: None,
            managed_version: lin_x64_loc.map(|loc| format!("glibc [{loc}]")),
        });

        statuses.push(ToolStatus {
            name: "Linux Sysroot aarch64".into(),
            system_version: None,
            managed_version: lin_arm_loc.map(|loc| format!("glibc [{loc}]")),
        });

        statuses
    }

    pub fn missing_required_tools(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        for group in &self.audit_groups {
            for probe in &group.probes {
                if probe.optional_note.is_none() && probe.resolve_binary_path().is_err() {
                    missing.push(probe.binary_identifier());
                }
            }
        }
        missing
    }

    pub fn audit_offline_readiness(
        &self,
        cache_roots: &[PathBuf],
        host_platform: TargetPlatform,
        host_arch: TargetArch,
        min_sdk: Option<u32>,
        emit_sideload: bool,
        sideload_out_path: Option<&Path>,
    ) -> Result<bool> {
        let mut missing_items: Vec<SideloadItem> = Vec::new();

        println!("\n================== [crorust: Offline Readiness Audit] ==================");
        println!("Host:          {} {}", host_platform, host_arch);
        println!(
            "Cache Roots:   {}",
            cache_roots
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("------------------------------------------------------------------------");

        self.check_offline_artifact(
            cache_roots,
            ToolCatalog::get_jdk(host_platform, host_arch),
            &mut missing_items,
        );
        self.check_offline_artifact(
            cache_roots,
            ToolCatalog::get_r8("9.5.26"),
            &mut missing_items,
        );
        self.check_offline_artifact(
            cache_roots,
            ToolCatalog::get_kotlinc("2.1.0"),
            &mut missing_items,
        );
        self.check_offline_artifact(
            cache_roots,
            ToolCatalog::get_platform_tools(host_platform),
            &mut missing_items,
        );
        self.check_offline_artifact(
            cache_roots,
            Some(ToolCatalog::get_ndk()),
            &mut missing_items,
        );

        let min_sdk_val = min_sdk.unwrap_or(23);
        if min_sdk_val < 26 {
            self.check_offline_artifact(
                cache_roots,
                ToolCatalog::get_desugar_jdk_libs("2.1.4"),
                &mut missing_items,
            );
            self.check_offline_artifact(
                cache_roots,
                ToolCatalog::get_desugar_jdk_libs_config("2.1.4"),
                &mut missing_items,
            );
        }

        if missing_items.is_empty() {
            println!("✓ Status: Workstation is 100% READY for offline compilation.");
            println!("  All required SDKs, compilers, and library descriptors are present with valid .complete markers.");
            return Ok(true);
        }

        println!("✗ Status: Workstation is NOT ready for offline compilation.");
        println!(
            "  {} required toolchain component(s) are missing from local cache.\n",
            missing_items.len()
        );

        for item in &missing_items {
            println!("  - Missing: {} v{}", item.name, item.version);
            println!("    Expected Path: {}", item.target_cache_dir.display());
            println!("    SHA-256:       {}", item.sha256);
            println!("    Download URL:  {}\n", item.url);
        }

        if emit_sideload {
            let out_file = sideload_out_path
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("sideload_tools.sh"));

            let mut script = String::from(
                "#!/usr/bin/env bash\n# crorust Air-Gapped Sideload Script\nset -euo pipefail\n\n",
            );
            for item in &missing_items {
                let filename = item.url.rsplit('/').next().unwrap_or("tool.tar.gz");
                script.push_str(&format!(
                    "# Tool: {} v{}\necho \"Fetching {}...\"\ncurl -Lo \"{}\" \"{}\"\necho \"{}  {}\" | sha256sum -c -\n\n",
                    item.name, item.version, item.name, filename, item.url, item.sha256, filename
                ));
            }

            if let Some(parent) = out_file.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&out_file, &script)?;
            println!(
                "✓ Generated air-gapped download script: {}",
                out_file.display()
            );

            let ps1_file = out_file.with_extension("ps1");
            let mut ps1_script = String::from(
                "# crorust Air-Gapped Sideload Script (Windows PowerShell)\n$ErrorActionPreference = 'Stop'\n\n",
            );
            for item in &missing_items {
                let filename = item.url.rsplit('/').next().unwrap_or("tool.zip");
                ps1_script.push_str(&format!(
                    "# Tool: {} v{}\nWrite-Host \"Fetching {}...\"\nInvoke-WebRequest -Uri \"{}\" -OutFile \"{}\"\n$hash = (Get-FileHash -Path \"{}\" -Algorithm SHA256).Hash.ToLower()\nif ($hash -ne \"{}\") {{ throw \"Checksum mismatch for {}! Expected {}, got $hash\" }}\n\n",
                    item.name, item.version, item.name, item.url, filename, filename, item.sha256.to_lowercase(), item.name, item.sha256.to_lowercase()
                ));
            }
            std::fs::write(&ps1_file, ps1_script)?;
            println!(
                "✓ Generated air-gapped PowerShell script: {}",
                ps1_file.display()
            );
        }

        Ok(false)
    }

    fn check_offline_artifact(
        &self,
        cache_roots: &[PathBuf],
        artifact: Option<ToolArtifact>,
        missing: &mut Vec<SideloadItem>,
    ) {
        let Some(art) = artifact else { return };

        let mut is_installed = false;
        for root in cache_roots {
            let canonical = match art.category {
                ToolCategory::Sysroot => root.join("sysroots").join(art.name).join(art.version),
                ToolCategory::Ndk => root.join("tools").join("ndk").join(art.version),
                _ => root.join("tools").join(art.name).join(art.version),
            };

            if canonical.join(".complete").exists()
                || root
                    .join(art.name)
                    .join(art.version)
                    .join(".complete")
                    .exists()
            {
                is_installed = true;
                break;
            }

            let tools_dir = root.join("tools");
            if tools_dir
                .join(art.name)
                .join(art.version)
                .join(".complete")
                .exists()
            {
                is_installed = true;
                break;
            }
            if let Ok(entries) = std::fs::read_dir(&tools_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir()
                        && p.join(art.name)
                            .join(art.version)
                            .join(".complete")
                            .exists()
                    {
                        is_installed = true;
                        break;
                    }
                }
            }
            if is_installed {
                break;
            }
        }

        if is_installed {
            println!("  [INSTALLED] {} v{}", art.name, art.version);
        } else {
            let primary_root = cache_roots
                .first()
                .cloned()
                .unwrap_or_else(|| PathBuf::from(".cache/crorust"));
            let target_dir = match art.category {
                ToolCategory::Sysroot => primary_root
                    .join("sysroots")
                    .join(art.name)
                    .join(art.version),
                ToolCategory::Ndk => primary_root.join("tools").join("ndk").join(art.version),
                _ => primary_root.join("tools").join(art.name).join(art.version),
            };
            println!("  [MISSING]   {} v{}", art.name, art.version);
            missing.push(SideloadItem {
                name: art.name,
                version: art.version,
                url: art.url,
                sha256: art.sha256,
                target_cache_dir: target_dir,
            });
        }
    }
}

pub struct SideloadItem {
    pub name: &'static str,
    pub version: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub target_cache_dir: PathBuf,
}

impl Default for SystemIEngine {
    fn default() -> Self {
        Self {
            audit_groups: vec![
                DiagnosticCategory {
                    name: "LLVM / Clang Toolchain",
                    probes: vec![
                        DiagnosticToolProbe::required("clang", "--version"),
                        DiagnosticToolProbe::required("clang++", "--version"),
                        DiagnosticToolProbe::unversioned("llvm-ar"),
                        DiagnosticToolProbe::unversioned("llvm-lib"),
                        DiagnosticToolProbe::required("llvm-readobj", "--version"),
                        DiagnosticToolProbe::required("lld", "--version"),
                        DiagnosticToolProbe::required("lld-link", "--version"),
                        DiagnosticToolProbe::required("lldb", "--version"),
                        DiagnosticToolProbe::unversioned("lldb-server"),
                    ],
                },
                DiagnosticCategory {
                    name: "Rust Toolchain",
                    probes: vec![
                        DiagnosticToolProbe::required("rustup", "--version"),
                        DiagnosticToolProbe::required("cargo", "--version"),
                    ],
                },
                DiagnosticCategory {
                    name: "Android Platform",
                    probes: vec![
                        DiagnosticToolProbe::required("adb", "version"),
                        DiagnosticToolProbe::required("javac", "-version"),
                        DiagnosticToolProbe::required("java", "-version"),
                        DiagnosticToolProbe::required("kotlinc", "-version"),
                        DiagnosticToolProbe::new_optional(
                            "gradle",
                            "--version",
                            "only needed when android.use_gradle=true",
                        ),
                    ],
                },
                DiagnosticCategory {
                    name: "Apple Darwin Platform",
                    probes: vec![
                        DiagnosticToolProbe::required("idevice_id", "-v"),
                        DiagnosticToolProbe::required("ideviceinfo", "-v"),
                        DiagnosticToolProbe::required("ideviceinstaller", "-v"),
                        DiagnosticToolProbe::required("ideviceimagemounter", "-v"),
                        DiagnosticToolProbe::required("idevicedebug", "-v"),
                        DiagnosticToolProbe::required("idevicedebugserverproxy", "-v"),
                    ],
                },
                DiagnosticCategory {
                    name: "Linux Distribution Packaging",
                    probes: vec![DiagnosticToolProbe::new_optional(
                        "mksquashfs",
                        "-version",
                        "pure-Rust backhand fallback available",
                    )],
                },
            ],
        }
    }
}

impl std::fmt::Display for SystemIEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for category in &self.audit_groups {
            write!(f, "{category}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct DiagnosticCategory {
    probes: Vec<DiagnosticToolProbe>,
    name: &'static str,
}

impl std::fmt::Display for DiagnosticCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "[Toolchain Probe Category: {}]", self.name)?;
        for probe in &self.probes {
            write!(f, "  - {:18} ", probe.binary_identifier())?;
            if let Ok(path) = probe.resolve_binary_path() {
                let ver = probe
                    .query_version()
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "detected".into());
                write!(f, "{ver:18} (located at {})", path.display())?;
            } else if let Some(hint) = probe.optional_note {
                write!(f, "not found (optional: {hint})")?;
            } else {
                write!(f, "missing from system PATH")?;
            }
            writeln!(f)?;
        }
        writeln!(f)
    }
}

#[derive(Clone, Copy, Debug)]
struct DiagnosticToolProbe {
    binary_name: &'static str,
    version_arg: Option<&'static str>,
    optional_note: Option<&'static str>,
}

impl DiagnosticToolProbe {
    pub const fn required(name: &'static str, version_arg: &'static str) -> Self {
        Self {
            binary_name: name,
            version_arg: Some(version_arg),
            optional_note: None,
        }
    }

    pub const fn unversioned(name: &'static str) -> Self {
        Self {
            binary_name: name,
            version_arg: None,
            optional_note: None,
        }
    }

    pub const fn new_optional(
        name: &'static str,
        version_arg: &'static str,
        hint: &'static str,
    ) -> Self {
        Self {
            binary_name: name,
            version_arg: Some(version_arg),
            optional_note: Some(hint),
        }
    }

    fn binary_identifier(self) -> &'static str {
        self.binary_name
    }

    fn resolve_binary_path(self) -> Result<PathBuf> {
        Ok(which::which(self.binary_name)?)
    }

    fn query_version(self) -> Result<Option<String>> {
        let Some(arg) = self.version_arg else {
            return Ok(None);
        };

        let output = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", self.binary_name]);
            cmd.args(arg.split_whitespace());
            cmd.output()?
        } else {
            let mut cmd = Command::new(self.binary_name);
            cmd.args(arg.split_whitespace());
            cmd.output()?
        };

        if !output.status.success() {
            anyhow::bail!("Probe command for '{}' failed", self.binary_name);
        }

        let raw = String::from_utf8_lossy(&output.stdout);
        let text = if raw.trim().is_empty() {
            String::from_utf8_lossy(&output.stderr)
        } else {
            raw
        };

        for word in text.split_whitespace() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
            if trimmed.contains('.')
                && trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            {
                return Ok(Some(trimmed.to_string()));
            }
        }

        let first = text.lines().next().unwrap_or("").trim();
        if !first.is_empty() {
            return Ok(Some(first.to_string()));
        }

        Ok(Some("detected".to_string()))
    }
}

fn compute_file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn audit_and_clean_cache(cache_root: &Path, _label: &str) -> Result<(usize, u64)> {
    if !cache_root.exists() {
        return Ok((0, 0));
    }

    crate::commands::cache::prune_cache_dir(cache_root);
    let mut removed_count = 0usize;
    let mut freed_bytes = 0u64;

    let download_dir = cache_root.join("download");
    if download_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&download_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if let Some(expected_sha) = ToolCatalog::expected_sha256_for_filename(fname) {
                    match compute_file_sha256(&path) {
                        Ok(actual_sha) if actual_sha.eq_ignore_ascii_case(expected_sha) => {}
                        Ok(actual_sha) => {
                            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                            println!(
                                "  [clean-cache] Removing corrupted archive {} (expected {}, got {})",
                                path.display(),
                                &expected_sha[..8],
                                &actual_sha[..8]
                            );
                            let _ = std::fs::remove_file(&path);
                            removed_count += 1;
                            freed_bytes += size;
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }

    let tools_dir = cache_root.join("tools");
    if tools_dir.is_dir() {
        if let Ok(hosts) = std::fs::read_dir(&tools_dir) {
            for host_entry in hosts.flatten() {
                let host_path = host_entry.path();
                if !host_path.is_dir() {
                    continue;
                }
                if let Ok(categories) = std::fs::read_dir(&host_path) {
                    for cat_entry in categories.flatten() {
                        let cat_path = cat_entry.path();
                        if !cat_path.is_dir() {
                            continue;
                        }
                        if let Ok(versions) = std::fs::read_dir(&cat_path) {
                            for ver_entry in versions.flatten() {
                                let ver_path = ver_entry.path();
                                if ver_path.is_dir() && !ver_path.join(".complete").exists() {
                                    let size = crate::commands::cache::compute_dir_size(&ver_path);
                                    println!(
                                        "  [clean-cache] Removing incomplete tool extraction {}",
                                        ver_path.display()
                                    );
                                    let _ = std::fs::remove_dir_all(&ver_path);
                                    removed_count += 1;
                                    freed_bytes += size;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let sysroots_dir = cache_root.join("sysroots");
    if sysroots_dir.is_dir() {
        if let Ok(sdk_entries) = std::fs::read_dir(&sysroots_dir) {
            for sdk_entry in sdk_entries.flatten() {
                let sdk_path = sdk_entry.path();
                if !sdk_path.is_dir() {
                    continue;
                }
                if let Ok(versions) = std::fs::read_dir(&sdk_path) {
                    for ver_entry in versions.flatten() {
                        let ver_path = ver_entry.path();
                        if ver_path.is_dir() && !ver_path.join(".complete").exists() {
                            let size = crate::commands::cache::compute_dir_size(&ver_path);
                            println!(
                                "  [clean-cache] Removing incomplete sysroot extraction {}",
                                ver_path.display()
                            );
                            let _ = std::fs::remove_dir_all(&ver_path);
                            removed_count += 1;
                            freed_bytes += size;
                        }
                    }
                }
            }
        }
    }

    Ok((removed_count, freed_bytes))
}

pub const UNIVERSAL_TARGET_TRIPLES: &[&str] = &[
    "aarch64-linux-android",
    "x86_64-linux-android",
    "armv7-linux-androideabi",
    "i686-linux-android",
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
];

fn ensure_rustup_targets(triples: &[&str]) -> Result<Vec<String>> {
    if which::which("rustup").is_err() {
        println!("  [i --fix] `rustup` not found on PATH; skipping Rust target installation.");
        return Ok(vec![]);
    }

    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .context("Failed to run `rustup target list --installed`")?;

    let installed: HashSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .collect();

    let mut newly_added = Vec::new();
    for &triple in triples {
        if !installed.contains(triple) {
            println!("  [i --fix] Installing missing Rust target `{triple}`...");
            let status = Command::new("rustup")
                .args(["target", "add", triple])
                .status()
                .with_context(|| format!("Failed to invoke `rustup target add {triple}`"))?;
            if status.success() {
                newly_added.push(triple.to_string());
            } else {
                println!("  [i --fix] Warning: `rustup target add {triple}` exited with non-zero status.");
            }
        }
    }

    Ok(newly_added)
}

fn ensure_rustup_components(components: &[&str]) -> Result<Vec<String>> {
    if which::which("rustup").is_err() {
        println!("  [i --fix] `rustup` not found on PATH; skipping Rustup component installation.");
        return Ok(vec![]);
    }

    let output = Command::new("rustup")
        .args(["component", "list", "--installed"])
        .output()
        .context("Failed to run `rustup component list --installed`")?;

    let installed: HashSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
        .collect();

    let mut newly_added = Vec::new();
    for &comp in components {
        if !installed.iter().any(|c| c.starts_with(comp)) {
            println!("  [i --fix] Installing missing Rustup component `{comp}`...");
            let status = Command::new("rustup")
                .args(["component", "add", comp])
                .status()
                .with_context(|| format!("Failed to invoke `rustup component add {comp}`"))?;
            if status.success() {
                newly_added.push(comp.to_string());
            } else {
                println!("  [i --fix] Warning: `rustup component add {comp}` exited with non-zero status.");
            }
        }
    }

    Ok(newly_added)
}

pub fn determine_info_target_cache(
    args: &IArgs,
    project_root: Option<&Path>,
    manifest: Option<&crate::config::ManifestConfig>,
) -> PathBuf {
    if let Some(custom) = &args.cache_dir {
        return dunce::simplified(custom).to_path_buf();
    }
    if args.global_cache {
        return crate::SessionExecutionContext::global_cache_dir();
    }
    if args.project_cache {
        let base = project_root
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        return base.join(".cache").join("crorust");
    }
    if let Ok(env_path) = std::env::var("CRORUST_CACHE_DIR") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(env_scope) = std::env::var("CRORUST_CACHE_SCOPE") {
        if let Ok(scope) = env_scope.parse::<crate::config::CacheScope>() {
            match scope {
                crate::config::CacheScope::Global => {
                    return crate::SessionExecutionContext::global_cache_dir();
                }
                crate::config::CacheScope::Project => {
                    let base = project_root.map(PathBuf::from).unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                    });
                    return base.join(".cache").join("crorust");
                }
            }
        }
    }
    if let Some(m) = manifest {
        if let Some(explicit) = m.toolchain.explicit_cache_dir() {
            if explicit.is_absolute() {
                return explicit.to_path_buf();
            } else if let Some(root) = project_root {
                return root.join(explicit);
            } else {
                return explicit.to_path_buf();
            }
        }
        if m.toolchain.cache_scope() == crate::config::CacheScope::Global {
            return crate::SessionExecutionContext::global_cache_dir();
        }
    }
    // Strict default: Always local .cache/crorust
    let base = project_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    base.join(".cache").join("crorust")
}

#[derive(Debug, Default)]
struct ProvisioningReport {
    succeeded: Vec<(String, PathBuf)>,
    failed: Vec<(String, String)>,
    /// Tools we deliberately do not download (nothing to fetch, host provides them).
    skipped: Vec<(String, String)>,
}

/// Tools whose absence is not an error: the host normally supplies them.
const OPTIONAL_TOOLS: &[&str] = &["iOS Simulator SDK"];

/// Decides whether a provisioning run counts as a failure.
///
/// Returns an error if any required platform tools could not be provisioned
/// unless partial provisioning is explicitly allowed.
fn evaluate_report(report: &ProvisioningReport, allow_partial: bool) -> Result<()> {
    let required_failed: Vec<&str> = report
        .failed
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !OPTIONAL_TOOLS.contains(name))
        .collect();
    if required_failed.is_empty() || allow_partial {
        return Ok(());
    }
    anyhow::bail!(
        "Provisioning failed for required tools: {}. \
         Re-run with --allow-partial to continue without them.",
        required_failed.join(", ")
    )
}

fn provision_artifact_with_verification(
    mgr: &ToolManager,
    artifact: &crate::download::catalog::ToolArtifact,
    display_title: &str,
    report: &mut ProvisioningReport,
) {
    print!(
        "  [i --fix] Provisioning {} (v{})... ",
        display_title, artifact.version
    );
    let _ = std::io::stdout().flush();

    // A catalog entry without a URL has nothing to download (host-provided,
    // e.g. the iOS Simulator SDK from Xcode). Trying anyway only produced
    // "relative URL without a base".
    if artifact.url.is_empty() {
        let why =
            "no download source; provided by the host (Xcode) or set an SDK path in the manifest";
        println!("\x1b[33m- skipped ({why})\x1b[0m");
        report
            .skipped
            .push((display_title.to_string(), why.to_string()));
        return;
    }

    match mgr.ensure_tool(artifact) {
        Ok(dir) => {
            println!("\x1b[32m✓ ready at {}\x1b[0m", dir.display());
            report.succeeded.push((display_title.to_string(), dir));
        }
        Err(err) => {
            println!("\x1b[31m✗ failed ({err:#})\x1b[0m");
            report
                .failed
                .push((display_title.to_string(), format!("{err:#}")));
        }
    }
}

fn provision_managed_tools(
    cache_root: &Path,
    host_triple: &str,
    mirror: Option<String>,
    allow_partial: bool,
) -> Result<()> {
    let host_platform = TargetPlatform::detect_host()?;
    let host_arch = TargetArch::detect_host()?;
    let mgr = ToolManager::with_cache_dir(cache_root.to_path_buf(), host_triple, mirror, false)?;

    println!(
        "\n  [i --fix] Provisioning hermetic compilers & tools into {}...",
        cache_root.display()
    );

    let mut report = ProvisioningReport::default();

    // 1. Managed Compilers & Bytecode Tools
    if let Some(jdk) = ToolCatalog::get_jdk(host_platform, host_arch) {
        provision_artifact_with_verification(&mgr, &jdk, "JDK 17", &mut report);
    }
    if let Some(kotlin) = ToolCatalog::get_kotlinc("2.1.0") {
        provision_artifact_with_verification(&mgr, &kotlin, "Kotlin Compiler", &mut report);
    }
    if let Some(r8) = ToolCatalog::get_r8("9.5.26") {
        provision_artifact_with_verification(&mgr, &r8, "Android D8/R8 Dexer", &mut report);
    }
    if let Some(pt) = ToolCatalog::get_platform_tools(host_platform) {
        provision_artifact_with_verification(
            &mgr,
            &pt,
            "Android Platform Tools (ADB)",
            &mut report,
        );
    }
    if let Some(gradle) = ToolCatalog::get_gradle("8.7") {
        provision_artifact_with_verification(&mgr, &gradle, "Gradle 8.7", &mut report);
    }
    if let Some(desugar) = ToolCatalog::get_desugar_jdk_libs("2.1.4") {
        provision_artifact_with_verification(
            &mgr,
            &desugar,
            "Android Desugar JDK Libs",
            &mut report,
        );
    }
    if let Some(desugar_cfg) = ToolCatalog::get_desugar_jdk_libs_config("2.1.4") {
        provision_artifact_with_verification(
            &mgr,
            &desugar_cfg,
            "Android Desugar Config",
            &mut report,
        );
    }

    // 2. Cross-Target Managed Sysroots & SDKs
    println!("\n  [i --fix] Provisioning cross-compilation target sysroots...");

    let ndk = ToolCatalog::get_ndk();
    provision_artifact_with_verification(&mgr, &ndk, "Android NDK", &mut report);

    let sysroot_names = [
        ("Windows.sdk", "Windows SDK"),
        ("MacOSX.sdk", "macOS SDK"),
        ("iPhoneOS.sdk", "iOS Device SDK"),
        ("iPhoneSimulator.sdk", "iOS Simulator SDK"),
        ("Linux.sysroot-x86_64", "Linux Sysroot x86_64"),
        ("Linux.sysroot-aarch64", "Linux Sysroot aarch64"),
    ];

    for (sdk_key, sdk_title) in sysroot_names {
        if sdk_key == "Windows.sdk" {
            let xwin_mgr = crate::download::xwin::XWinManager::with_cache_dir(
                cache_root,
                "10.0.22621.0",
                false,
            );
            print!("  [i --fix] Provisioning Windows SDK (v10.0.22621.0)... ");
            let _ = std::io::stdout().flush();
            match xwin_mgr.ensure_sysroot() {
                Ok(sdk_dir) => {
                    println!("\x1b[32m✓ ready at {}\x1b[0m", sdk_dir.display());
                    report.succeeded.push((sdk_title.to_string(), sdk_dir));
                }
                Err(e) => {
                    println!("\x1b[31m✗ failed ({})\x1b[0m", e);
                    report.failed.push((sdk_title.to_string(), e.to_string()));
                }
            }
        } else if sdk_key == "MacOSX.sdk" {
            let dest_dir = cache_root.join("sysroots").join("MacOSX.sdk").join("14.0");
            if dest_dir.join(".complete").exists() {
                println!(
                    "  [i --fix] Provisioning macOS SDK (v14.0)... \x1b[32m✓ ready at {}\x1b[0m",
                    dest_dir.display()
                );
                report.succeeded.push((sdk_title.to_string(), dest_dir));
            } else {
                print!("  [i --fix] Provisioning macOS SDK (v14.0)... ");
                let _ = std::io::stdout().flush();
                match crate::download::apple_catalog::AppleCdnResolver::resolve_clt_sdk_package_url(
                ) {
                    Ok(package_url) => {
                        let download_dir = cache_root.join("download");
                        let _ = std::fs::create_dir_all(&download_dir);
                        let pkg_filename = package_url
                            .rsplit_once('/')
                            .map(|(_, f)| f)
                            .unwrap_or("CLTools_macOS_SDK.pkg");
                        let pkg_path = download_dir.join(pkg_filename);
                        let res = (|| -> Result<()> {
                            let client = crate::download::create_hardened_http_client()?;
                            if !pkg_path.exists() {
                                let verifier =
                                    crate::download::VerifiableDownload::new(&package_url, None);
                                verifier.download(&client, &pkg_path)?;
                            }
                            crate::download::apple_pkg::ApplePkgExtractor::extract_pkg(
                                &pkg_path, &dest_dir,
                            )?;
                            std::fs::write(dest_dir.join(".complete"), b"ok\n")?;
                            Ok(())
                        })();

                        match res {
                            Ok(()) => {
                                println!("\x1b[32m✓ ready at {}\x1b[0m", dest_dir.display());
                                report.succeeded.push((sdk_title.to_string(), dest_dir));
                            }
                            Err(e) => {
                                println!("\x1b[31m✗ failed ({})\x1b[0m", e);
                                report.failed.push((sdk_title.to_string(), e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        println!("\x1b[31m✗ failed ({})\x1b[0m", e);
                        report.failed.push((sdk_title.to_string(), e.to_string()));
                    }
                }
            }
        } else if let Some(sysroot) = ToolCatalog::get_sysroot(sdk_key) {
            provision_artifact_with_verification(&mgr, &sysroot, sdk_title, &mut report);
        }
    }

    // Print Provisioning Summary
    println!("\n  [i --fix] Provisioning Summary:");
    println!(
        "    ✓ Successfully provisioned & verified: {} tools",
        report.succeeded.len()
    );
    if !report.skipped.is_empty() {
        println!("    - Skipped: {} tools", report.skipped.len());
        for (name, why) in &report.skipped {
            println!("       - {}: {}", name, why);
        }
    }
    if !report.failed.is_empty() {
        println!(
            "    ⚠️  Failed / deferred to runtime: {} tools",
            report.failed.len()
        );
        for (name, err) in &report.failed {
            let tag = if OPTIONAL_TOOLS.contains(&name.as_str()) {
                " (optional)"
            } else {
                ""
            };
            println!("       - {}{}: {}", name, tag, err);
        }
    }

    evaluate_report(&report, allow_partial)
}

fn execute_system_package_installation(missing: &[&str]) -> Result<()> {
    if missing.is_empty() {
        return Ok(());
    }

    let needs_llvm = missing.iter().any(|t| {
        matches!(
            *t,
            "clang"
                | "clang++"
                | "llvm-ar"
                | "llvm-lib"
                | "llvm-readobj"
                | "lld"
                | "lld-link"
                | "lldb"
                | "lldb-server"
        )
    });
    let needs_imobile = missing.iter().any(|t| t.starts_with("idevice"));

    if !needs_llvm && !needs_imobile {
        return Ok(());
    }

    println!("\n  [i --fix --system] Installing host system packages...");
    if cfg!(target_os = "linux") {
        if Path::new("/etc/NIXOS").exists() || Path::new("/nix/store").exists() {
            println!("  [i --fix --system] NixOS detected: Run `nix develop` in the repository root to enter the hermetic devShell.");
        } else if which::which("apt-get").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["clang", "llvm", "lld", "lldb"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice-utils", "ideviceinstaller"]);
            }
            println!(
                "  [i --fix --system] Running: sudo apt-get update && sudo apt-get install -y {}",
                pkgs.join(" ")
            );
            let _ = Command::new("sudo").args(["apt-get", "update"]).status();
            let mut cmd = Command::new("sudo");
            cmd.args(["apt-get", "install", "-y"]);
            cmd.args(&pkgs);
            let status = cmd.status().context("Failed to invoke apt-get install")?;
            if !status.success() {
                println!(
                    "  [i --fix --system] Warning: apt-get install exited with non-zero status."
                );
            }
        } else if which::which("dnf").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["clang", "llvm", "lld", "lldb"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice-utils", "ideviceinstaller"]);
            }
            println!(
                "  [i --fix --system] Running: sudo dnf install -y {}",
                pkgs.join(" ")
            );
            let mut cmd = Command::new("sudo");
            cmd.args(["dnf", "install", "-y"]);
            cmd.args(&pkgs);
            let status = cmd.status().context("Failed to invoke dnf install")?;
            if !status.success() {
                println!("  [i --fix --system] Warning: dnf install exited with non-zero status.");
            }
        } else if which::which("pacman").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["clang", "llvm", "lld", "lldb"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice", "ideviceinstaller"]);
            }
            println!(
                "  [i --fix --system] Running: sudo pacman -S --noconfirm {}",
                pkgs.join(" ")
            );
            let mut cmd = Command::new("sudo");
            cmd.args(["pacman", "-S", "--noconfirm"]);
            cmd.args(&pkgs);
            let status = cmd.status().context("Failed to invoke pacman")?;
            if !status.success() {
                println!("  [i --fix --system] Warning: pacman exited with non-zero status.");
            }
        }
    } else if cfg!(target_os = "macos") {
        if which::which("brew").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["llvm", "lld"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice", "ideviceinstaller"]);
            }
            println!(
                "  [i --fix --system] Running: brew install {}",
                pkgs.join(" ")
            );
            let mut cmd = Command::new("brew");
            cmd.arg("install");
            cmd.args(&pkgs);
            let status = cmd.status().context("Failed to invoke brew install")?;
            if !status.success() {
                println!("  [i --fix --system] Warning: brew install exited with non-zero status.");
            }
        } else if needs_llvm {
            println!("  [i --fix --system] Running: xcode-select --install");
            let _ = Command::new("xcode-select").arg("--install").status();
        }
    } else if cfg!(target_os = "windows") && needs_llvm && which::which("winget").is_ok() {
        println!("  [i --fix --system] Running: winget install LLVM.LLVM --silent");
        let status = Command::new("winget")
            .args([
                "install",
                "LLVM.LLVM",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ])
            .status()
            .context("Failed to invoke winget")?;
        if !status.success() {
            println!("  [i --fix --system] Warning: winget exited with non-zero status.");
        }
    }
    Ok(())
}

fn print_system_remediation_hints(missing: &[&str]) {
    if missing.is_empty() {
        return;
    }

    let needs_llvm = missing.iter().any(|t| {
        matches!(
            *t,
            "clang"
                | "clang++"
                | "llvm-ar"
                | "llvm-lib"
                | "llvm-readobj"
                | "lld"
                | "lld-link"
                | "lldb"
                | "lldb-server"
        )
    });
    let needs_imobile = missing.iter().any(|t| t.starts_with("idevice"));

    if !needs_llvm && !needs_imobile {
        return;
    }

    println!("\nSystem Package Remediation Suggestions:");
    if cfg!(target_os = "linux") {
        if Path::new("/etc/NIXOS").exists() || Path::new("/nix/store").exists() {
            println!("  NixOS / Nix: Run `nix develop` in the repository root to enter the hermetic devShell.");
        } else if which::which("apt-get").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["clang", "llvm", "lld", "lldb"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice-utils", "ideviceinstaller"]);
            }
            println!("  Debian/Ubuntu: sudo apt-get install {}", pkgs.join(" "));
        } else if which::which("dnf").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["clang", "llvm", "lld", "lldb"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice-utils", "ideviceinstaller"]);
            }
            println!("  Fedora/RHEL:   sudo dnf install {}", pkgs.join(" "));
        } else if which::which("pacman").is_ok() {
            let mut pkgs = Vec::new();
            if needs_llvm {
                pkgs.extend(["clang", "llvm", "lld", "lldb"]);
            }
            if needs_imobile {
                pkgs.extend(["libimobiledevice", "ideviceinstaller"]);
            }
            println!("  Arch Linux:    sudo pacman -S {}", pkgs.join(" "));
        }
    } else if cfg!(target_os = "macos") {
        if needs_llvm {
            println!("  macOS (LLVM):  xcode-select --install   OR   brew install llvm lld");
        }
        if needs_imobile {
            println!("  macOS (iOS):   brew install libimobiledevice ideviceinstaller");
        }
    } else if cfg!(target_os = "windows") && needs_llvm {
        println!("  Windows:       winget install LLVM.LLVM");
    }
}

pub fn run_system_i_check(args: &IArgs) -> Result<()> {
    let search_dir = args
        .project
        .clone()
        .or_else(|| std::env::current_dir().ok());
    let detected_root = crate::commands::cache::detect_project_root(search_dir.as_deref());
    let project_cache = crate::commands::cache::discover_project_cache(search_dir.as_deref());
    let global_cache = crate::SessionExecutionContext::global_cache_dir();

    let parsed_manifest = search_dir.as_deref().and_then(|dir| {
        let m = if dir.is_file() {
            dir.to_path_buf()
        } else {
            dir.join("manifest.toml")
        };
        if m.is_file() {
            crate::config::ManifestConfig::parse_file(&m).ok()
        } else {
            let cargo_path = if dir.is_file() {
                dir.parent().unwrap_or(Path::new(".")).join("Cargo.toml")
            } else {
                dir.join("Cargo.toml")
            };
            crate::config::ManifestConfig::parse_cargo_metadata(&cargo_path)
                .ok()
                .flatten()
        }
    });

    let target_cache =
        determine_info_target_cache(args, detected_root.as_deref(), parsed_manifest.as_ref());

    if args.clean_cache || args.fix {
        println!("Auditing and cleaning crorust toolchain caches...");
        if target_cache != global_cache {
            let (count, bytes) = audit_and_clean_cache(&target_cache, "Target Cache")?;
            if count > 0 {
                println!(
                    "  Cleaned {} corrupted/incomplete item(s) ({}) from Target Cache.",
                    count,
                    crate::commands::cache::format_bytes(bytes)
                );
            }
        }
        if let Some(proj_cache) = &project_cache {
            if *proj_cache != target_cache {
                let (count, bytes) = audit_and_clean_cache(proj_cache, "Project Cache")?;
                if count > 0 {
                    println!(
                        "  Cleaned {} corrupted/incomplete item(s) ({}) from Project Cache.",
                        count,
                        crate::commands::cache::format_bytes(bytes)
                    );
                }
            }
        }
        let (count, bytes) = audit_and_clean_cache(&global_cache, "Global Cache")?;
        if count > 0 {
            println!(
                "  Cleaned {} corrupted/incomplete item(s) ({}) from Global Cache.",
                count,
                crate::commands::cache::format_bytes(bytes)
            );
        }
        println!();
    }

    if args.fix {
        println!("Running diagnostic self-healing (`crorust i --fix`)...");
        let host_target = TargetCompilationUnit::new(
            TargetPlatform::detect_host()?,
            TargetArch::detect_host()?,
            TargetOpt::Debug,
        );
        let host_triple = host_target.rust_target_triple()?;
        let mut target_triples = vec![host_triple];
        for &triple in UNIVERSAL_TARGET_TRIPLES {
            if !target_triples.contains(&triple) {
                target_triples.push(triple);
            }
        }
        let added = ensure_rustup_targets(&target_triples)?;
        if added.is_empty() {
            println!("  [i --fix] ✓ Universal Rust target triples are already installed.");
        } else {
            println!(
                "  [i --fix] ✓ Added Rust target triples: {}",
                added.join(", ")
            );
        }

        let added_components = ensure_rustup_components(&["llvm-tools-preview"])?;
        if added_components.is_empty() {
            println!("  [i --fix] ✓ Required Rustup components (`llvm-tools-preview`) already installed.");
        } else {
            println!(
                "  [i --fix] ✓ Added Rustup components: {}",
                added_components.join(", ")
            );
        }

        let mirror = parsed_manifest
            .as_ref()
            .and_then(|c| c.toolchain().mirror_url.clone());

        provision_managed_tools(&target_cache, host_triple, mirror, args.allow_partial)?;
        println!();
    }

    let inspector = SystemIEngine::default();
    print!("{inspector}");

    let mut cache_roots: Vec<PathBuf> = Vec::new();
    if !cache_roots.contains(&target_cache) {
        cache_roots.push(target_cache.clone());
    }
    if let Some(proj) = &project_cache {
        if !cache_roots.contains(proj) {
            cache_roots.push(proj.clone());
        }
    }
    if !cache_roots.contains(&global_cache) {
        cache_roots.push(global_cache.clone());
    }

    let managed = inspector.check_managed_tools(&cache_roots);
    if !managed.is_empty() {
        println!("\n-----------------------managed tools------------------------");
        for tool in managed {
            let status = match (&tool.managed_version, &tool.system_version) {
                (Some(managed), _) => managed.clone(),
                (None, Some(system)) => format!("Not cached ({system})"),
                (None, None) => "Not installed (run `crorust i --fix` to provision)".into(),
            };
            println!("{:<28} {}", tool.name, status);
        }
    }

    println!("\nCache Diagnostics:");
    println!(
        "  Active Cache Directory:  {} ({})",
        target_cache.display(),
        crate::commands::cache::format_bytes(crate::commands::cache::compute_dir_size(
            &target_cache
        ))
    );
    if let Some(proj_cache) = &project_cache {
        if *proj_cache != target_cache {
            let proj_size = crate::commands::cache::compute_dir_size(proj_cache);
            println!(
                "  Project Cache Directory: {} ({})",
                proj_cache.display(),
                crate::commands::cache::format_bytes(proj_size)
            );
        }
    }

    let global_size = crate::commands::cache::compute_dir_size(&global_cache);
    if global_cache != target_cache {
        println!(
            "  Global Cache Directory:  {} ({})",
            global_cache.display(),
            crate::commands::cache::format_bytes(global_size)
        );
        if global_size > 0 {
            println!("  Recommendation:          Run 'crorust cache clean --global' if not sharing SDKs across projects.");
        }
    }

    let missing_sys = inspector.missing_required_tools();
    if args.fix && args.system {
        execute_system_package_installation(&missing_sys)?;
    } else {
        print_system_remediation_hints(&missing_sys);
    }

    if args.offline {
        let host_platform = TargetPlatform::detect_host()?;
        let host_arch = TargetArch::detect_host()?;
        let min_sdk = parsed_manifest
            .as_ref()
            .and_then(|c| c.android_platform_config().min_sdk);
        let emit_sideload = args.emit_sideload_manifest || args.sideload_out.is_some();
        let ready = inspector.audit_offline_readiness(
            &cache_roots,
            host_platform,
            host_arch,
            min_sdk,
            emit_sideload,
            args.sideload_out.as_deref(),
        )?;
        if !ready {
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_audit_and_clean_cache_removes_corrupted_archive_and_incomplete_dir() {
        let temp = TempDir::new().unwrap();
        let cache_root = temp.path();

        let download_dir = cache_root.join("download");
        std::fs::create_dir_all(&download_dir).unwrap();
        let corrupted_zip = download_dir.join("gradle-8.7-bin.zip");
        std::fs::write(&corrupted_zip, b"corrupted-zip-payload").unwrap();

        let incomplete_tool = cache_root
            .join("tools")
            .join("linux-x64")
            .join("gradle")
            .join("8.7");
        std::fs::create_dir_all(&incomplete_tool).unwrap();
        std::fs::write(incomplete_tool.join("partial.jar"), b"partial").unwrap();

        let valid_tool = cache_root
            .join("tools")
            .join("linux-x64")
            .join("kotlin")
            .join("1.9.22");
        std::fs::create_dir_all(&valid_tool).unwrap();
        std::fs::write(valid_tool.join(".complete"), b"ok").unwrap();

        let (removed, freed) = audit_and_clean_cache(cache_root, "Test Cache").unwrap();
        assert!(removed >= 2);
        assert!(freed > 0);
        assert!(
            !corrupted_zip.exists(),
            "Corrupted archive should be removed"
        );
        assert!(
            !incomplete_tool.exists(),
            "Incomplete extraction missing .complete should be removed"
        );
        assert!(
            valid_tool.exists(),
            "Valid extraction with .complete must be preserved"
        );
    }

    #[test]
    fn test_check_managed_tools_detects_project_and_global_caches() {
        let proj_tmp = TempDir::new().unwrap();
        let global_tmp = TempDir::new().unwrap();

        let kotlin_complete = proj_tmp
            .path()
            .join("tools")
            .join("linux-x64")
            .join("kotlin")
            .join("1.9.22");
        std::fs::create_dir_all(&kotlin_complete).unwrap();
        std::fs::write(kotlin_complete.join(".complete"), b"ok").unwrap();

        let d8_complete = global_tmp
            .path()
            .join("tools")
            .join("linux-x64")
            .join("d8")
            .join("34.0.0");
        std::fs::create_dir_all(&d8_complete).unwrap();
        std::fs::write(d8_complete.join(".complete"), b"ok").unwrap();

        let doc = SystemIEngine::default();
        let statuses = doc.check_managed_tools(&[
            proj_tmp.path().to_path_buf(),
            global_tmp.path().to_path_buf(),
        ]);

        let kotlin_status = statuses.iter().find(|s| s.name.contains("Kotlin")).unwrap();
        assert!(kotlin_status.managed_version.is_some());

        let d8_status = statuses.iter().find(|s| s.name.contains("D8")).unwrap();
        assert!(d8_status.managed_version.is_some());
    }

    #[test]
    fn test_determine_info_target_cache_defaults_to_dot_cache() {
        let _guard = crate::core::env::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev_cache = std::env::var("CRORUST_CACHE_DIR").ok();
        std::env::remove_var("CRORUST_CACHE_DIR");

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();

        let args = IArgs::default();
        let cache = determine_info_target_cache(&args, Some(project_root), None);

        if let Some(val) = prev_cache {
            std::env::set_var("CRORUST_CACHE_DIR", val);
        }

        assert_eq!(cache, project_root.join(".cache").join("crorust"));
    }

    #[test]
    fn test_determine_info_target_cache_respects_cli_overrides() {
        let _guard = crate::core::env::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let custom_dir = temp.path().join("custom_cache_location");

        // 1. Explicit --cache-dir
        let args_explicit = IArgs {
            cache_dir: Some(custom_dir.clone()),
            ..Default::default()
        };
        let cache = determine_info_target_cache(&args_explicit, Some(project_root), None);
        assert_eq!(cache, custom_dir);

        // 2. Explicit --global-cache
        let args_global = IArgs {
            global_cache: true,
            ..Default::default()
        };
        let cache_global = determine_info_target_cache(&args_global, Some(project_root), None);
        assert_eq!(
            cache_global,
            crate::SessionExecutionContext::global_cache_dir()
        );

        // 3. Explicit --project-cache
        let args_project = IArgs {
            project_cache: true,
            ..Default::default()
        };
        let cache_proj = determine_info_target_cache(&args_project, Some(project_root), None);
        assert_eq!(cache_proj, project_root.join(".cache").join("crorust"));
    }

    #[test]
    fn test_determine_info_target_cache_respects_manifest_configuration() {
        let _guard = crate::core::env::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let prev_cache = std::env::var("CRORUST_CACHE_DIR").ok();
        std::env::remove_var("CRORUST_CACHE_DIR");

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();

        // Manifest with cache_scope = "global"
        let manifest_global: crate::config::ManifestConfig = toml::from_str(
            r#"
            name = "test"
            version = "1.0.0"
            [toolchain]
            cache_scope = "global"
            "#,
        )
        .unwrap();

        let args = IArgs::default();
        let cache = determine_info_target_cache(&args, Some(project_root), Some(&manifest_global));
        assert_eq!(cache, crate::SessionExecutionContext::global_cache_dir());

        // Manifest with explicit cache_dir
        let manifest_dir: crate::config::ManifestConfig = toml::from_str(
            r#"
            name = "test"
            version = "1.0.0"
            [toolchain]
            cache_dir = "my_custom_cache"
            "#,
        )
        .unwrap();

        let cache_dir = determine_info_target_cache(&args, Some(project_root), Some(&manifest_dir));

        if let Some(val) = prev_cache {
            std::env::set_var("CRORUST_CACHE_DIR", val);
        }

        assert_eq!(cache_dir, project_root.join("my_custom_cache"));
    }

    fn failed(names: &[&str]) -> ProvisioningReport {
        ProvisioningReport {
            failed: names
                .iter()
                .map(|n| (n.to_string(), "HTTP 404".to_string()))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn report_with_required_failure_is_an_error() {
        // Verify that a report containing missing required tools returns an error
        let err = evaluate_report(&failed(&["Android NDK", "iOS Simulator SDK"]), false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Android NDK"), "{err}");
        assert!(
            !err.contains("Simulator"),
            "optional tool must not be listed: {err}"
        );
    }

    #[test]
    fn report_with_only_optional_failure_is_ok() {
        assert!(evaluate_report(&failed(&["iOS Simulator SDK"]), false).is_ok());
    }

    #[test]
    fn allow_partial_downgrades_required_failure() {
        assert!(evaluate_report(&failed(&["Android NDK"]), true).is_ok());
    }

    #[test]
    fn artifact_without_url_is_skipped_without_network() {
        let temp = TempDir::new().unwrap();
        let mgr = ToolManager::with_cache_dir(
            temp.path().to_path_buf(),
            "x86_64-unknown-linux-gnu",
            None,
            true, // offline: any download attempt would error, not skip
        )
        .unwrap();
        let artifact = ToolCatalog::get_sysroot("iPhoneSimulator.sdk").unwrap();
        assert!(artifact.url.is_empty());

        let mut report = ProvisioningReport::default();
        provision_artifact_with_verification(&mgr, &artifact, "iOS Simulator SDK", &mut report);

        assert_eq!(report.skipped.len(), 1);
        assert!(report.failed.is_empty());
        assert!(report.succeeded.is_empty());
    }
}
