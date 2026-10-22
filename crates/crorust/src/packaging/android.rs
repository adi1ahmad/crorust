use super::{PackagingContext, PackagingOutput, PlatformPackager};
use crate::android::compiler::AndroidBytecodeCompiler;
use crate::cargo::CompilationCrateKind;
use crate::core::env::SessionExecutionContext;
use crate::core::target::{PackageFormat, TargetOpt, TargetPlatform};
use crate::packaging_cache::{AndroidBytecodeStamp, IncrementalDecision, PackagingStamp};
use anyhow::{ensure, Context, Result};
use crorust_android::{Aab, AndroidAbi, AndroidPackageContainer};
use crorust_common::ArchiveEntryOptions;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(crate) fn collect_unpacked_r_symbols(unpacked_aars_dir: &Path) -> String {
    let mut r_concat = String::new();
    if unpacked_aars_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(unpacked_aars_dir) {
            let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
            paths.sort();
            for dir in paths {
                let r_txt = dir.join("R.txt");
                if let Ok(content) = std::fs::read_to_string(r_txt) {
                    r_concat.push_str(&content);
                }
            }
        }
    }
    r_concat
}

#[derive(Default)]
pub struct AndroidPackager;

impl AndroidPackager {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformPackager for AndroidPackager {
    fn name(&self) -> &'static str {
        "Android APK / AAB"
    }

    fn target_platform(&self) -> TargetPlatform {
        TargetPlatform::Android
    }

    fn check_incremental(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
    ) -> Result<(IncrementalDecision, PackagingStamp)> {
        let out = ctx.platform_dir.join(format!(
            "{}.{}",
            env.app_name(),
            env.build_target().package_format()
        ));
        let stamp_path = ctx.platform_dir.join(PackagingStamp::FILE_NAME);

        let asset_roots: Vec<PathBuf> = env
            .manifest_config()
            .android_platform_config()
            .assets
            .iter()
            .map(|a| {
                env.cargo_session()
                    .package_root_path()
                    .join(a.source_path())
            })
            .collect();

        let mut lib_paths = Vec::new();
        for target in env.build_target().iter_compile_targets() {
            if let Ok(lib) =
                env.resolve_cargo_artifact(&ctx.cargo_dir, target, CompilationCrateKind::Cdylib)
            {
                lib_paths.push(lib);
            }
        }

        let kotlin_src_dir = env.cargo_session().package_root_path().join("kotlin");
        let min_sdk = env
            .manifest_config()
            .android_platform_config()
            .min_sdk
            .or(env
                .manifest_config()
                .android_platform_config()
                .manifest
                .sdk
                .min_sdk_version)
            .unwrap_or(23);
        let desugar_enabled = min_sdk < 26;
        let desugar_rules_hash = if desugar_enabled {
            crate::download::catalog::ToolCatalog::get_desugar_jdk_libs_config("2.1.4")
                .map(|a| a.sha256.to_string())
        } else {
            None
        };

        let unpacked_aars_dir = ctx.platform_dir.join("unpacked_aars");
        let r_concat = collect_unpacked_r_symbols(&unpacked_aars_dir);

        let dex_stamp = AndroidBytecodeStamp::compute(
            std::slice::from_ref(&kotlin_src_dir),
            &env.manifest_config().android_platform_config().dependencies,
            &r_concat,
            min_sdk,
            env.manifest_config()
                .android_platform_config()
                .manifest
                .sdk
                .target_sdk_version
                .unwrap_or(34),
            desugar_enabled,
            desugar_rules_hash,
        )?;

        let prev_stamp = PackagingStamp::load(&stamp_path);
        let current_stamp = PackagingStamp::compute_incremental(
            &ctx.config_bytes,
            env.app_icon_path(),
            &asset_roots,
            &lib_paths,
            Some(dex_stamp.fingerprint()),
            prev_stamp.as_ref(),
        )?;
        let decision = if env.build_target().package_format() == PackageFormat::Apk {
            current_stamp.evaluate(prev_stamp.as_ref(), &out)
        } else {
            IncrementalDecision::FullRebuild
        };
        Ok((decision, current_stamp))
    }

