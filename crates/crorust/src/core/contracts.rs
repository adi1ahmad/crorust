use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub trait ToolchainProvider: Send + Sync {
    fn resolve_android_sdk(&self) -> Result<PathBuf>;
    fn resolve_android_ndk(&self) -> Result<PathBuf>;
    fn resolve_android_jar(&self, target_api: u32) -> Result<PathBuf>;
    fn resolve_windows_sdk(&self) -> Result<PathBuf>;
    fn resolve_macos_sdk(&self) -> Result<PathBuf>;
    fn resolve_ios_sdk(&self) -> Result<PathBuf>;
    fn resolve_jdk(&self) -> Result<PathBuf>;
}

#[derive(Clone, Debug, Default)]
pub struct SynthesizedBuildEnvironment {
    pub cargo_flags: Vec<String>,
    pub environment_overrides: HashMap<String, String>,
    pub sysroot_directory: Option<PathBuf>,
    pub target_triple: String,
}

pub trait BuildEnvironmentSynthesizer: Send + Sync {
    fn synthesize(
        &self,
        target_triple: &str,
        toolchains: &dyn ToolchainProvider,
    ) -> Result<SynthesizedBuildEnvironment>;
}

#[derive(Clone, Debug)]
pub struct PackagingInput {
    pub application_name: String,
    pub package_root_directory: PathBuf,
    pub build_output_directory: PathBuf,
    pub compiled_binaries: Vec<(String, PathBuf)>,
    pub assets_directory: Option<PathBuf>,
}

pub trait PackageAssembler: Send + Sync {
    fn assemble_distribution_package(
        &self,
        inputs: &PackagingInput,
        toolchains: &dyn ToolchainProvider,
    ) -> Result<PathBuf>;
}
