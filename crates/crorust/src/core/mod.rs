pub mod args;
pub mod contracts;
pub mod env;
pub mod target;

pub use args::{BuildMode, CargoPipelineArguments, PlatformTargetArguments, ProjectBuildArguments};
pub use contracts::{
    BuildEnvironmentSynthesizer, PackageAssembler, PackagingInput, SynthesizedBuildEnvironment,
    ToolchainProvider,
};
pub use env::{
    resolve_ndk_sysroot, sanitize_path, AndroidNdkRoot, AndroidSdkRoot, AppleSdkSysroot,
    SessionExecutionContext, WindowsSdkDirectory,
};
pub type BuildEnv = SessionExecutionContext;
pub use target::{
    AppStore, AppleTargetEnv, LinuxLibcVariant, PackageFormat, SessionBuildTarget, TargetArch,
    TargetCompilationUnit, TargetOpt, TargetPlatform,
};
