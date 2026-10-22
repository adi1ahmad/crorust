use crate::config::{AndroidPlatformConfig, GradleConfig, RepositoryEntry};
use crate::core::env::SessionExecutionContext;
use crate::core::target::{PackageFormat, TargetArch, TargetOpt, TargetPlatform};
use crate::task;
use anyhow::{Context, Result};
use crorust_android::AndroidAbi;
use std::path::{Path, PathBuf};

pub fn generate_adaptive_icon_manifest(
    background_drawable: &str,
    foreground_drawable: &str,
    monochrome_drawable: Option<&str>,
) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n");
    xml.push_str(&format!(
        "    <background android:drawable=\"@mipmap/{background_drawable}\"/>\n"
    ));
    xml.push_str(&format!(
        "    <foreground android:drawable=\"@mipmap/{foreground_drawable}\"/>\n"
    ));
    if let Some(mono) = monochrome_drawable {
        xml.push_str(&format!(
            "    <monochrome android:drawable=\"@mipmap/{mono}\"/>\n"
        ));
    }
    xml.push_str("</adaptive-icon>\n");
    xml
}

pub fn escape_groovy_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

pub fn generate_root_build_gradle(android: &AndroidPlatformConfig) -> String {
    let agp = escape_groovy_string(android.agp_version());
    let kotlin = escape_groovy_string(android.kotlin_version());

    [
        "// Automatically synthesized by crorust - DO NOT EDIT MANUALLY",
        "plugins {",
        &format!("    id 'com.android.application' version '{agp}' apply false"),
        &format!("    id 'com.android.library' version '{agp}' apply false"),
        &format!("    id 'org.jetbrains.kotlin.android' version '{kotlin}' apply false"),
        "}",
        "",
        "task clean(type: Delete) {",
        "    delete rootProject.buildDir",
        "}",
        "",
    ]
    .join("\n")
}

pub fn generate_settings_gradle(android: &AndroidPlatformConfig) -> String {
    generate_settings_gradle_with_env(android, |k| std::env::var(k).ok())
}

pub fn generate_settings_gradle_with_env<F>(
    android: &AndroidPlatformConfig,
    env_lookup: F,
) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let mut repos = String::new();

    if android.repositories.is_empty() {
        repos.push_str("        google()\n        mavenCentral()\n");
    } else {
        for repo_entry in &android.repositories {
            match repo_entry {
                RepositoryEntry::Standard { standard } => match standard.as_str() {
                    "google" => repos.push_str("        google()\n"),
                    "mavenCentral" => repos.push_str("        mavenCentral()\n"),
                    "gradlePluginPortal" => repos.push_str("        gradlePluginPortal()\n"),
                    unknown => {
                        tracing::warn!("Unrecognized standard repository `{unknown}`; skipping");
                    }
                },
                RepositoryEntry::Maven {
                    maven,
                    username_env,
                    password_env,
                } => {
                    let sanitized_url = escape_groovy_string(maven);
                    if let (Some(u_name), Some(p_name)) = (username_env, password_env) {
                        let user = env_lookup(u_name).unwrap_or_default();
                        let pass = env_lookup(p_name).unwrap_or_default();
                        if !user.is_empty() || !pass.is_empty() {
                            let esc_u = escape_groovy_string(&user);
                            let esc_p = escape_groovy_string(&pass);
                            repos.push_str(&format!(
                                "        maven {{\n            url '{sanitized_url}'\n            credentials {{\n                username = '{esc_u}'\n                password = '{esc_p}'\n            }}\n        }}\n"
                            ));
                            continue;
                        }
                    }
                    repos.push_str(&format!("        maven {{ url '{sanitized_url}' }}\n"));
                }
            }
        }
    }

    [
        "// Automatically synthesized by crorust - DO NOT EDIT MANUALLY",
        "pluginManagement {",
        "    repositories {",
        "        gradlePluginPortal()",
        "        google()",
        "        mavenCentral()",
        "    }",
        "}",
        "dependencyResolutionManagement {",
        "    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)",
        "    repositories {",
        &repos,
        "    }",
        "}",
        "",
        "include ':app'",
        "",
    ]
    .join("\n")
}

