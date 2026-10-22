use anyhow::{bail, ensure, Context, Result};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

mod artifact;
pub mod config;
pub mod manifest;
pub mod synthesizer;
pub mod tool_resolver;
mod utils;

pub use artifact::{CompilationArtifact, CompilationCrateKind};
pub use config::{CargoBuildConfig, CargoEnvVarSpec, CargoTomlConfig, CargoWorkspaceConfig};
pub use manifest::{
    CargoBinTarget, CargoLibTarget, CargoPackageManifest, CargoPackageSection,
    CargoWorkspacePackageMetadata, CargoWorkspaceSection, WorkspaceInheritable,
};
pub use synthesizer::CargoEnvironmentSynthesizer;

use crate::{task, TargetArch, TargetCompilationUnit, TargetOpt, TargetPlatform};

#[derive(Debug)]
pub struct CargoWorkspaceSession {
    selected_package_name: String,
    selected_binary_target: Option<String>,
    active_cargo_features: Vec<String>,
    disable_default_features: bool,
    workspace_manifest_cache: Option<CargoPackageManifest>,
    workspace_root_dir: Option<PathBuf>,
    package_manifest_cache: CargoPackageManifest,
    package_source_root: PathBuf,
    build_target_directory: PathBuf,
    offline_mode_enabled: bool,
}

impl CargoWorkspaceSession {
    pub fn new(
        requested_package: Option<&str>,
        requested_bin: Option<&str>,
        enabled_features: Vec<String>,
        disable_default_features: bool,
        explicit_manifest_path: Option<PathBuf>,
        explicit_target_dir: Option<PathBuf>,
        offline_mode: bool,
    ) -> Result<Self> {
        let validated_manifest_path = explicit_manifest_path
            .map(|raw_path| {
                if raw_path.file_name() != Some(OsStr::new("Cargo.toml")) || !raw_path.is_file() {
                    bail!(
                        "The manifest-path argument must point directly to a valid Cargo.toml file"
                    );
                }
                Ok(raw_path)
            })
            .transpose()?;

        let discovery_dir = match validated_manifest_path {
            Some(ref explicit_path) => utils::canonicalize_safe_path(
                explicit_path
                    .parent()
                    .context("Manifest path has no parent folder")?,
            )?,
            None => {
                std::env::current_dir().context("Unable to access current working directory")?
            }
        };

        let nearest_manifest_info = utils::locate_nearest_manifest(&discovery_dir)?;
        let mut workspace_manifest_info = utils::locate_nearest_workspace_root(&discovery_dir)?;

        if let Some((ref ws_file, ref ws_dom)) = workspace_manifest_info {
            let ws_folder = ws_file.parent().unwrap();
            let pkg_folder = nearest_manifest_info.0.parent().unwrap();
            if ws_dom.is_excluded(ws_folder, pkg_folder)? {
                workspace_manifest_info = None;
            }
        }

        let detected_workspace_root = workspace_manifest_info
            .as_ref()
            .map(|(p, _)| p.parent().unwrap().to_path_buf());

        let mut manifest_package_override = None;
        let mut manifest_bin_override = None;

        if requested_package.is_none() || requested_bin.is_none() {
            let search_manifest_toml = discovery_dir.join("manifest.toml");
            let ws_manifest_toml = detected_workspace_root
                .as_ref()
                .map(|root| root.join("manifest.toml"));

            let parsed_config = if search_manifest_toml.is_file() {
                crate::config::ManifestConfig::parse_file(&search_manifest_toml).ok()
            } else if let Some(ws_path) = ws_manifest_toml.filter(|p| p.is_file()) {
                crate::config::ManifestConfig::parse_file(&ws_path).ok()
            } else {
                let pkg_cargo = discovery_dir.join("Cargo.toml");
                crate::config::ManifestConfig::parse_cargo_metadata(&pkg_cargo)
                    .ok()
                    .flatten()
            };

            if let Some(cfg) = parsed_config {
                manifest_package_override = cfg.package_id().map(|s| s.to_string());
                manifest_bin_override = cfg.bin().map(|s| s.to_string());
            }
        }

        let effective_package_name = requested_package
            .map(|s| s.to_string())
            .or(manifest_package_override);

        let effective_binary_name = requested_bin
            .map(|s| s.to_string())
            .or(manifest_bin_override);

        let (chosen_manifest_path, parsed_manifest) = match workspace_manifest_info {
            Some(ref ws_info) => utils::locate_package_manifest_in_workspace(
                ws_info,
                nearest_manifest_info,
                effective_package_name.as_deref(),
            )?,
            None => {
                let (path, dom) = nearest_manifest_info;
                dom.resolve_nonvirtual_package(path, effective_package_name.as_deref())?
            }
        };

        let resolved_package_name = parsed_manifest
            .package
            .as_ref()
            .context("Resolved manifest contains no [package] definition")?
            .name
            .clone();

        let package_root_dir = chosen_manifest_path
            .parent()
            .context("Manifest path has no parent")?;

        let resolved_binary_target = if let Some(ref req_bin) = effective_binary_name {
            let available_bins =
                utils::discover_available_binaries(package_root_dir, &parsed_manifest)?;
            if !available_bins.iter().any(|b| b == req_bin) {
                bail!(
                    "binary `{}` not found in package `{}`. Available binaries: {}",
                    req_bin,
                    resolved_package_name,
                    if available_bins.is_empty() {
                        "none found".to_string()
                    } else {
                        available_bins.join(", ")
                    }
                );
            }
            Some(req_bin.clone())
        } else {
            parsed_manifest
                .primary_bin_name()
                .map(|s| s.to_string())
                .or_else(|| {
                    if package_root_dir.join("src").join("main.rs").is_file() {
                        Some(resolved_package_name.clone())
                    } else {
                        None
                    }
                })
        };

        let cargo_cfg = CargoWorkspaceConfig::locate_workspace_cargo_config(package_root_dir)?;
        if let Some(ref c) = cargo_cfg {
            c.apply_cargo_environment_overrides()?;
        }

        let resolved_target_dir = explicit_target_dir
            .or_else(|| {
                let env_var = std::env::var_os("CARGO_BUILD_TARGET_DIR");
                env_var
                    .or_else(|| std::env::var_os("CARGO_TARGET_DIR"))
                    .map(PathBuf::from)
            })
            .map(|td| {
                if td.is_relative() {
                    std::env::current_dir().unwrap().join(td)
                } else {
                    td
                }
            })
            .or_else(|| cargo_cfg.as_ref().and_then(|c| c.resolved_target_dir()))
            .unwrap_or_else(|| {
                workspace_manifest_info
                    .as_ref()
                    .map(|(p, _)| p)
                    .unwrap_or(&chosen_manifest_path)
                    .parent()
                    .unwrap()
                    .join(
                        utils::resolve_target_dir_name(cargo_cfg.as_ref().map(|c| &c.config))
                            .unwrap(),
                    )
            });

        Ok(Self {
            selected_package_name: resolved_package_name,
            selected_binary_target: resolved_binary_target,
            active_cargo_features: enabled_features,
            disable_default_features,
            workspace_manifest_cache: workspace_manifest_info.map(|(_, m)| m),
            workspace_root_dir: detected_workspace_root,
            package_manifest_cache: parsed_manifest,
            package_source_root: package_root_dir.to_path_buf(),
            build_target_directory: resolved_target_dir,
            offline_mode_enabled: offline_mode,
        })
    }

