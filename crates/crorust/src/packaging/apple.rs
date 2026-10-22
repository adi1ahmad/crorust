use super::{PackagingContext, PackagingOutput, PlatformPackager};
use crate::cargo::CompilationCrateKind;
use crate::core::env::SessionExecutionContext;
use crate::core::target::{PackageFormat, TargetPlatform};
use crate::packaging_cache::{IncrementalDecision, PackagingStamp};
use anyhow::Result;
use crorust_apple::AppleBundleContainer;
use crorust_common::{ArchiveEntryOptions, BoundaryZipArchive};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct MacosPackager;

impl MacosPackager {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformPackager for MacosPackager {
    fn name(&self) -> &'static str {
        "macOS AppBundle / DMG"
    }

    fn target_platform(&self) -> TargetPlatform {
        TargetPlatform::Macos
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

        let app_out = arch_dir.join(format!("{}.app", env.app_name()));
        let prev_stamp = PackagingStamp::load(&stamp_path);
        let current_stamp = PackagingStamp::compute_incremental(
            &ctx.config_bytes,
            env.app_icon_path(),
            &[],
            &bins,
            None,
            prev_stamp.as_ref(),
        )?;
        let decision = current_stamp.evaluate(prev_stamp.as_ref(), &app_out);
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

        let app_out = if decision != IncrementalDecision::SkipAll {
            let mut app = if decision == IncrementalDecision::FastPathBinaryOnly {
                let mut app = AppleBundleContainer::open_or_new(
                    &arch_dir,
                    env.manifest_config().macos_platform_config().info.clone(),
                )?;
                if env.app_icon_path().is_some() {
                    app.register_existing_icon();
                }
                app
            } else {
                let mut app = AppleBundleContainer::new(
                    &arch_dir,
                    env.manifest_config().macos_platform_config().info.clone(),
                )?;
                app.set_icon_cache_dir(env.icon_cache_dir());
                if let Some(icon) = env.app_icon_path() {
                    app.attach_application_icon(icon)?;
                }
                app
            };

            app.stage_executable_binary(&main)?;

            if let Some(lib) = cdylib {
                app.stage_library(&lib)?;
            }

            app.finalize_bundle(env.build_target().target_signer().cloned())?;
            if let Some(api_key) = env.build_target().apple_api_key() {
                crorust_apple::submit_artifact_for_apple_notarization(
                    app.bundle_appdir(),
                    api_key,
                )?;
            }
            if env.build_target().package_format() == PackageFormat::Dmg {
                let out = arch_dir.join(format!("{}.dmg", env.app_name()));
                apple_dmg::create_dmg(app.bundle_appdir(), &out, env.app_name(), 0x40000)?;
                if let Some(signer) = env.build_target().target_signer() {
                    app.sign_dmg_archive(&out, signer)?;
                    if let Some(api_key) = env.build_target().apple_api_key() {
                        crorust_apple::submit_artifact_for_apple_notarization(&out, api_key)?;
                    }
                }
            }

            stamp.save(&stamp_path)?;
            app.bundle_appdir().to_path_buf()
        } else {
            // Find existing .app directory in arch_dir or fallback to app_name.app
            arch_dir
                .read_dir()
                .ok()
                .and_then(|mut entries| {
                    entries.find_map(|e| {
                        let p = e.ok()?.path();
                        if p.extension().and_then(|ext| ext.to_str()) == Some("app") {
                            Some(p)
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| arch_dir.join(format!("{}.app", env.app_name())))
        };

        let mut output = PackagingOutput::single(app_out);
        if env.build_target().package_format() == PackageFormat::Dmg {
            output = output.with_auxiliary(arch_dir.join(format!("{}.dmg", env.app_name())));
        }
        Ok(output)
    }

    fn verify(&self, artifact: &Path) -> Result<()> {
        anyhow::ensure!(
            artifact.exists(),
            "macOS artifact was not created: {}",
            artifact.display()
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct IosPackager;

impl IosPackager {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformPackager for IosPackager {
    fn name(&self) -> &'static str {
        "iOS AppBundle / IPA"
    }

    fn target_platform(&self) -> TargetPlatform {
        TargetPlatform::Ios
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
        let app_out = arch_dir.join(format!("{}.app", env.app_name()));
        let prev_stamp = PackagingStamp::load(&stamp_path);
        let asset_roots: Vec<PathBuf> = env
            .manifest_config()
            .ios_platform_config()
            .assets_car
            .iter()
            .cloned()
            .collect();
        let current_stamp = PackagingStamp::compute_incremental(
            &ctx.config_bytes,
            env.app_icon_path(),
            &asset_roots,
            std::slice::from_ref(&main),
            None,
            prev_stamp.as_ref(),
        )?;
        let decision = current_stamp.evaluate(prev_stamp.as_ref(), &app_out);
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
        let app_out = if decision != IncrementalDecision::SkipAll {
            let mut app = if decision == IncrementalDecision::FastPathBinaryOnly {
                let mut app = AppleBundleContainer::open_or_new(
                    &arch_dir,
                    env.manifest_config().ios_platform_config().info.clone(),
                )?;
                if env.app_icon_path().is_some() {
                    app.register_existing_icon();
                }
                app
            } else {
                let mut app = AppleBundleContainer::new(
                    &arch_dir,
                    env.manifest_config().ios_platform_config().info.clone(),
                )?;
                app.set_icon_cache_dir(env.icon_cache_dir());
                if let Some(icon) = env.app_icon_path() {
                    app.attach_application_icon(icon)?;
                }
                if let Some(assets_car) = env
                    .manifest_config()
                    .ios_platform_config()
                    .assets_car
                    .as_ref()
                {
                    app.install_resource_file(assets_car, "Assets.car".as_ref())?;
                }
                app
            };

            app.stage_executable_binary(&main)?;
            if let Some(provisioning_profile) = env.build_target().apple_provisioning_profile() {
                app.install_provisioning_profile(provisioning_profile)?;
            }
            app.finalize_bundle(env.build_target().target_signer().cloned())?;

            if env.build_target().package_format() == PackageFormat::Ipa {
                let app_dir = app.bundle_appdir();
                let out = arch_dir.join(format!("{}.ipa", env.app_name()));
                let mut ipa = BoundaryZipArchive::new(&out, false)?;
                ipa.add_directory(
                    app_dir,
                    &Path::new("Payload").join(app_dir.file_name().unwrap()),
                    ArchiveEntryOptions::Compressed,
                )?;
                ipa.finish()?;
            }

            stamp.save(&stamp_path)?;
            app.bundle_appdir().to_path_buf()
        } else {
            arch_dir
                .read_dir()
                .ok()
                .and_then(|mut entries| {
                    entries.find_map(|e| {
                        let p = e.ok()?.path();
                        if p.extension().and_then(|ext| ext.to_str()) == Some("app") {
                            Some(p)
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| arch_dir.join(format!("{}.app", env.app_name())))
        };

        let mut output = PackagingOutput::single(app_out);
        if env.build_target().package_format() == PackageFormat::Ipa {
            output = output.with_auxiliary(arch_dir.join(format!("{}.ipa", env.app_name())));
        }
        Ok(output)
    }

    fn verify(&self, artifact: &Path) -> Result<()> {
        anyhow::ensure!(
            artifact.exists(),
            "iOS artifact was not created: {}",
            artifact.display()
        );
        Ok(())
    }
}
