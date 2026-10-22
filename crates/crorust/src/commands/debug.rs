use crate::cargo::CompilationCrateKind;
use crate::core::env::SessionExecutionContext;
use crate::core::target::{TargetCompilationUnit, TargetPlatform};
use anyhow::Result;

pub fn debug(env: &SessionExecutionContext) -> Result<()> {
    if let Some(device) = env.build_target().target_device() {
        let target = TargetCompilationUnit::new(
            device.platform()?,
            device.arch()?,
            env.build_target().optimization_profile(),
        );
        let cargo_dir = env.build_directory().join("cargo");
        let executable = match target.target_platform() {
            TargetPlatform::Android => {
                env.resolve_cargo_artifact(&cargo_dir, target, CompilationCrateKind::Cdylib)?
            }
            TargetPlatform::Ios => env.output_path(),
            TargetPlatform::Linux => env.output_path().join(env.app_name()),
            TargetPlatform::Macos => env.executable_path(),
            TargetPlatform::Windows => {
                anyhow::bail!(
                    "Interactive LLDB debugging for Windows targets is not yet supported by crorust. \
                     Use Visual Studio or WinDbg with generated PDB symbols."
                );
            }
            TargetPlatform::Web => {
                anyhow::bail!(
                    "Interactive LLDB debugging is not supported for Web targets. Use browser developer tools."
                );
            }
        };
        let lldb_server = match target.target_platform() {
            TargetPlatform::Android => Some(env.resolve_lldb_server(target)?),
            _ => None,
        };
        device.lldb(env, &executable, lldb_server.as_deref())?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}