    #[inline]
    pub fn target_directory(&self) -> &Path {
        &self.build_target_directory
    }

    #[inline]
    pub fn package_identifier(&self) -> &str {
        &self.selected_package_name
    }

    #[inline]
    pub fn selected_bin(&self) -> Option<&str> {
        self.selected_binary_target.as_deref()
    }

    pub fn host_triple(&self) -> &'static str {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return "x86_64-unknown-linux-gnu";
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        return "aarch64-unknown-linux-gnu";
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return "x86_64-apple-darwin";
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return "aarch64-apple-darwin";
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        return "x86_64-pc-windows-msvc";
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        return "aarch64-pc-windows-msvc";
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(
                target_os = "macos",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(
                target_os = "windows",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )
        )))]
        return "unknown-host-triple";
    }

    #[inline]
    pub fn workspace_root_path(&self) -> Option<&Path> {
        self.workspace_root_dir.as_deref()
    }

    #[inline]
    pub fn workspace_manifest_data(&self) -> Option<&CargoPackageManifest> {
        self.workspace_manifest_cache.as_ref()
    }

    #[inline]
    pub fn package_manifest_data(&self) -> &CargoPackageManifest {
        &self.package_manifest_cache
    }

    #[inline]
    pub fn package_root_path(&self) -> &Path {
        &self.package_source_root
    }

    #[inline]
    pub fn cargo_manifest_file_path(&self) -> PathBuf {
        self.package_source_root.join("Cargo.toml")
    }

    pub fn discover_example_artifacts(&self) -> Result<Vec<CompilationArtifact>> {
        let mut list = Vec::new();
        for file_path in
            utils::discover_rust_source_stems(&self.package_root_path().join("examples"))?
        {
            list.push(CompilationArtifact::ExampleTarget(file_path));
        }
        Ok(list)
    }

    pub fn discover_binary_artifacts(&self) -> Result<Vec<CompilationArtifact>> {
        let mut list = Vec::new();
        for file_path in
            utils::discover_rust_source_stems(&self.package_root_path().join("src").join("bin"))?
        {
            list.push(CompilationArtifact::PackageRoot(file_path));
        }
        Ok(list)
    }

    pub fn create_compilation_invocation(
        &self,
        target_spec: TargetCompilationUnit,
        destination_dir: &Path,
    ) -> Result<CargoCompilationEnvironment> {
        CargoCompilationEnvironment::new(
            target_spec,
            &self.active_cargo_features,
            self.disable_default_features,
            self.package_root_path(),
            destination_dir,
            self.offline_mode_enabled,
            Some(&self.selected_package_name),
            self.selected_binary_target.as_deref(),
        )
    }

    pub fn resolve_compiled_artifact_path(
        &self,
        base_target_dir: &Path,
        compile_target: TargetCompilationUnit,
        specified_artifact: Option<CompilationArtifact>,
        crate_kind: CompilationCrateKind,
    ) -> Result<PathBuf> {
        let architecture_folder = if compile_target.targets_current_host()? {
            base_target_dir.to_path_buf()
        } else {
            base_target_dir.join(compile_target.rust_target_triple()?)
        };

        let profile_folder =
            architecture_folder.join(compile_target.optimization_profile().to_string());
        let selected_artifact = specified_artifact.unwrap_or_else(|| {
            if crate_kind == CompilationCrateKind::Bin {
                if let Some(ref binary_name) = self.selected_binary_target {
                    return CompilationArtifact::PackageRoot(binary_name.clone());
                }
                if let Some(target) = self.package_manifest_cache.bin.first() {
                    return CompilationArtifact::PackageRoot(target.name.clone());
                }
            }
            CompilationArtifact::PackageRoot(self.selected_package_name.clone())
        });

        let target_triple_name = compile_target.rust_target_triple()?;
        let output_binary_path = profile_folder
            .join(selected_artifact.as_ref())
            .join(selected_artifact.target_file_name(crate_kind, target_triple_name));

        ensure!(
            output_binary_path.exists(),
            "Compiled binary not found at expected location: {}",
            output_binary_path.display()
        );

        Ok(output_binary_path)
    }

    pub fn collect_build_dependency_search_paths(
        &self,
        base_target_dir: &Path,
        compile_target: TargetCompilationUnit,
    ) -> Result<Vec<PathBuf>> {
        let arch_dir = if compile_target.targets_current_host()? {
            base_target_dir.to_path_buf()
        } else {
            base_target_dir.join(compile_target.rust_target_triple()?)
        };

        let deps_dir = arch_dir
            .join(compile_target.optimization_profile().to_string())
            .join("build");

        let mut discovered_paths = Vec::new();

        if deps_dir.is_dir() {
            for entry_res in deps_dir.read_dir().with_context(|| {
                format!("Failed scanning dependencies in {}", deps_dir.display())
            })? {
                let output_file = entry_res?.path().join("output");
                if output_file.is_file() {
                    for line_res in BufReader::new(File::open(output_file)?).lines() {
                        let text = line_res?;
                        if let Some(search_suffix) = text.strip_prefix("cargo:rustc-link-search=") {
                            let (scope, directory) = search_suffix
                                .split_once('=')
                                .unwrap_or(("all", search_suffix));
                            match scope {
                                "dependency" | "native" | "all" => {
                                    discovered_paths.push(PathBuf::from(directory))
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(discovered_paths)
    }
}

pub struct CargoCompilationEnvironment {
    execution_command: Command,
    target: TargetCompilationUnit,
    target_triple: Option<&'static str>,
    c_compiler_flags: String,
    cxx_compiler_flags: String,
    rust_compiler_flags: String,
}

impl CargoCompilationEnvironment {
    #[allow(clippy::too_many_arguments)]
    fn new(
        target: TargetCompilationUnit,
        enabled_features: &[String],
        disable_default_features_flag: bool,
        workspace_dir: &Path,
        output_dir: &Path,
        offline_mode_flag: bool,
        package_filter: Option<&str>,
        bin_filter: Option<&str>,
    ) -> Result<Self> {
        let target_triple = if target.targets_current_host()? {
            None
        } else {
            Some(target.rust_target_triple()?)
        };

        let cargo_executable = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let mut cmd = Command::new(cargo_executable);
        cmd.current_dir(workspace_dir);

        cmd.args(["build", "--target-dir"]);
        cmd.arg(output_dir);

        if let Some(pkg) = package_filter {
            cmd.args(["-p", pkg]);
        }

        if target.target_platform() != TargetPlatform::Android {
            if let Some(bin_name) = bin_filter {
                cmd.args(["--bin", bin_name]);
            }
        }

        if target.optimization_profile() == TargetOpt::Release {
            cmd.args(["--release"]);
        }

        if let Some(triple) = target_triple {
            cmd.args(["--target", triple]);
        }

        if offline_mode_flag {
            cmd.args(["--offline"]);
        }

        if disable_default_features_flag {
            cmd.args(["--no-default-features"]);
        }

        for feat in enabled_features {
            cmd.args(["--features", feat]);
        }

        Ok(Self {
            execution_command: cmd,
            target,
            target_triple,
            c_compiler_flags: String::new(),
            cxx_compiler_flags: String::new(),
            rust_compiler_flags: String::new(),
        })
    }

    /// `extra_dirs` are searched for LLVM binaries when they are not on `PATH`
    /// (for example the Android NDK's `toolchains/llvm/prebuilt/*/bin`).
    pub fn configure_llvm_toolchain(
        &mut self,
        linker_name: Option<&str>,
        extra_dirs: &[PathBuf],
    ) -> Result<()> {
        let shim_dir = tool_resolver::default_shim_dir();
        let archiver = tool_resolver::resolve(&tool_resolver::LLVM_AR, extra_dirs, &shim_dir)?;
        let c_compiler = tool_resolver::resolve(&tool_resolver::CLANG, extra_dirs, &shim_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "clang".to_string());
        let cxx_compiler = tool_resolver::resolve(&tool_resolver::CLANG_CXX, extra_dirs, &shim_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "clang++".to_string());

        self.configure_tool_executable(CompilationTool::CCompiler, &c_compiler);
        self.configure_tool_executable(CompilationTool::CxxCompiler, &cxx_compiler);
        self.configure_tool_executable(CompilationTool::StaticArchiver, archiver);
        self.configure_tool_executable(CompilationTool::BinaryLinker, &c_compiler);
        if let Some(ld) = linker_name {
            self.configure_linker_flavor(ld);
        }
        Ok(())
    }

    pub fn configure_msvc_toolchain(&mut self) -> Result<()> {
        let shim_dir = tool_resolver::default_shim_dir();
        let cl = tool_resolver::resolve(&tool_resolver::CLANG_CL, &[], &shim_dir)?;
        let lib = tool_resolver::resolve(&tool_resolver::LLVM_LIB, &[], &shim_dir)?;
        self.configure_tool_executable(CompilationTool::CCompiler, &cl);
        self.configure_tool_executable(CompilationTool::CxxCompiler, &cl);
        self.configure_tool_executable(CompilationTool::StaticArchiver, lib);
        self.configure_tool_executable(CompilationTool::BinaryLinker, "rust-lld");
        Ok(())
    }

    pub fn attach_android_ndk_sysroot(
        &mut self,
        ndk_sysroot: &Path,
        minimum_sdk_level: u32,
    ) -> Result<()> {
        let canonical_sysroot = dunce::canonicalize(ndk_sysroot)?;
        let ndk_abi_triple = self.target.android_ndk_triple();
        assert_eq!(Some(ndk_abi_triple), self.target_triple);

        let lib_dir = canonical_sysroot
            .join("usr")
            .join("lib")
            .join(ndk_abi_triple);

        let mut available_sdks = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                if let Ok(sdk_num) = entry.file_name().to_string_lossy().parse::<u32>() {
                    available_sdks.push(sdk_num);
                }
            }
        }

        let chosen_sdk_level = available_sdks
            .iter()
            .copied()
            .filter(|&level| level <= minimum_sdk_level)
            .max()
            .or_else(|| available_sdks.iter().copied().min())
            .with_context(|| {
                format!(
                    "Android NDK directory `{}` contains no API level libraries",
                    lib_dir.display()
                )
            })?;

        let versioned_ndk_target = format!("{ndk_abi_triple}{chosen_sdk_level}");
        // The NDK ships its own llvm-ar next to clang: <ndk>/toolchains/llvm/prebuilt/<host>/bin
        let ndk_bin: Vec<PathBuf> = canonical_sysroot
            .parent()
            .map(|p| vec![p.join("bin")])
            .unwrap_or_default();
        self.configure_llvm_toolchain(Some("lld"), &ndk_bin)?;
        self.configure_sysroot_path(&canonical_sysroot);

        self.append_c_compiler_flag(&format!("--target={versioned_ndk_target}"));
        self.append_cxx_compiler_flag("-stdlib=libc++");

        let api_lib_dir = lib_dir.join(chosen_sdk_level.to_string());
        let clean_api_lib = crorust_common::sanitize_path(&api_lib_dir);
        let clean_lib = crorust_common::sanitize_path(&lib_dir);

        self.append_linker_arg(&format!("--target={versioned_ndk_target}"));
        self.append_linker_arg(&format!("-B{clean_api_lib}"));
        self.append_linker_arg(&format!("-L{clean_api_lib}"));
        self.append_linker_arg(&format!("-L{clean_lib}"));

        if let Some(runtime_lib_dir) =
            find_ndk_clang_lib_dir(&canonical_sysroot, self.target.target_arch())
        {
            let clean_runtime = crorust_common::sanitize_path(&runtime_lib_dir);
            self.append_linker_arg(&format!("-L{clean_runtime}"));
        }

        Ok(())
    }

    pub fn attach_windows_sdk_sysroot(
        &mut self,
        sdk_root: &Path,
        target: TargetCompilationUnit,
    ) -> Result<()> {
        let canonical_root = dunce::canonicalize(sdk_root)?;
        let arch_dir_name = match target.target_arch() {
            TargetArch::X64 => "x86_64",
            TargetArch::Arm64 => "arm64",
        };

        self.configure_msvc_toolchain()?;

        // Suppress MSVC STL1000 static assertion ("Unexpected compiler version, expected Clang 19.0.0 or newer")
        // when compiling C++ sources (e.g. libmimalloc-sys) with Clang 18 on Linux/macOS cross-compilation hosts.
        self.define_macro_constant("_ALLOW_COMPILER_AND_STL_VERSION_MISMATCH", "1");

        let resolve_case_insensitive = |relative: &str| -> PathBuf {
            crorust_common::case_fold_lookup(&canonical_root, Path::new(relative))
                .unwrap_or_else(|| canonical_root.join(relative))
        };

        let include_crt = resolve_case_insensitive("crt/include");
        let include_um = resolve_case_insensitive("sdk/include/um");
        let include_ucrt = resolve_case_insensitive("sdk/include/ucrt");
        let include_shared = resolve_case_insensitive("sdk/include/shared");

        self.append_msvc_include_dir(&include_crt);
        self.append_msvc_include_dir(&include_um);
        self.append_msvc_include_dir(&include_ucrt);
        self.append_msvc_include_dir(&include_shared);

        let lib_crt = resolve_case_insensitive(&format!("crt/lib/{arch_dir_name}"));
        let lib_um = resolve_case_insensitive(&format!("sdk/lib/um/{arch_dir_name}"));
        let lib_ucrt = resolve_case_insensitive(&format!("sdk/lib/ucrt/{arch_dir_name}"));

        self.append_library_search_dir(&lib_crt);
        self.append_library_search_dir(&lib_um);
        self.append_library_search_dir(&lib_ucrt);

        #[cfg(not(target_os = "windows"))]
        {
            let overlay_yaml = canonical_root.join(".crorust_clang_vfs.yaml");
            let include_tree = vec![include_crt, include_um, include_ucrt, include_shared];
            if crorust_common::generate_clang_vfs_overlay(&overlay_yaml, &include_tree).is_ok() {
                let overlay_arg = crorust_common::sanitize_path(&overlay_yaml);
                // Note: c_compiler_flags is automatically prepended to CXXFLAGS in execute_compilation_task,
                // so appending here applies the overlay cleanly to both C and C++ without duplicate flags.
                self.append_c_compiler_flag("-vfsoverlay");
                self.append_c_compiler_flag(&overlay_arg);
            }
        }

        Ok(())
    }

    pub fn attach_macos_sdk_sysroot(
        &mut self,
        sdk_root: &Path,
        target: TargetCompilationUnit,
        min_os_release: &str,
    ) -> Result<()> {
        let canonical_sdk = dunce::canonicalize(sdk_root)?;
        let triple_name = target.rust_target_triple()?;

        #[cfg(not(target_os = "macos"))]
        self.execution_command.env("SDKROOT", &canonical_sdk);

        let linker_flavor = if cfg!(target_os = "macos") {
            None
        } else {
            Some("lld")
        };
        self.configure_llvm_toolchain(linker_flavor, &[])?;
        self.configure_sysroot_path(&canonical_sdk);

        self.append_cxx_compiler_flag("-stdlib=libc++");
        self.append_c_compiler_flag(&format!("-mmacosx-version-min={min_os_release}"));
        self.append_c_compiler_flag(&format!("--target={triple_name}"));

        self.append_linker_arg(&format!("--target={triple_name}"));
        self.append_linker_arg(&format!("-mmacosx-version-min={min_os_release}"));
        self.append_linker_arg("-rpath");
        self.append_linker_arg("@executable_path/../Frameworks");

        let resolve_path = |subpath: &str| -> PathBuf {
            crorust_common::case_fold_lookup(&canonical_sdk, Path::new(subpath))
                .unwrap_or_else(|| canonical_sdk.join(subpath))
        };

        let lib_system = resolve_path("usr/lib");
        let lib_subsystem = resolve_path("usr/lib/system");
        let sys_frameworks = resolve_path("System/Library/Frameworks");
        let private_frameworks = resolve_path("System/Library/PrivateFrameworks");

        self.append_library_search_dir(&lib_system);
        self.append_library_search_dir(&lib_subsystem);
        self.append_framework_search_dir(&sys_frameworks);
        self.append_framework_search_dir(&private_frameworks);

        #[cfg(not(target_os = "macos"))]
        {
            let overlay_yaml = canonical_sdk.join(".crorust_clang_vfs.yaml");
            let inc_dirs = vec![resolve_path("usr/include"), sys_frameworks];
            if crorust_common::generate_clang_vfs_overlay(&overlay_yaml, &inc_dirs).is_ok() {
                let vfs_clean = crorust_common::sanitize_path(&overlay_yaml);
                self.append_c_compiler_flag("-ivfsoverlay");
                self.append_c_compiler_flag(&vfs_clean);
                self.append_cxx_compiler_flag("-ivfsoverlay");
                self.append_cxx_compiler_flag(&vfs_clean);
            }
        }

        Ok(())
    }

    pub fn attach_ios_sdk_sysroot(
        &mut self,
        sdk_root: &Path,
        target: TargetCompilationUnit,
        min_os_release: &str,
    ) -> Result<()> {
        let canonical_sdk = dunce::canonicalize(sdk_root)?;
        let triple_name = target.rust_target_triple()?;

        #[cfg(not(target_os = "macos"))]
        self.execution_command.env("SDKROOT", &canonical_sdk);

        let linker_flavor = if cfg!(target_os = "macos") {
            None
        } else {
            Some("lld")
        };
        self.configure_llvm_toolchain(linker_flavor, &[])?;
        self.configure_sysroot_path(&canonical_sdk);
        self.append_cxx_compiler_flag("-stdlib=libc++");

        let version_flag = target
            .apple_version_min_flag(min_os_release)
            .unwrap_or_else(|| format!("-miphoneos-version-min={min_os_release}"));

        self.append_c_compiler_flag(&version_flag);
        self.append_c_compiler_flag(&format!("--target={triple_name}"));

        self.append_linker_arg(&format!("--target={triple_name}"));
        self.append_linker_arg(&version_flag);
        self.append_linker_arg("-rpath");
        self.append_linker_arg("@executable_path/Frameworks");

        let resolve_path = |subpath: &str| -> PathBuf {
            crorust_common::case_fold_lookup(&canonical_sdk, Path::new(subpath))
                .unwrap_or_else(|| canonical_sdk.join(subpath))
        };

        let sys_frameworks = resolve_path("System/Library/Frameworks");
        self.append_library_search_dir(&resolve_path("usr/lib"));
        self.append_framework_search_dir(&sys_frameworks);

        #[cfg(not(target_os = "macos"))]
        {
            let overlay_yaml = canonical_sdk.join(".crorust_clang_vfs.yaml");
            let inc_dirs = vec![resolve_path("usr/include"), sys_frameworks];
            if crorust_common::generate_clang_vfs_overlay(&overlay_yaml, &inc_dirs).is_ok() {
                let vfs_clean = crorust_common::sanitize_path(&overlay_yaml);
                self.append_c_compiler_flag("-ivfsoverlay");
                self.append_c_compiler_flag(&vfs_clean);
                self.append_cxx_compiler_flag("-ivfsoverlay");
                self.append_cxx_compiler_flag(&vfs_clean);
            }
        }

        Ok(())
    }

    pub fn attach_linux_sysroot(
        &mut self,
        sysroot_path: &Path,
        target: TargetCompilationUnit,
    ) -> Result<()> {
        let canonical_sysroot = dunce::canonicalize(sysroot_path)?;
        let triple_name = target.rust_target_triple()?;

        self.configure_llvm_toolchain(Some("lld"), &[])?;
        self.configure_sysroot_path(&canonical_sysroot);

        self.append_c_compiler_flag(&format!("--target={triple_name}"));
        self.append_cxx_compiler_flag("-stdlib=libc++");

        self.append_linker_arg(&format!("--target={triple_name}"));
        self.append_linker_arg("-Wl,-rpath,$ORIGIN/lib");
        self.append_linker_arg("-Wl,-z,origin");

        let usr_lib = canonical_sysroot.join("usr").join("lib");
        let base_lib = canonical_sysroot.join("lib");
        self.append_library_search_dir(&usr_lib);
        self.append_library_search_dir(&base_lib);

        let multiarch_triplet = match target.target_arch() {
            TargetArch::X64 => "x86_64-linux-gnu",
            TargetArch::Arm64 => "aarch64-linux-gnu",
        };
        let multiarch_usr_lib = usr_lib.join(multiarch_triplet);
        let multiarch_base_lib = base_lib.join(multiarch_triplet);
        if multiarch_usr_lib.is_dir() {
            self.append_library_search_dir(&multiarch_usr_lib);
        }
        if multiarch_base_lib.is_dir() {
            self.append_library_search_dir(&multiarch_base_lib);
        }

        Ok(())
    }

    pub fn configure_tool_executable<P: AsRef<Path>>(
        &mut self,
        tool: CompilationTool,
        binary_path: P,
    ) {
        let path_str = binary_path
            .as_ref()
            .to_str()
            .expect("Valid UTF-8 tool path");
        match tool {
            CompilationTool::CCompiler
            | CompilationTool::CxxCompiler
            | CompilationTool::StaticArchiver => {
                self.export_triple_environment(&tool.to_string(), path_str);
            }
            CompilationTool::BinaryLinker => {
                self.export_target_environment("LINKER", path_str);
            }
        }
    }

    pub fn export_target_environment(&mut self, variable_suffix: &str, value: &str) {
        if let Some(triple) = self.target_triple {
            let normalized_triple = triple.replace('-', "_");
            let key = format!(
                "CARGO_TARGET_{}_{}",
                normalized_triple.to_uppercase(),
                variable_suffix
            );
            self.execution_command.env(key, value);
        } else {
            self.execution_command.env(variable_suffix, value);
        }
    }

    pub fn export_bindgen_clang_environment(&mut self, args_payload: &str) {
        if let Some(triple) = self.target_triple {
            let normalized_triple = triple.replace('-', "_");
            let key = format!("BINDGEN_EXTRA_CLANG_ARGS_{normalized_triple}");
            self.execution_command.env(key, args_payload);
        } else {
            self.execution_command
                .env("BINDGEN_EXTRA_CLANG_ARGS", args_payload);
        }
    }

    pub fn export_triple_environment(&mut self, prefix: &str, value: &str) {
        if let Some(triple) = self.target_triple {
            self.execution_command
                .env(format!("{prefix}_{triple}"), value);
        } else {
            self.execution_command.env(prefix, value);
        }
    }

    pub fn append_library_search_dir(&mut self, dir_path: &Path) {
        let clean = crate::sanitize_path(dir_path);
        self.rust_compiler_flags
            .push_str(&format!("-Lnative={clean} "));
    }

    pub fn append_framework_search_dir(&mut self, framework_path: &Path) {
        let clean = crate::sanitize_path(framework_path);
        self.rust_compiler_flags
            .push_str(&format!("-Lframework={clean} "));
    }

    pub fn link_named_library(&mut self, library_name: &str) {
        self.rust_compiler_flags
            .push_str(&format!("-l{library_name} "));
    }

    pub fn link_named_framework(&mut self, framework_name: &str) {
        self.rust_compiler_flags
            .push_str(&format!("-lframework={framework_name} "));
    }

    pub fn append_target_feature(&mut self, feature: &str) {
        self.rust_compiler_flags
            .push_str(&format!("-Ctarget-feature={feature} "));
    }

    pub fn append_linker_arg(&mut self, linker_argument: &str) {
        self.rust_compiler_flags
            .push_str(&format!("-Clink-arg={linker_argument} "));
    }

    pub fn define_macro_constant(&mut self, macro_name: &str, macro_val: &str) {
        self.c_compiler_flags
            .push_str(&format!("-D{macro_name}={macro_val} "));
    }

    pub fn append_c_include_dir(&mut self, inc_path: &Path) {
        let clean = crate::sanitize_path(inc_path);
        self.c_compiler_flags.push_str(&format!("-I{clean} "));
    }

    pub fn append_msvc_include_dir(&mut self, inc_path: &Path) {
        let clean = crate::sanitize_path(inc_path);
        self.c_compiler_flags.push_str(&format!("-imsvc{clean} "));
    }

    pub fn configure_sysroot_path(&mut self, sysroot_path: &Path) {
        let clean = crate::sanitize_path(sysroot_path);
        let flag = format!("--sysroot={clean}");
        self.append_c_compiler_flag(&flag);
        self.append_linker_arg(&flag);
    }

    pub fn append_c_compiler_flag(&mut self, compiler_flag: &str) {
        self.c_compiler_flags.push_str(compiler_flag);
        self.c_compiler_flags.push(' ');
    }

    pub fn append_cxx_compiler_flag(&mut self, compiler_flag: &str) {
        self.cxx_compiler_flags.push_str(compiler_flag);
        self.cxx_compiler_flags.push(' ');
    }

    pub fn configure_linker_flavor(&mut self, linker_name: &str) {
        self.append_linker_arg(&format!("-fuse-ld={linker_name}"));
    }

    pub fn append_cargo_argument(&mut self, cli_argument: &str) {
        self.execution_command.arg(cli_argument);
    }

    pub fn execute_compilation_task(mut self) -> Result<()> {
        self.export_target_environment("RUSTFLAGS", &self.rust_compiler_flags.clone());
        self.export_bindgen_clang_environment(&self.c_compiler_flags.clone());
        self.export_triple_environment("CFLAGS", &self.c_compiler_flags.clone());
        self.export_triple_environment(
            "CXXFLAGS",
            &format!("{}{}", self.c_compiler_flags, self.cxx_compiler_flags),
        );
        task::run(&mut self.execution_command)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn c_compiler_flags_for_test(&self) -> &str {
        &self.c_compiler_flags
    }

    #[cfg(test)]
    pub fn cxx_compiler_flags_for_test(&self) -> &str {
        &self.cxx_compiler_flags
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilationTool {
    CCompiler,
    CxxCompiler,
    BinaryLinker,
    StaticArchiver,
}

impl std::fmt::Display for CompilationTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CCompiler => f.write_str("CC"),
            Self::CxxCompiler => f.write_str("CXX"),
            Self::BinaryLinker => f.write_str("LINKER"),
            Self::StaticArchiver => f.write_str("AR"),
        }
    }
}

pub fn find_ndk_clang_lib_dir(ndk_sysroot: &Path, arch: TargetArch) -> Option<PathBuf> {
    let toolchain_parent = ndk_sysroot.parent()?;
    let abi_subfolder = match arch {
        TargetArch::Arm64 => "aarch64",
        TargetArch::X64 => "x86_64",
    };

    for candidate_lib in &["lib", "lib64"] {
        let base_clang = toolchain_parent.join(candidate_lib).join("clang");
        if base_clang.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&base_clang) {
                for node in entries.flatten() {
                    let runtime_path = node.path().join("lib").join("linux").join(abi_subfolder);
                    if runtime_path.is_dir() {
                        return Some(runtime_path);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::target::{TargetArch, TargetCompilationUnit, TargetOpt, TargetPlatform};

    #[test]
    fn test_windows_sdk_sysroot_injects_stl_version_mismatch_guard() {
        let temp = tempfile::tempdir().unwrap();
        let fake_cl = temp.path().join("fake-clang-cl");
        std::fs::write(&fake_cl, "").unwrap();
        let fake_lib = temp.path().join("fake-llvm-lib");
        std::fs::write(&fake_lib, "").unwrap();

        let prev_cl = std::env::var_os("CRORUST_CLANG_CL");
        let prev_lib = std::env::var_os("CRORUST_LLVM_LIB");
        std::env::set_var("CRORUST_CLANG_CL", &fake_cl);
        std::env::set_var("CRORUST_LLVM_LIB", &fake_lib);

        let sdk_root = temp.path().join("Windows.sdk").join("10.0.22621.0");
        std::fs::create_dir_all(sdk_root.join("crt").join("include")).unwrap();
        std::fs::create_dir_all(sdk_root.join("sdk").join("include").join("um")).unwrap();
        std::fs::create_dir_all(sdk_root.join("sdk").join("include").join("ucrt")).unwrap();
        std::fs::create_dir_all(sdk_root.join("sdk").join("include").join("shared")).unwrap();
        std::fs::create_dir_all(sdk_root.join("crt").join("lib").join("x86_64")).unwrap();
        std::fs::create_dir_all(sdk_root.join("sdk").join("lib").join("um").join("x86_64"))
            .unwrap();
        std::fs::create_dir_all(sdk_root.join("sdk").join("lib").join("ucrt").join("x86_64"))
            .unwrap();

        let dummy_target =
            TargetCompilationUnit::new(TargetPlatform::Windows, TargetArch::X64, TargetOpt::Debug);

        let mut env = CargoCompilationEnvironment::new(
            dummy_target,
            &[],
            false,
            temp.path(),
            &temp.path().join("target"),
            true,
            None,
            None,
        )
        .unwrap();

        env.attach_windows_sdk_sysroot(&sdk_root, dummy_target)
            .unwrap();

        let cflags = env.c_compiler_flags_for_test();
        assert!(
            cflags.contains("-D_ALLOW_COMPILER_AND_STL_VERSION_MISMATCH=1"),
            "Expected _ALLOW_COMPILER_AND_STL_VERSION_MISMATCH macro definition in CFLAGS"
        );

        #[cfg(not(target_os = "windows"))]
        {
            let count = cflags.matches("-vfsoverlay").count();
            assert_eq!(
                count, 1,
                "Expected exactly 1 -vfsoverlay flag in CFLAGS, found {}",
                count
            );
        }

        match prev_cl {
            Some(val) => std::env::set_var("CRORUST_CLANG_CL", val),
            None => std::env::remove_var("CRORUST_CLANG_CL"),
        }
        match prev_lib {
            Some(val) => std::env::set_var("CRORUST_LLVM_LIB", val),
            None => std::env::remove_var("CRORUST_LLVM_LIB"),
        }
    }

    #[test]
    fn test_macos_sdk_sysroot_vfs_overlay_on_non_macos() {
        let temp = tempfile::tempdir().unwrap();
        let sdk_root = temp.path().join("MacOSX.sdk");
        std::fs::create_dir_all(sdk_root.join("usr/lib/system")).unwrap();
        std::fs::create_dir_all(sdk_root.join("usr/include")).unwrap();
        std::fs::create_dir_all(sdk_root.join("System/Library/Frameworks")).unwrap();
        std::fs::create_dir_all(sdk_root.join("System/Library/PrivateFrameworks")).unwrap();

        let dummy_target =
            TargetCompilationUnit::new(TargetPlatform::Macos, TargetArch::Arm64, TargetOpt::Debug);

        let mut env = CargoCompilationEnvironment::new(
            dummy_target,
            &[],
            false,
            temp.path(),
            &temp.path().join("target"),
            true,
            None,
            None,
        )
        .unwrap();

        env.attach_macos_sdk_sysroot(&sdk_root, dummy_target, "11.0")
            .unwrap();

        #[cfg(not(target_os = "macos"))]
        {
            let cflags = env.c_compiler_flags_for_test();
            let cxxflags = env.cxx_compiler_flags_for_test();
            assert!(
                cflags.contains("-ivfsoverlay"),
                "Expected -ivfsoverlay in CFLAGS for macOS cross-compilation on non-macOS hosts"
            );
            assert!(
                cxxflags.contains("-ivfsoverlay"),
                "Expected -ivfsoverlay in CXXFLAGS for macOS cross-compilation on non-macOS hosts"
            );
            assert!(
                sdk_root.join(".crorust_clang_vfs.yaml").exists(),
                "Expected .crorust_clang_vfs.yaml to be generated in SDK root"
            );
        }
    }

    #[test]
    fn test_ios_sdk_sysroot_vfs_overlay_on_non_macos() {
        let temp = tempfile::tempdir().unwrap();
        let sdk_root = temp.path().join("iPhoneOS.sdk");
        std::fs::create_dir_all(sdk_root.join("usr/lib")).unwrap();
        std::fs::create_dir_all(sdk_root.join("usr/include")).unwrap();
        std::fs::create_dir_all(sdk_root.join("System/Library/Frameworks")).unwrap();

        let dummy_target =
            TargetCompilationUnit::new(TargetPlatform::Ios, TargetArch::Arm64, TargetOpt::Debug);

        let mut env = CargoCompilationEnvironment::new(
            dummy_target,
            &[],
            false,
            temp.path(),
            &temp.path().join("target"),
            true,
            None,
            None,
        )
        .unwrap();

        env.attach_ios_sdk_sysroot(&sdk_root, dummy_target, "14.0")
            .unwrap();

        #[cfg(not(target_os = "macos"))]
        {
            let cflags = env.c_compiler_flags_for_test();
            let cxxflags = env.cxx_compiler_flags_for_test();
            assert!(
                cflags.contains("-ivfsoverlay"),
                "Expected -ivfsoverlay in CFLAGS for iOS cross-compilation on non-macOS hosts"
            );
            assert!(
                cxxflags.contains("-ivfsoverlay"),
                "Expected -ivfsoverlay in CXXFLAGS for iOS cross-compilation on non-macOS hosts"
            );
            assert!(
                sdk_root.join(".crorust_clang_vfs.yaml").exists(),
                "Expected .crorust_clang_vfs.yaml to be generated in SDK root"
            );
        }
    }
}
