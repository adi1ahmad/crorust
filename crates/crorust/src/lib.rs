//! # crorust: Universal Cross-Compilation & Packaging Orchestrator
//!
//! `crorust` orchestrates the complete cross-compilation, packaging, cryptographic signing,
//! and device deployment pipeline for multi-platform Rust applications.
//!
//! ```mermaid
//! flowchart LR
//!     CLI[CLI Subcommand] --> Res[Target & Arch Resolution]
//!     Res --> Tool[Hermetic Tool Provisioning]
//!     Tool --> Synth[Environment Synthesis]
//!     Synth --> Cargo[Cargo / LLVM Compilation]
//!     Cargo --> Pack[Pure-Rust Platform Bundler]
//!     Pack --> Dev[Device Runner & Logcat Streamer]
//! ```
//!
//! ## Core Subsystems
//! - [`cargo`]: Environment synthesis (`RUSTFLAGS`, `CFLAGS`), linker wiring, and compiler driving.
//! - [`commands`]: Implementations of all subcommands (`build`, `run`, `deploy`, `debug`, `i`, `devices`, `cert`, `cache`).
//! - [`core`]: Contracts, argument types, target models, and path sanitization.
//! - [`devices`]: Device bridge controllers for Android (ADB), Apple (`libimobiledevice`, `simctl`), and local hosts.
//! - [`download`]: Hermetic asset manager with streaming SHA-256 verification and cross-process `fd-lock` locking.
//! - [`packaging`]: Dispatch layer invoking format bundlers (`crorust-android`, `crorust-apple`, `crorust-windows`, `crorust-linux`).
//! - [`task`]: Terminal progress bars and spinners powered by `indicatif`.

pub mod android;
pub mod cargo;
pub mod cli;
pub mod commands;
pub mod config;
pub mod core;
pub mod devices;
pub mod download;
pub mod packaging;
pub mod packaging_cache;
pub mod task;

pub use cargo::{
    CargoCompilationEnvironment, CargoEnvironmentSynthesizer, CargoWorkspaceSession,
    CompilationArtifact, CompilationCrateKind,
};
pub use commands::i::{run_system_i_check, IArgs, SystemIEngine};
pub use core::args::{
    BuildMode, CargoPipelineArguments, PlatformTargetArguments, ProjectBuildArguments,
};
pub use core::contracts::{
    BuildEnvironmentSynthesizer, PackageAssembler, PackagingInput, SynthesizedBuildEnvironment,
    ToolchainProvider,
};
pub use core::env::{
    resolve_ndk_sysroot, sanitize_path, AndroidNdkRoot, AndroidSdkRoot, AppleSdkSysroot,
    SessionExecutionContext, WindowsSdkDirectory,
};
pub use core::target::{
    AppStore, AppleTargetEnv, LinuxLibcVariant, PackageFormat, SessionBuildTarget, TargetArch,
    TargetCompilationUnit, TargetOpt, TargetPlatform,
};
pub use devices::{AndroidDebugBridgeController, DeviceBackend, TargetDevice};
pub use download::manager::ToolManager;
pub use task::{CliTaskReporter, ProgressIndicator};