pub fn generate_gradle_properties(gradle: Option<&GradleConfig>) -> String {
    let jvm_opt = gradle
        .map(|g| g.jvm_args.as_str())
        .unwrap_or("-Xmx2048m -Dfile.encoding=UTF-8");
    let clean_jvm = escape_groovy_string(jvm_opt);

    [
        "# Automatically synthesized by crorust - DO NOT EDIT MANUALLY",
        &format!("org.gradle.jvmargs={clean_jvm}"),
        "android.useAndroidX=true",
        "android.nonTransitiveRClass=true",
        "",
    ]
    .join("\n")
}

struct IconDensitySpec {
    bucket: &'static str,
    dimension: u32,
}

const ICON_DENSITIES: &[IconDensitySpec] = &[
    IconDensitySpec {
        bucket: "mdpi",
        dimension: 48,
    },
    IconDensitySpec {
        bucket: "hdpi",
        dimension: 72,
    },
    IconDensitySpec {
        bucket: "xhdpi",
        dimension: 96,
    },
    IconDensitySpec {
        bucket: "xxhdpi",
        dimension: 144,
    },
    IconDensitySpec {
        bucket: "xxhdpi",
        dimension: 192,
    },
    IconDensitySpec {
        bucket: "xxxhdpi",
        dimension: 256,
    },
];

fn resolve_output_artifact(app_dir: &Path, format: PackageFormat, opt: TargetOpt) -> PathBuf {
    let subfolder = match format {
        PackageFormat::Aab => "bundle",
        PackageFormat::Apk => "apk",
        _ => "output",
    };
    let mode_str = match opt {
        TargetOpt::Debug => "debug",
        TargetOpt::Release => "release",
    };
    let file_leaf = match (format, opt) {
        (PackageFormat::Apk, TargetOpt::Debug) => "app-debug.apk",
        (PackageFormat::Apk, TargetOpt::Release) => "app-release-unsigned.apk",
        (PackageFormat::Aab, TargetOpt::Debug) => "app-debug.aab",
        (PackageFormat::Aab, TargetOpt::Release) => "app-release.aab",
        _ => "app.out",
    };

    app_dir
        .join("build")
        .join("outputs")
        .join(subfolder)
        .join(mode_str)
        .join(file_leaf)
}

