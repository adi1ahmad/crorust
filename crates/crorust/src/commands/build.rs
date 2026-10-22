use crate::cargo::CompilationCrateKind;
use crate::core::env::SessionExecutionContext;
use crate::core::target::TargetPlatform;
use crate::download::SdkToolchainFetcher;
use crate::packaging::{PackagerRegistry, PackagingContext};
use crate::task::ProgressIndicator;
use anyhow::Result;

pub fn build(env: &SessionExecutionContext) -> Result<()> {
    let platform_dir = env.target_platform_dir();
    std::fs::create_dir_all(&platform_dir)?;

    let mut runner = ProgressIndicator::new(3, env.is_verbose());

    runner.begin_phase("Fetch precompiled artifacts");
    let manager = SdkToolchainFetcher::new(env)?;
    if !env.is_offline() {
        manager.prefetch_target_toolchains()?;
        runner.complete_verbose_phase();
    }

    runner.begin_phase(format!("Build rust `{}`", env.app_name()));
    let bin_target = env.build_target().target_platform() != TargetPlatform::Android;
    let has_lib = env.project_root_dir().join("src").join("lib.rs").exists();
    let has_cdylib = env.cargo_session().package_manifest_data().has_cdylib();
    let cargo_dir = env.build_directory().join("cargo");

    let mut compiled_binaries = Vec::new();
    let mut compiled_cdylibs = Vec::new();

    let has_trunk = env.build_target().target_platform() == TargetPlatform::Web
        && (env.project_root_dir().join("Trunk.toml").exists()
            || env.project_root_dir().join("index.html").exists());

    if (bin_target || has_lib) && !has_trunk {
        for target in env.build_target().iter_compile_targets() {
            let mut cargo = env.prepare_cargo_compilation(target, &cargo_dir)?;
            if !bin_target || has_cdylib {
                cargo.append_cargo_argument("--lib");
            }
            cargo.execute_compilation_task()?;

            if bin_target {
                if let Ok(bin) =
                    env.resolve_cargo_artifact(&cargo_dir, target, CompilationCrateKind::Bin)
                {
                    compiled_binaries.push(bin);
                }
            }
            if has_cdylib {
                if let Ok(lib) =
                    env.resolve_cargo_artifact(&cargo_dir, target, CompilationCrateKind::Cdylib)
                {
                    compiled_cdylibs.push(lib);
                }
            }
        }
        runner.complete_verbose_phase();
    } else if has_trunk {
        runner.complete_verbose_phase();
    }

    runner.begin_phase(format!("Create {}", env.build_target().package_format()));

    let ctx = PackagingContext::from_env(env, compiled_binaries, compiled_cdylibs);
    let registry = PackagerRegistry::standard();
    let packager = registry.get(env.build_target().target_platform())?;

    let (decision, stamp) = packager.check_incremental(env, &ctx)?;
    if decision != crate::packaging_cache::IncrementalDecision::SkipAll {
        let output = packager.package(env, &ctx, decision, stamp)?;
        packager.verify(&output.primary_artifact)?;
    } else {
        tracing::debug!(
            "Packaging skipped: inputs unchanged ({})",
            env.build_target().package_format()
        );
    }

    runner.complete_phase();
    Ok(())
}
