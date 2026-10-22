use super::{PackagingContext, PackagingOutput, PlatformPackager};
use crate::cargo::CompilationCrateKind;
use crate::core::env::SessionExecutionContext;
use crate::core::target::{PackageFormat, TargetOpt, TargetPlatform};
use crate::packaging_cache::{IncrementalDecision, PackagingStamp};
use anyhow::{Context, Result};
use crorust_common::ArchiveEntryOptions;
use crorust_windows::WindowsPackageContainer;
use std::path::Path;

#[derive(Default)]
pub struct WindowsPackager;

impl WindowsPackager {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformPackager for WindowsPackager {
    fn name(&self) -> &'static str {
        "Windows MSIX / Exe"
    }

    fn target_platform(&self) -> TargetPlatform {
        TargetPlatform::Windows
    }

    fn check_incremental(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
    ) -> Result<(IncrementalDecision, PackagingStamp)> {
        let target = env.build_target().iter_compile_targets().next().unwrap();
        let arch_dir = ctx.platform_dir.join(target.target_arch().to_string());
        let stamp_path = arch_dir.join(PackagingStamp::FILE_NAME);
        let out = arch_dir.join(format!(
            "{}.{}",
            env.app_name(),
            env.build_target().package_format()
        ));

        let main = env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Bin)?;
        let mut bins = vec![main];
        if ctx.has_cdylib {
            if let Ok(lib) =
                env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Cdylib)
            {
                bins.push(lib);
            }
        }

        let mut asset_roots = Vec::new();
        let project_assets = env.cargo_session().package_root_path().join("assets");
        if project_assets.is_dir() {
            asset_roots.push(project_assets);
        }

        let prev_stamp = PackagingStamp::load(&stamp_path);
        let current_stamp = PackagingStamp::compute_incremental(
            &ctx.config_bytes,
            env.app_icon_path(),
            &asset_roots,
            &bins,
            None,
            prev_stamp.as_ref(),
        )?;
        let decision = current_stamp.evaluate(prev_stamp.as_ref(), &out);
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
        let out = arch_dir.join(format!(
            "{}.{}",
            env.app_name(),
            env.build_target().package_format()
        ));
        let main = env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Bin)?;
        let cdylib = if ctx.has_cdylib {
            let lib =
                env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Cdylib)?;
            Some(lib)
        } else {
            None
        };

        let mut asset_roots = Vec::new();
        let project_assets = env.cargo_session().package_root_path().join("assets");
        if project_assets.is_dir() {
            asset_roots.push(project_assets);
        }

        match env.build_target().package_format() {
            PackageFormat::Exe => {
                std::fs::copy(&main, &out)?;
            }
            PackageFormat::Msix => {
                let stamp_path = arch_dir.join(PackagingStamp::FILE_NAME);

                match decision {
                    IncrementalDecision::SkipAll => {
                        tracing::debug!("Windows MSIX up to date; skipping packaging");
                    }
                    IncrementalDecision::FastPathBinaryOnly => {
                        tracing::info!("Executing fast-path binary update for Windows MSIX");
                        let mut repack_bins = vec![(&main, format!("{}.exe", env.app_name()))];
                        if let Some(ref lib) = cdylib {
                            let lib_file_name = lib.file_name().with_context(|| {
                                format!(
                                    "Dynamic library path `{}` is missing a filename component",
                                    lib.display()
                                )
                            })?;
                            repack_bins.push((lib, lib_file_name.to_string_lossy().to_string()));
                        }
                        let binaries_slice: Vec<(&Path, String)> = repack_bins
                            .iter()
                            .map(|(p, s)| (p.as_path(), s.clone()))
                            .collect();
                        WindowsPackageContainer::repackage_binaries(
                            &out,
                            &binaries_slice,
                            env.build_target().target_signer().cloned(),
                            target.optimization_profile() != TargetOpt::Debug,
                        )?;
                        let _ = stamp.save(&stamp_path);
                    }
                    IncrementalDecision::FullRebuild => {
                        let mut msix = WindowsPackageContainer::new(
                            out.clone(),
                            env.manifest_config()
                                .windows_platform_config()
                                .manifest
                                .clone(),
                            target.optimization_profile() != TargetOpt::Debug,
                        )?;
                        msix.set_icon_cache_dir(env.icon_cache_dir());
                        if let Some(icon) = env.app_icon_path() {
                            msix.embed_application_icon(icon)?;
                        }

                        for asset_dir in &asset_roots {
                            if asset_dir.is_dir() {
                                msix.add_assets_dir(asset_dir, Path::new("assets"))?;
                            }
                        }

                        msix.embed_file(
                            &main,
                            format!("{}.exe", env.app_name()).as_ref(),
                            ArchiveEntryOptions::Compressed,
                        )?;

                        if let Some(lib) = cdylib {
                            let lib_file_name = lib.file_name().with_context(|| {
                                format!(
                                    "Dynamic library path `{}` is missing a filename component",
                                    lib.display()
                                )
                            })?;
                            msix.embed_file(
                                &lib,
                                Path::new(lib_file_name),
                                ArchiveEntryOptions::Compressed,
                            )?;
                        }

                        msix.finalize_package(env.build_target().target_signer().cloned())?;
                        let _ = stamp.save(&stamp_path);
                    }
                }
            }
            _ => {
                anyhow::bail!(
                    "Unsupported Windows format: {}",
                    env.build_target().package_format()
                );
            }
        }

        Ok(PackagingOutput::single(out))
    }

    fn verify(&self, artifact: &Path) -> Result<()> {
        anyhow::ensure!(
            artifact.exists(),
            "Windows artifact was not created: {}",
            artifact.display()
        );
        Ok(())
    }
}