    fn package(
        &self,
        env: &SessionExecutionContext,
        ctx: &PackagingContext,
        decision: IncrementalDecision,
        stamp: PackagingStamp,
    ) -> Result<PackagingOutput> {
        let out = ctx.platform_dir.join(format!(
            "{}.{}",
            env.app_name(),
            env.build_target().package_format()
        ));
        ensure!(ctx.has_lib, "Android APKs/AABs require a library");
        let mut libraries: Vec<(AndroidAbi, PathBuf)> = Vec::new();
        let ndk_home = env.resolve_android_ndk();

        for target_desc in env.build_target().iter_compile_targets() {
            let compiled_cdylib = env.resolve_cargo_artifact(
                &ctx.cargo_dir,
                target_desc,
                CompilationCrateKind::Cdylib,
            )?;

            let target_subfolder = match target_desc.rust_target_triple() {
                Ok(triple) if !target_desc.targets_current_host().unwrap_or(false) => {
                    ctx.cargo_dir.join(triple)
                }
                _ => ctx.cargo_dir.to_path_buf(),
            };
            let build_deps_folder = target_subfolder
                .join(target_desc.optimization_profile().to_string())
                .join("deps");

            let mut candidate_search_dirs = env
                .cargo_session()
                .collect_build_dependency_search_paths(&ctx.cargo_dir, target_desc)
                .with_context(|| {
                    format!(
                        "Locating dependent native shared libraries in `{}` for target {:?}",
                        ctx.cargo_dir.display(),
                        target_desc
                    )
                })?;
            candidate_search_dirs.push(build_deps_folder);
            if let Some(ndk_clang_dir) =
                crate::cargo::find_ndk_clang_lib_dir(&ndk_home, target_desc.target_arch())
            {
                candidate_search_dirs.push(ndk_clang_dir);
            }
            let search_paths = candidate_search_dirs
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>();

            let ndk_sysroot_libs = ndk_home
                .join("usr/lib")
                .join(target_desc.android_ndk_triple());
            let min_api_str = env
                .manifest_config()
                .android_platform_config()
                .manifest
                .sdk
                .min_sdk_version
                .unwrap_or(21)
                .to_string();
            let min_api_libs = ndk_sysroot_libs.join(&min_api_str);
            let provided_libs_paths = [ndk_sysroot_libs.as_path(), min_api_libs.as_path()];

            let mut explicit_libs = vec![compiled_cdylib];

            for runtime_rel_path in env
                .manifest_config()
                .external_runtime_libs(env.build_target().target_platform())
            {
                let current_abi = target_desc.android_target_abi().as_str();
                let candidate_dir = env
                    .cargo_session()
                    .package_root_path()
                    .join(runtime_rel_path)
                    .join(current_abi);
                let dir_iterator = std::fs::read_dir(&candidate_dir).with_context(|| {
                    format!(
                        "Target ABI runtime library directory does not exist at `{}`",
                        candidate_dir.display()
                    )
                })?;
                for item in dir_iterator {
                    let entry_item = item?;
                    let file_candidate = entry_item.path();
                    if file_candidate.is_file()
                        && file_candidate.extension() == Some(OsStr::new("so"))
                    {
                        explicit_libs.push(file_candidate);
                    }
                }
            }

            let mut library_filter = HashSet::new();
            for bin_file in &explicit_libs {
                if let Some(name) = bin_file.file_name() {
                    library_filter.insert(name.to_os_string());
                }
            }
            for candidate_dir in provided_libs_paths {
                library_filter.extend(crorust_common::elf_resolver::scan_directory_for_libraries(
                    candidate_dir,
                )?);
            }
            library_filter.remove(OsStr::new("libc++_shared.so"));

            let mut requires_cxx_runtime = false;
            for bin_file in explicit_libs {
                libraries.push((target_desc.android_target_abi(), bin_file.clone()));

                let resolution = crorust_common::elf_resolver::trace_dynamic_dependencies(
                    &bin_file,
                    &search_paths,
                    &library_filter,
                )
                .with_context(|| {
                    format!(
                        "Transitive ELF dependency traversal failed for `{}`",
                        bin_file.display()
                    )
                })?;

                requires_cxx_runtime |= resolution.requires_ndk_cxx_runtime;
                for dep_lib in resolution.resolved_paths {
                    libraries.push((target_desc.android_target_abi(), dep_lib));
                }
            }

            if requires_cxx_runtime {
                let libcxx = ndk_sysroot_libs.join("libc++_shared.so");
                libraries.push((target_desc.android_target_abi(), libcxx));
            }
        }

        if env.build_target().is_android_gradle_enabled() {
            crate::android::gradle::build(env, libraries.clone(), &out)?;
            return Ok(PackagingOutput::single(out));
        }

        let stamp_path = ctx.platform_dir.join(PackagingStamp::FILE_NAME);

        match decision {
            IncrementalDecision::SkipAll => {
                tracing::debug!(
                    "APK is up to date ({}); skipping repackaging",
                    out.display()
                );
            }
            IncrementalDecision::FastPathBinaryOnly => {
                tracing::debug!(
                    "Fast-path Android repackaging: cloning non-lib APK entries and updating native libraries"
                );
                AndroidPackageContainer::repackage_libs(
                    &out,
                    &libraries,
                    env.build_target().target_signer().cloned(),
                    crorust_android::ANDROID_SO_ALIGNMENT,
                    env.build_target().optimization_profile() != TargetOpt::Debug,
                )?;
                stamp.save(&stamp_path)?;
            }
            IncrementalDecision::FullRebuild => {
                let resolved_deps = if !env
                    .manifest_config()
                    .android_platform_config()
                    .dependencies
                    .is_empty()
                {
                    let resolver = crate::android::mvn::AndroidMvnResolver::new(env)?;
                    resolver.resolve_and_unpack(env.manifest_config().android_platform_config())?
                } else {
                    crate::android::mvn::ResolvedArtifacts::default()
                };

                let target_abis: HashSet<_> = env
                    .build_target()
                    .iter_compile_targets()
                    .map(|t| t.android_target_abi())
                    .collect();
                for (target, lib_path) in &resolved_deps.native_libs {
                    if target_abis.contains(target) {
                        libraries.push((*target, lib_path.clone()));
                    }
                }

                let kotlin_src_dir = env.cargo_session().package_root_path().join("kotlin");
                let mut asset_roots: Vec<PathBuf> = env
                    .manifest_config()
                    .android_platform_config()
                    .assets
                    .iter()
                    .map(|a| {
                        env.cargo_session()
                            .package_root_path()
                            .join(a.source_path())
                    })
                    .collect();
                asset_roots.extend(resolved_deps.asset_dirs.iter().cloned());
                let has_user_kotlin = kotlin_src_dir.exists()
                    && kotlin_src_dir.read_dir()?.filter_map(|e| e.ok()).any(|e| {
                        let p = e.path();
                        p.is_file()
                            && p.extension()
                                .is_some_and(|ext| ext == "kt" || ext == "java")
                    });

                let (dex_paths, mut resource_symbols): (
                    Vec<PathBuf>,
                    Option<crorust_android::ResourceSymbols>,
                ) = if has_user_kotlin
                    || !resolved_deps.classpath_jars.is_empty()
                    || !resolved_deps.r_txt_files.is_empty()
                {
                    let compiler = AndroidBytecodeCompiler::new(env)?;
                    let (dex, sym) = compiler.compile(
                        env.manifest_config().android_platform_config(),
                        &resolved_deps,
                    )?;
                    (dex, sym)
                } else {
                    (Vec::new(), None)
                };

                let aar_strings =
                    crorust_android::manifest::collect_aar_strings(&resolved_deps.res_dirs)?;
                let aar_files =
                    crorust_android::manifest::collect_aar_file_resources(&resolved_deps.res_dirs)?;

                let mut file_res_paths = BTreeMap::new();
                for (key, (_src, rel)) in &aar_files {
                    file_res_paths.insert(key.clone(), rel.clone());
                }

                if let Some(ref mut symbols) = resource_symbols {
                    symbols.string_values = aar_strings;
                    symbols.file_resources = file_res_paths;
                } else if !aar_strings.is_empty() || !file_res_paths.is_empty() {
                    resource_symbols = Some(crorust_android::ResourceSymbols {
                        string_values: aar_strings,
                        file_resources: file_res_paths,
                        ..Default::default()
                    });
                }

                let mut manifest = env
                    .manifest_config()
                    .android_platform_config()
                    .manifest
                    .clone();
                if !resolved_deps.manifest_files.is_empty() {
                    crorust_android::manifest::merge_manifests(
                        &mut manifest,
                        &resolved_deps.manifest_files,
                    )?;
                }

                if env.build_target().package_format() == PackageFormat::Aab {
                    let mut aab = Aab::new(out.clone(), manifest)?;
                    aab.add_bundle_config()?;
                    aab.add_manifest()?;
                    aab.add_resources(env.app_icon_path(), resource_symbols.as_ref())?;

                    for (idx, dex_path) in dex_paths.iter().enumerate() {
                        aab.add_dex(dex_path, idx)?;
                    }

                    for asset in &env.manifest_config().android_platform_config().assets {
                        let path = env
                            .cargo_session()
                            .package_root_path()
                            .join(asset.source_path());

                        if !asset.is_optional_asset() || path.exists() {
                            let dest_rel = match asset.destination_target() {
                                Some(t) => t.to_path_buf(),
                                None => {
                                    let is_root_dir = path.is_dir()
                                        && path.file_name().is_some_and(|n| n == "assets");
                                    if is_root_dir {
                                        PathBuf::from("")
                                    } else {
                                        PathBuf::from(
                                            path.file_name().context("Invalid asset path")?,
                                        )
                                    }
                                }
                            };
                            if path.is_file() {
                                aab.add_asset(&path, &dest_rel)?;
                            } else if path.is_dir() {
                                for entry in walkdir::WalkDir::new(&path)
                                    .into_iter()
                                    .filter_map(|e| e.ok())
                                {
                                    if entry.path().is_file() {
                                        let rel = entry.path().strip_prefix(&path)?;
                                        aab.add_asset(entry.path(), &dest_rel.join(rel))?;
                                    }
                                }
                            }
                        }
                    }

                    for aar_asset_dir in &resolved_deps.asset_dirs {
                        if aar_asset_dir.is_dir() {
                            for entry in walkdir::WalkDir::new(aar_asset_dir)
                                .into_iter()
                                .filter_map(|e| e.ok())
                            {
                                if entry.path().is_file() {
                                    let rel = entry.path().strip_prefix(aar_asset_dir)?;
                                    aab.add_asset(entry.path(), rel)?;
                                }
                            }
                        }
                    }

                    for (target, lib) in &libraries {
                        aab.add_lib(target.as_str(), lib)?;
                    }

                    aab.finish(env.build_target().target_signer().cloned())?;
                } else {
                    let mut apk = AndroidPackageContainer::new(
                        out.clone(),
                        manifest,
                        env.build_target().optimization_profile() != TargetOpt::Debug,
                    )?;
                    apk.set_icon_cache_dir(env.icon_cache_dir());
                    apk.bundle_resources_with_symbols(
                        env.app_icon_path(),
                        &env.resolve_android_jar(),
                        Some("Theme.AppCompat.NoActionBar"),
                        resource_symbols.as_ref(),
                    )?;

                    let mut res_table = crorust_android::ResourceCatalogTable::default();
                    res_table.import_apk(&env.resolve_android_jar())?;

                    for (src_path, apk_rel_path) in aar_files.values() {
                        let dest = Path::new(apk_rel_path);
                        if src_path.extension().is_some_and(|e| e == "xml") {
                            if let Ok(xml_content) = std::fs::read_to_string(src_path) {
                                match crorust_android::compile_xml_with_symbols(
                                    &xml_content,
                                    &res_table,
                                    resource_symbols.as_ref(),
                                ) {
                                    Ok(chunk) => {
                                        let mut axml_bytes = Vec::new();
                                        let mut cursor = std::io::Cursor::new(&mut axml_bytes);
                                        if chunk.write(&mut cursor).is_ok() {
                                            let _ = apk.add_raw_file(
                                                dest,
                                                ArchiveEntryOptions::Compressed,
                                                &axml_bytes,
                                            );
                                            continue;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::debug!(
                                            "Could not compile XML `{}`: {}, storing verbatim",
                                            src_path.display(),
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        let _ = apk.add_file_at(src_path, dest, ArchiveEntryOptions::Compressed);
                    }

                    for dex_path in &dex_paths {
                        apk.bundle_dex(dex_path)?;
                    }

                    for asset in &env.manifest_config().android_platform_config().assets {
                        let path = env
                            .cargo_session()
                            .package_root_path()
                            .join(asset.source_path());

                        if !asset.is_optional_asset() || path.exists() {
                            let dest_rel = match asset.destination_target() {
                                Some(t) => t.to_path_buf(),
                                None => {
                                    let is_root_dir = path.is_dir()
                                        && path.file_name().is_some_and(|n| n == "assets");
                                    if is_root_dir {
                                        PathBuf::from("")
                                    } else {
                                        PathBuf::from(
                                            path.file_name().context("Invalid asset path")?,
                                        )
                                    }
                                }
                            };
                            apk.add_asset_with_dest(
                                &path,
                                &dest_rel,
                                asset.entry_alignment().into_archive_entry_options(),
                            )?;
                        }
                    }

                    for aar_asset_dir in &resolved_deps.asset_dirs {
                        apk.add_asset_with_dest(
                            aar_asset_dir,
                            Path::new(""),
                            ArchiveEntryOptions::Compressed,
                        )?;
                    }

                    for (target_abi, library_file) in &libraries {
                        apk.bundle_shared_library(*target_abi, library_file)?;
                    }

                    let packaging_keypair = env.build_target().target_signer().cloned();
                    apk.finalize_apk(packaging_keypair)?;
                    let unpacked_aars_dir = ctx.platform_dir.join("unpacked_aars");
                    let final_r_concat = collect_unpacked_r_symbols(&unpacked_aars_dir);
                    let min_sdk = env
                        .manifest_config()
                        .android_platform_config()
                        .min_sdk
                        .or(env
                            .manifest_config()
                            .android_platform_config()
                            .manifest
                            .sdk
                            .min_sdk_version)
                        .unwrap_or(23);
                    let desugar_enabled = min_sdk < 26;
                    let desugar_rules_hash = if desugar_enabled {
                        crate::download::catalog::ToolCatalog::get_desugar_jdk_libs_config("2.1.4")
                            .map(|a| a.sha256.to_string())
                    } else {
                        None
                    };
                    let mut lib_paths = Vec::new();
                    for target in env.build_target().iter_compile_targets() {
                        if let Ok(lib) = env.resolve_cargo_artifact(
                            &ctx.cargo_dir,
                            target,
                            CompilationCrateKind::Cdylib,
                        ) {
                            lib_paths.push(lib);
                        }
                    }
                    let final_dex_stamp = AndroidBytecodeStamp::compute(
                        std::slice::from_ref(&kotlin_src_dir),
                        &env.manifest_config().android_platform_config().dependencies,
                        &final_r_concat,
                        min_sdk,
                        env.manifest_config()
                            .android_platform_config()
                            .manifest
                            .sdk
                            .target_sdk_version
                            .unwrap_or(34),
                        desugar_enabled,
                        desugar_rules_hash,
                    )?;
                    let post_stamp = PackagingStamp::compute_incremental(
                        &ctx.config_bytes,
                        env.app_icon_path(),
                        &asset_roots,
                        &lib_paths,
                        Some(final_dex_stamp.fingerprint()),
                        Some(&stamp),
                    )?;
                    post_stamp.save(&stamp_path)?;
                }
            }
        }

        Ok(PackagingOutput::single(out))
    }

    fn verify(&self, artifact: &Path) -> Result<()> {
        if artifact.extension() == Some(OsStr::new("apk")) {
            if let Ok(certs) = AndroidPackageContainer::verify_apk_target(artifact) {
                tracing::debug!(
                    "APK signature verified successfully with {} certificate(s)",
                    certs.len()
                );
            }
        }
        Ok(())
    }
}
