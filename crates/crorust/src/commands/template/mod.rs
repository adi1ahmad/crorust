use crate::commands;
use crate::core::args::ProjectBuildArguments;
use crate::core::env::SessionExecutionContext;
use crate::core::target::TargetPlatform;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub mod scaffold;
pub use scaffold::{extract_raw_template_archive, init, ProjectNames, ScaffoldEngine};

pub fn resolve_template_dir() -> Result<PathBuf> {
    let in_workspace = Path::new("templates/crorust.ruxt");
    if in_workspace.join("Cargo.toml").exists() {
        return dunce::canonicalize(in_workspace)
            .context("Failed to canonicalize in-repo template path");
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("../../templates/crorust.ruxt");
            if candidate.join("Cargo.toml").exists() {
                return dunce::canonicalize(candidate)
                    .context("Failed to canonicalize template path relative to executable");
            }
        }
    }

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("crorust/template/ruxt");

    let marker = cache_dir.join(".extracted");
    if !marker.exists() {
        std::fs::create_dir_all(&cache_dir)?;
        scaffold::extract_raw_template_archive(&cache_dir)?;
        std::fs::write(marker, b"1")?;
    }

    Ok(cache_dir)
}

fn inject_template_project(mut args: ProjectBuildArguments) -> Result<ProjectBuildArguments> {
    let tmpl_dir = resolve_template_dir()?;
    args.cargo.set_project(tmpl_dir);
    Ok(args)
}

pub fn run(args: ProjectBuildArguments, launch_args: &[String]) -> Result<()> {
    let args = inject_template_project(args)?;
    let env = SessionExecutionContext::new_with_context(args, false)?;

    if let Some(device) = env.build_target().target_device() {
        println!(
            "Targeting template: {} ({}) [{}]",
            device,
            device
                .device_name()
                .unwrap_or_else(|_| "Unknown Device".into()),
            env.build_target().optimization_profile()
        );
    }

    if env.build_target().target_platform() != TargetPlatform::Web {
        commands::build(&env)?;
    }
    commands::run(&env, launch_args)
}

pub fn deploy(args: ProjectBuildArguments, launch_args: &[String]) -> Result<()> {
    let args = inject_template_project(args)?;
    let env = SessionExecutionContext::new_with_context(args, true)?;

    if let Some(device) = env.build_target().target_device() {
        println!(
            "Deploying template: {} ({}) [{}]",
            device,
            device
                .device_name()
                .unwrap_or_else(|_| "Unknown Device".into()),
            env.build_target().optimization_profile()
        );
    }

    if env.build_target().target_platform() != TargetPlatform::Web {
        commands::build(&env)?;
    }
    commands::run(&env, launch_args)
}

pub fn build(args: ProjectBuildArguments) -> Result<()> {
    let args = inject_template_project(args)?;
    let env = SessionExecutionContext::new(args)?;
    commands::build(&env)
}

pub fn check(platform_filter: Option<TargetPlatform>) -> Result<()> {
    let tmpl_dir = resolve_template_dir()?;
    println!("Typechecking template at `{}`...", tmpl_dir.display());

    let host_platform = TargetPlatform::detect_host().unwrap_or(TargetPlatform::Linux);
    let platforms = match platform_filter {
        Some(p) => vec![p],
        None => vec![host_platform, TargetPlatform::Web],
    };

    for platform in platforms {
        print!("  Checking platform [{platform}]... ");
        let mut cmd = std::process::Command::new("cargo");
        cmd.current_dir(&tmpl_dir);

        cmd.env_remove("RUSTUP_TOOLCHAIN");
        cmd.arg("check");

        match platform {
            TargetPlatform::Web => {
                cmd.arg("--target")
                    .arg("wasm32-unknown-unknown")
                    .arg("--lib");
            }
            TargetPlatform::Android => {
                cmd.arg("--target")
                    .arg("aarch64-linux-android")
                    .arg("--lib");
            }
            TargetPlatform::Linux | TargetPlatform::Macos | TargetPlatform::Windows => {}
            TargetPlatform::Ios => {
                cmd.arg("--target")
                    .arg("aarch64-apple-ios")
                    .arg("--bin")
                    .arg("ruxt");
            }
        }

        let status = cmd
            .status()
            .with_context(|| format!("Failed to run cargo check for {platform}"))?;

        if status.success() {
            println!("PASS");
        } else {
            anyhow::bail!("cargo check failed for template on platform {}", platform);
        }
    }

    println!("All requested template platform checks passed successfully.");
    Ok(())
}

/// Display metadata and capabilities of the bundled reference template
pub fn info() -> Result<()> {
    println!("Embedded Reference Template: crorust.ruxt");
    println!("  Primary Framework:  egui / eframe (Cross-platform immediate-mode GUI)");
    println!("  Graphics Backend:   wgpu (DirectX 12, Metal, Vulkan, WebGPU, GLES 3)");
    println!("  Supported Targets:");
    println!("    - Linux Desktop:  Native X11 / Wayland (.AppImage / AppDir)");
    println!("    - macOS:          Native Cocoa (.app bundle / DMG)");
    println!("    - Windows:        Native Win32 (.exe / MSIX package)");
    println!("    - Android:        NativeActivity + Kotlin bridge (.apk)");
    println!("    - iOS:            UIKit (.app / IPA)");
    println!("    - Web:            WebAssembly (wasm32-unknown-unknown + HTML5 Canvas)");
    println!("  Included Tooling:   Hermetic Nix Flake, CI workflow scripts, VS Code settings");
    println!("\nTo scaffold a new project:");
    println!("  crorust template init <project_name>");
    Ok(())
}
