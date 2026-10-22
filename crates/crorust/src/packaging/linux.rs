use super::{PackagingContext, PackagingOutput, PlatformPackager};
use crate::cargo::CompilationCrateKind;
use crate::core::env::SessionExecutionContext;
use crate::core::target::{PackageFormat, TargetPlatform};
use crate::packaging_cache::{IncrementalDecision, PackagingStamp};
use anyhow::Result;
use crorust_linux::{AppImageContainerBuilder, AppImageOptions};
use std::path::Path;

#[derive(Default)]
pub struct LinuxPackager;

impl LinuxPackager {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformPackager for LinuxPackager {
    fn name(&self) -> &'static str {
        "Linux AppImage / AppDir"
    }

    fn target_platform(&self) -> TargetPlatform {
        TargetPlatform::Linux
    }

    fn check_incremental(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
    ) -> Result<(IncrementalDecision, PackagingStamp)> {
        let target = env.build_target().iter_compile_targets().next().unwrap();
        let arch_dir = ctx.platform_dir.join(target.target_arch().to_string());
        let stamp_path = arch_dir.join(PackagingStamp::FILE_NAME);

        let main = env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Bin)?;
        let mut bins = vec![main];
        if ctx.has_cdylib {
            if let Ok(lib) =
                env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Cdylib)
            {
                bins.push(lib);
            }
        }

        let out_artifact = if env.build_target().package_format() == PackageFormat::Appimage {
            arch_dir.join(format!("{}.AppImage", env.app_name()))
        } else {
            arch_dir
                .join(format!("{}.AppDir", env.app_name()))
                .join(env.app_name())
        };

        let prev_stamp = PackagingStamp::load(&stamp_path);
        let current_stamp = PackagingStamp::compute_incremental(
            &ctx.config_bytes,
            env.app_icon_path(),
            &[],
            &bins,
            None,
            prev_stamp.as_ref(),
        )?;
        let decision = current_stamp.evaluate(prev_stamp.as_ref(), &out_artifact);
        Ok((decision, current_stamp))
    }

    fn package(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
        decision: IncrementalDecision,
        stamp: PackagingStamp,
    ) -> Result<PackagingOutput> {
        let target = env.build_target().iter_compile_targets().next().unwrap();
        let arch_dir = ctx.platform_dir.join(target.target_arch().to_string());
        std::fs::create_dir_all(&arch_dir)?;
        let stamp_path = arch_dir.join(PackagingStamp::FILE_NAME);

        let main = env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Bin)?;
        let cdylib = if ctx.has_cdylib {
            let lib =
                env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Cdylib)?;
            Some(lib)
        } else {
            None
        };

        let appimage = if decision == IncrementalDecision::FastPathBinaryOnly {
            AppImageContainerBuilder::open_or_new(
                &arch_dir,
                env.app_name().to_string(),
                target.target_arch().to_string(),
            )?
        } else {
            let app = AppImageContainerBuilder::new(
                &arch_dir,
                env.app_name().to_string(),
                target.target_arch().to_string(),
            )?;
            app.configure_apprun_entrypoint()?;
            app.generate_desktop_file()?;
            if let Some(icon) = env.app_icon_path() {
                app.install_icon(icon)?;
            }
            app
        };

        appimage.install_file(&main, Path::new(env.app_name()))?;

        if let Some(lib) = &cdylib {
            appimage.install_file(lib, &Path::new("lib").join(lib.file_name().unwrap()))?;
        }

        let primary_artifact = if env.build_target().package_format() == PackageFormat::Appimage {
            let out = arch_dir.join(format!("{}.AppImage", env.app_name()));
            let backend = env
                .build_target()
                .target_squashfs_backend()
                .or(env
                    .manifest_config()
                    .linux_platform_config()
                    .squashfs_backend)
                .unwrap_or_default();
            let compression = env
                .manifest_config()
                .linux_platform_config()
                .compression
                .unwrap_or_default();
            let options = AppImageOptions {
                backend,
                compression,
            };
            appimage.build_with_options(
                &out,
                env.build_target().target_signer().cloned(),
                &options,
            )?;
            out
        } else {
            arch_dir.join(format!("{}.AppDir", env.app_name()))
        };

        stamp.save(&stamp_path)?;

        Ok(PackagingOutput::single(primary_artifact))
    }

    fn verify(&self, artifact: &Path) -> Result<()> {
        anyhow::ensure!(
            artifact.exists(),
            "Linux artifact was not created: {}",
            artifact.display()
        );
        Ok(())
    }
}