pub fn build(
    env: &SessionExecutionContext,
    libraries: Vec<(AndroidAbi, PathBuf)>,
    destination_package: &Path,
) -> Result<()> {
    let root_build_dir = env.target_platform_dir();
    let project_dir = root_build_dir.join("gradle");
    let app_mod_dir = project_dir.join("app");
    let src_main_dir = app_mod_dir.join("src").join("main");
    let kt_dest = src_main_dir.join("kotlin");
    let jni_dest = src_main_dir.join("jniLibs");
    let res_dest = src_main_dir.join("res");
    let assets_dest = src_main_dir.join("assets");

    std::fs::create_dir_all(&kt_dest)?;
    let android_cfg = env.manifest_config().android_platform_config();

    std::fs::write(
        project_dir.join("build.gradle"),
        generate_root_build_gradle(android_cfg),
    )?;
    std::fs::write(
        project_dir.join("gradle.properties"),
        generate_gradle_properties(android_cfg.gradle.as_ref()),
    )?;
    std::fs::write(
        project_dir.join("settings.gradle"),
        generate_settings_gradle(android_cfg),
    )?;

    let sdk_loc = env.locate_android_sdk();
    let sdk_clean = dunce::simplified(&sdk_loc);
    let sdk_unix_path = sdk_clean.to_string_lossy().replace('\\', "/");
    std::fs::write(
        project_dir.join("local.properties"),
        format!("sdk.dir={sdk_unix_path}\n"),
    )?;

    if assets_dest.exists() {
        let _ = std::fs::remove_dir_all(&assets_dest);
    }
    let _ = std::fs::remove_dir_all(project_dir.join("baseAssets"));

    let mut app_manifest = android_cfg.manifest.clone();
    let app_pkg = app_manifest.package.take().unwrap_or_default();
    let target_api = app_manifest.sdk.target_sdk_version.take().unwrap_or(34);
    let min_api = app_manifest.sdk.min_sdk_version.take().unwrap_or(23);
    let vcode = app_manifest.version_code.take().unwrap_or(1);
    let vname = app_manifest
        .version_name
        .take()
        .unwrap_or_else(|| "1.0.0".to_string());
    let compile_api = app_manifest
        .compile_sdk_version
        .take()
        .unwrap_or(target_api);

    app_manifest.compile_sdk_version = None;
    app_manifest.compile_sdk_version_codename = None;
    app_manifest.platform_build_version_code = None;
    app_manifest.platform_build_version_name = None;
    app_manifest.application.debuggable = None;

    let mut dep_directives = String::new();
    for coordinate in &android_cfg.dependencies {
        dep_directives.push_str(&format!("    implementation '{coordinate}'\n"));
    }

    let mut configured_asset_dirs = Vec::new();
    for asset_entry in &android_cfg.assets {
        let source_abs = env
            .cargo_session()
            .package_root_path()
            .join(asset_entry.source_path());
        if !source_abs.exists() {
            if asset_entry.is_optional_asset() {
                continue;
            } else {
                anyhow::bail!(
                    "Configured asset path does not exist: `{}`",
                    source_abs.display()
                );
            }
        }

        let custom_target = asset_entry.destination_target();
        if source_abs.is_dir() {
            let is_direct_root = match custom_target {
                Some(t) => t.as_os_str().is_empty() || t == Path::new("."),
                None => source_abs.file_name().is_some_and(|n| n == "assets"),
            };

            if is_direct_root {
                let formatted = source_abs.to_string_lossy().replace('\\', "/");
                configured_asset_dirs.push(formatted);
            } else {
                let leaf = match custom_target {
                    Some(t) => t,
                    None => Path::new(source_abs.file_name().unwrap()),
                };
                let target_path = assets_dest.join(leaf);
                crorust_common::safe_symlink_or_copy(&source_abs, &target_path)?;
            }
        } else {
            let destination_leaf = match custom_target {
                Some(t) => {
                    if t.as_os_str().is_empty() || t == Path::new(".") {
                        PathBuf::from(source_abs.file_name().unwrap())
                    } else {
                        t.to_path_buf()
                    }
                }
                None => PathBuf::from(source_abs.file_name().unwrap()),
            };
            let target_path = assets_dest.join(destination_leaf);
            crorust_common::safe_symlink_or_copy(&source_abs, &target_path)?;
        }
    }

    let mut asset_source_set = String::new();
    if !configured_asset_dirs.is_empty() {
        asset_source_set
            .push_str("    sourceSets {\n        main {\n            assets.srcDirs += [\n");
        for dir in configured_asset_dirs {
            asset_source_set.push_str(&format!("                '{dir}',\n"));
        }
        asset_source_set.push_str("            ]\n        }\n    }\n");
    }

    let (jvm_version_token, kotlin_target_token) = match android_cfg.jvm_target() {
        "17" => ("JavaVersion.VERSION_17", "17"),
        "11" => ("JavaVersion.VERSION_11", "11"),
        "21" => ("JavaVersion.VERSION_21", "21"),
        _ => ("JavaVersion.VERSION_1_8", "1.8"),
    };

    let build_gradle_body = format!(
        "plugins {{\n    id 'com.android.application'\n    id 'org.jetbrains.kotlin.android'\n}}\n\nandroid {{\n    namespace '{app_pkg}'\n    compileSdk {compile_api}\n\n    defaultConfig {{\n        applicationId '{app_pkg}'\n        minSdk {min_api}\n        targetSdk {target_api}\n        versionCode {vcode}\n        versionName '{vname}'\n    }}\n\n    compileOptions {{\n        sourceCompatibility {jvm_version_token}\n        targetCompatibility {jvm_version_token}\n    }}\n\n    kotlinOptions {{\n        jvmTarget = '{kotlin_target_token}'\n    }}\n\n    buildFeatures {{\n        buildConfig = true\n    }}\n\n{asset_source_set}}}\n\ndependencies {{\n{dep_directives}}}\n"
    );

    if let Some(icon_asset) = env.icon.as_ref() {
        let mut raster = crorust_common::RasterResizer::open(icon_asset)?;
        raster.optimize();
        let anydpi_folder = res_dest.join("mipmap-anydpi-v26");
        std::fs::create_dir_all(&anydpi_folder)?;
        let icon_manifest = generate_adaptive_icon_manifest(
            "ic_launcher_background",
            "ic_launcher_foreground",
            Some("ic_launcher_monochrome"),
        );
        std::fs::write(
            anydpi_folder.join("ic_launcher.xml"),
            icon_manifest.as_bytes(),
        )?;

        for spec in ICON_DENSITIES {
            let target_dir = res_dest.join(format!("mipmap-{}", spec.bucket));
            std::fs::create_dir_all(&target_dir)?;
            for variant_tag in ["foreground", "monochrome"] {
                let mut out_file = std::fs::File::create(
                    target_dir.join(format!("ic_launcher_{variant_tag}.png")),
                )?;
                raster.write(
                    &mut out_file,
                    crorust_common::TransformConstraint::new(spec.dimension),
                )?;
            }
        }
        app_manifest.application.icon = Some("@mipmap/ic_launcher".into());
    }

    std::fs::write(app_mod_dir.join("build.gradle"), build_gradle_body)?;
    std::fs::write(
        src_main_dir.join("AndroidManifest.xml"),
        quick_xml::se::to_string(&app_manifest)?,
    )?;

    let user_kt_dir = env.cargo_session().package_root_path().join("kotlin");
    if user_kt_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(user_kt_dir) {
            for item in entries.flatten() {
                let _ = std::fs::copy(item.path(), kt_dest.join(item.file_name()));
            }
        }
    }

    for (target_triple, lib_path) in libraries {
        let fname = lib_path.file_name().context("invalid lib leaf")?;
        let abi_folder = jni_dest.join(target_triple.as_str());
        std::fs::create_dir_all(&abi_folder)?;
        std::fs::copy(&lib_path, abi_folder.join(fname))?;
    }

    let build_format = env.build_target().package_format();
    let build_opt = env.build_target().optimization_profile();
    let runner_task = match build_format {
        PackageFormat::Aab => "bundle",
        PackageFormat::Apk => "assemble",
        _ => anyhow::bail!("Unsupported package format for Gradle runner"),
    };

    let host_os = TargetPlatform::detect_host()?;
    let host_cpu = TargetArch::detect_host()?;

    let jdk_home = if let Some(found) = env.locate_java_jdk() {
        found
    } else if let Some(artifact) = crate::download::catalog::ToolCatalog::get_jdk(host_os, host_cpu)
    {
        let manager = crate::download::manager::ToolManager::new(env)?;
        manager.ensure_tool(&artifact)?
    } else {
        anyhow::bail!("Compatible Java Development Kit missing from environment");
    };

    let (gradle_root, gradle_binary) = if let Ok(bin) = which::which("gradle") {
        let real = std::fs::canonicalize(&bin).unwrap_or(bin);
        let root = real
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| real.clone());
        (root, real)
    } else {
        let gradle_desc = crate::download::catalog::ToolCatalog::get_gradle("8.7")
            .context("Gradle distribution entry not located")?;
        let manager = crate::download::manager::ToolManager::new(env)?;
        let extracted = manager.ensure_tool(&gradle_desc)?;
        let bin_path = if cfg!(target_os = "windows") {
            extracted.join("bin").join("gradle.bat")
        } else {
            extracted.join("bin").join("gradle")
        };
        (extracted, bin_path)
    };

    let mut execution = crorust_common::create_platform_command(&gradle_binary);
    execution.arg(runner_task);

    let mut synthesizer = crate::download::env::EnvSynthesizer::new();
    synthesizer
        .prepend_path(jdk_home.join("bin"))
        .prepend_path(gradle_root.join("bin"))
        .set_var("JAVA_HOME", &jdk_home)
        .set_var("ANDROID_HOME", &sdk_loc)
        .set_var("ANDROID_SDK_ROOT", &sdk_loc);

    synthesizer.apply(&mut execution)?;
    execution.current_dir(&project_dir);

    task::run(&mut execution)?;

    let compiled_file = resolve_output_artifact(&app_mod_dir, build_format, build_opt);
    std::fs::copy(compiled_file, destination_package)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_adaptive_icon_manifest_structure() {
        let xml = generate_adaptive_icon_manifest("bg", "fg", Some("mono"));
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains(
            "<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">"
        ));
        assert!(xml.contains("<background android:drawable=\"@mipmap/bg\"/>"));
        assert!(xml.contains("<foreground android:drawable=\"@mipmap/fg\"/>"));
        assert!(xml.contains("<monochrome android:drawable=\"@mipmap/mono\"/>"));
        assert!(xml.ends_with("</adaptive-icon>\n"));

        let xml_no_mono = generate_adaptive_icon_manifest("bg2", "fg2", None);
        assert!(!xml_no_mono.contains("<monochrome"));
    }
}
