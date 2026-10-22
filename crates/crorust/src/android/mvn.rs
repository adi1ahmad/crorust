use crate::config::{AndroidPlatformConfig, RepositoryEntry};
use crate::core::env::SessionExecutionContext;
use crate::download::SdkToolchainFetcher;
use anyhow::{Context, Result};
use crorust_android::AndroidAbi;
use crorust_mvn::{
    DependencyRequirement, MavenCoordinates, ParallelAarUnpacker, RepositoryClient,
    RepositoryHttpCredentials, SemverVersion, GOOGLE_MAVEN, MAVEN_CENTRAL,
};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Clone, Debug, Default)]
pub struct ResolvedArtifacts {
    pub classpath_jars: Vec<PathBuf>,

    pub native_libs: Vec<(AndroidAbi, PathBuf)>,

    pub res_dirs: Vec<PathBuf>,

    pub asset_dirs: Vec<PathBuf>,

    pub manifest_files: Vec<PathBuf>,

    pub r_txt_files: Vec<(String, PathBuf)>,
}

pub struct AndroidMvnResolver<'a> {
    env: Option<&'a SessionExecutionContext>,
    maven_cache_dir: PathBuf,
    unpack_dir: PathBuf,
}

impl<'a> AndroidMvnResolver<'a> {
    pub fn new(env: &'a SessionExecutionContext) -> Result<Self> {
        let maven_cache_dir = env.cache_directory().join("maven");
        let unpack_dir = env.target_platform_dir().join("unpacked_aars");
        std::fs::create_dir_all(&maven_cache_dir)?;
        std::fs::create_dir_all(&unpack_dir)?;

        Ok(Self {
            env: Some(env),
            maven_cache_dir,
            unpack_dir,
        })
    }
}

#[cfg(test)]
impl AndroidMvnResolver<'static> {
    pub fn new_for_test(unpack_dir: PathBuf) -> Self {
        Self {
            env: None,
            maven_cache_dir: unpack_dir.join("cache"),
            unpack_dir,
        }
    }
}

impl<'a> AndroidMvnResolver<'a> {
    pub fn build_root_dependencies(
        config: &AndroidPlatformConfig,
    ) -> Result<Vec<DependencyRequirement>> {
        let mut root_deps = Vec::new();
        let target_kotlin_version = config.kotlin_version();

        for dep_str in &config.dependencies {
            let mut dep = DependencyRequirement::from_str(dep_str).with_context(|| {
                format!(
                    "Invalid dependency coordinate `{}`. Expected format `group:artifact:version`",
                    dep_str
                )
            })?;

            // Align Kotlin runtime standard libraries with the hermetic kotlinc compiler version.
            // If an unconstrained or higher version of kotlin-stdlib is requested, clamp it to
            // [kotlin_version] so kotlinc never encounters incompatible forward metadata versions.
            if dep.group.as_ref() == "org.jetbrains.kotlin"
                && (dep.name.as_ref() == "kotlin-stdlib"
                    || dep.name.as_ref() == "kotlin-stdlib-jdk7"
                    || dep.name.as_ref() == "kotlin-stdlib-jdk8"
                    || dep.name.as_ref() == "kotlin-stdlib-common")
            {
                dep.version = Some(format!("[{target_kotlin_version}]"));
            }

            root_deps.push(dep);
        }

        // Also ensure an explicit root dependency on kotlin-stdlib pinned to [kotlin_version]
        // to prevent transitive dependencies from resolving a Kotlin standard library newer
        // than the active kotlinc compiler.
        let kotlin_stdlib_coord = DependencyRequirement::from_str(&format!(
            "org.jetbrains.kotlin:kotlin-stdlib:[{target_kotlin_version}]"
        ))?;
        if !root_deps
            .iter()
            .any(|d| d.package() == kotlin_stdlib_coord.package())
        {
            root_deps.push(kotlin_stdlib_coord);
        }

        Ok(root_deps)
    }

    pub fn resolve_and_unpack(&self, config: &AndroidPlatformConfig) -> Result<ResolvedArtifacts> {
        if config.dependencies.is_empty() {
            return Ok(ResolvedArtifacts::default());
        }

        let env = self.env.ok_or_else(|| {
            anyhow::anyhow!("SessionExecutionContext required for network resolution")
        })?;
        let download_mgr = SdkToolchainFetcher::new(env)?;
        let mut maven = RepositoryClient::new(self.maven_cache_dir.clone(), download_mgr)?;

        maven.add_repository(GOOGLE_MAVEN);
        maven.add_repository(MAVEN_CENTRAL);

        for repo in &config.repositories {
            match repo {
                RepositoryEntry::Standard { standard } => match standard.as_str() {
                    "google" => maven.add_repository(GOOGLE_MAVEN),
                    "mavenCentral" => maven.add_repository(MAVEN_CENTRAL),
                    "gradlePluginPortal" => maven.add_repository(crorust_mvn::GRADLE_PLUGIN_PORTAL),
                    unknown => {
                        tracing::warn!(
                            "Unrecognized standard repository `{unknown}` in android.repositories; ignoring"
                        );
                    }
                },
                RepositoryEntry::Maven {
                    maven: url,
                    username_env,
                    password_env,
                } => {
                    if let (Some(u_var), Some(p_var)) = (username_env, password_env) {
                        let user = std::env::var(u_var).unwrap_or_default();
                        let pass = std::env::var(p_var).unwrap_or_default();
                        if !user.is_empty() || !pass.is_empty() {
                            maven.add_repository_with_auth(
                                url.clone(),
                                RepositoryHttpCredentials::Basic {
                                    username: user,
                                    password: pass,
                                },
                            );
                            continue;
                        }
                    }
                    maven.add_repository(url.clone());
                }
            }
        }

        let root_deps = Self::build_root_dependencies(config)?;

        let root_pkg = MavenCoordinates::new("crorust.internal", "root-application");
        let root_ver = SemverVersion::from_str("1.0.0")?;
        maven.add_package(root_pkg.clone(), root_ver.clone(), root_deps)?;

        let resolved_files = maven
            .resolve(root_pkg, root_ver)
            .context("Failed to resolve Android Maven dependencies")?;

        self.unpack_artifacts(&resolved_files)
    }

    fn unpack_artifacts(&self, files: &[PathBuf]) -> Result<ResolvedArtifacts> {
        let mut artifacts = ResolvedArtifacts::default();
        let mut raw_jars = Vec::new();
        let mut aars_to_unpack = Vec::new();
        let mut aar_dest_folders = Vec::new();

        for file in files {
            let file_name = file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();

            if file_name.ends_with(".jar") {
                raw_jars.push(file.clone());
            } else if file_name.ends_with(".aar") {
                let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("aar");
                let dest_folder = self.unpack_dir.join(stem);
                let complete_marker = dest_folder.join(".complete");

                if !complete_marker.exists() {
                    if dest_folder.exists() {
                        let _ = std::fs::remove_dir_all(&dest_folder);
                    }
                    aars_to_unpack.push((file.clone(), dest_folder.clone()));
                }
                aar_dest_folders.push(dest_folder);
            }
        }

        if !aars_to_unpack.is_empty() {
            ParallelAarUnpacker::unpack_all_parallel(&aars_to_unpack)?;
            for (_, dest_folder) in &aars_to_unpack {
                File::create(dest_folder.join(".complete"))?;
            }
        }

        for dest_folder in aar_dest_folders {
            let classes_jar = dest_folder.join("classes.jar");
            if classes_jar.exists() {
                artifacts.classpath_jars.push(classes_jar);
            }

            let libs_dir = dest_folder.join("libs");
            if libs_dir.exists() && libs_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&libs_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() && p.extension().is_some_and(|ext| ext == "jar") {
                            artifacts.classpath_jars.push(p);
                        }
                    }
                }
            }

            let jni_dir = dest_folder.join("jni");
            if jni_dir.exists() {
                self.collect_jni_libs(&jni_dir, &mut artifacts.native_libs)?;
            }

            let res_dir = dest_folder.join("res");
            if res_dir.exists() && res_dir.is_dir() {
                artifacts.res_dirs.push(res_dir);
            }

            let asset_dir = dest_folder.join("assets");
            if asset_dir.exists() && asset_dir.is_dir() {
                artifacts.asset_dirs.push(asset_dir);
            }

            let manifest = dest_folder.join("AndroidManifest.xml");
            if manifest.exists() {
                artifacts.manifest_files.push(manifest.clone());
            }

            let r_txt = dest_folder.join("R.txt");
            if r_txt.exists() && manifest.exists() {
                if let Ok(pkg) = Self::extract_package_from_manifest(&manifest) {
                    artifacts.r_txt_files.push((pkg, r_txt));
                }
            }
        }

        let mut seen_classes = std::collections::HashSet::new();
        for jar in &artifacts.classpath_jars {
            if let Ok(file) = File::open(jar) {
                if let Ok(mut archive) = zip::ZipArchive::new(file) {
                    for i in 0..archive.len() {
                        if let Ok(entry) = archive.by_index(i) {
                            let name = entry.name();
                            if name.ends_with(".class") && !name.ends_with("module-info.class") {
                                seen_classes.insert(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        for jar in raw_jars {
            if let Ok(file) = File::open(&jar) {
                if let Ok(mut archive) = zip::ZipArchive::new(file) {
                    let mut has_classes = false;
                    let mut has_collision = false;
                    let mut current_classes = Vec::new();

                    for i in 0..archive.len() {
                        if let Ok(entry) = archive.by_index(i) {
                            let name = entry.name();
                            if name.ends_with(".class") && !name.ends_with("module-info.class") {
                                has_classes = true;
                                if seen_classes.contains(name) {
                                    has_collision = true;
                                    break;
                                }
                                current_classes.push(name.to_string());
                            }
                        }
                    }

                    if has_classes && !has_collision {
                        seen_classes.extend(current_classes);
                        artifacts.classpath_jars.push(jar);
                    }
                }
            }
        }

        Ok(artifacts)
    }

    fn collect_jni_libs(&self, jni_dir: &Path, out: &mut Vec<(AndroidAbi, PathBuf)>) -> Result<()> {
        for entry in std::fs::read_dir(jni_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let abi_name = entry.file_name().to_string_lossy().to_string();
                let target = match abi_name.as_str() {
                    "arm64-v8a" => AndroidAbi::Arm64V8a,
                    "armeabi-v7a" => AndroidAbi::ArmV7a,
                    "x86_64" => AndroidAbi::X86_64,
                    "x86" => AndroidAbi::X86,
                    _ => continue,
                };

                for lib_entry in std::fs::read_dir(&path)? {
                    let lib_entry = lib_entry?;
                    let lib_path = lib_entry.path();
                    if lib_path.extension().is_some_and(|ext| ext == "so") {
                        out.push((target, lib_path));
                    }
                }
            }
        }
        Ok(())
    }

    fn extract_package_from_manifest(manifest_path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(manifest_path)
            .with_context(|| format!("Failed to read manifest at `{}`", manifest_path.display()))?;

        let mut reader = quick_xml::Reader::from_str(&content);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref e))
                | Ok(quick_xml::events::Event::Empty(ref e)) => {
                    if e.name().as_ref() == b"manifest" {
                        for attr in e.attributes() {
                            let attr =
                                attr.map_err(|e| anyhow::anyhow!("XML attribute error: {}", e))?;
                            if attr.key.as_ref() == b"package" {
                                let val = attr
                                    .unescape_value()
                                    .map_err(|e| anyhow::anyhow!("XML decode error: {}", e))?;
                                return Ok(val.into_owned());
                            }
                        }
                    }
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "XML error parsing `{}`: {}",
                        manifest_path.display(),
                        e
                    ))
                }
                _ => {}
            }
        }

        if let Some(start) = content.find("package=\"") {
            let rest = &content[start + 9..];
            if let Some(end) = rest.find('"') {
                return Ok(rest[..end].to_string());
            }
        }

        anyhow::bail!(
            "Could not find `package` attribute in manifest `{}`",
            manifest_path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::FileOptions;

    #[test]
    fn test_unpack_aar_and_deduplicate_jars() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let unpack_dir = temp_dir.path().join("unpacked");
        let resolver = AndroidMvnResolver::new_for_test(unpack_dir);

        let aar_path = temp_dir.path().join("library.aar");
        {
            let file = File::create(&aar_path)?;
            let mut zip = zip::ZipWriter::new(file);

            zip.start_file("AndroidManifest.xml", FileOptions::default())?;
            zip.write_all(b"<manifest package=\"com.example.lib\"></manifest>")?;

            zip.start_file("R.txt", FileOptions::default())?;
            zip.write_all(b"int string app_name 0x7f010001\n")?;

            let mut classes_jar_buf = Vec::new();
            {
                let mut inner_zip = zip::ZipWriter::new(std::io::Cursor::new(&mut classes_jar_buf));
                inner_zip.start_file("com/example/lib/Foo.class", FileOptions::default())?;
                inner_zip.write_all(b"\xca\xfe\xba\xbe")?;
                inner_zip.finish()?;
            }
            zip.start_file("classes.jar", FileOptions::default())?;
            zip.write_all(&classes_jar_buf)?;

            zip.start_file("jni/arm64-v8a/libnative.so", FileOptions::default())?;
            zip.write_all(b"\x7fELF")?;

            zip.start_file("res/values/strings.xml", FileOptions::default())?;
            zip.write_all(b"<resources></resources>")?;

            zip.finish()?;
        }

        let colliding_jar_path = temp_dir.path().join("colliding.jar");
        {
            let file = File::create(&colliding_jar_path)?;
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("com/example/lib/Foo.class", FileOptions::default())?;
            zip.write_all(b"\xca\xfe\xba\xbe")?;
            zip.finish()?;
        }

        let unique_jar_path = temp_dir.path().join("unique.jar");
        {
            let file = File::create(&unique_jar_path)?;
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("com/example/lib/Bar.class", FileOptions::default())?;
            zip.write_all(b"\xca\xfe\xba\xbe")?;
            zip.finish()?;
        }

        let artifacts =
            resolver.unpack_artifacts(&[aar_path, colliding_jar_path, unique_jar_path])?;

        assert_eq!(artifacts.manifest_files.len(), 1);
        assert_eq!(artifacts.r_txt_files.len(), 1);
        assert_eq!(artifacts.r_txt_files[0].0, "com.example.lib");
        assert_eq!(artifacts.native_libs.len(), 1);
        assert_eq!(
            artifacts.native_libs[0].0,
            crorust_android::AndroidAbi::Arm64V8a
        );
        assert_eq!(artifacts.res_dirs.len(), 1);

        assert_eq!(artifacts.classpath_jars.len(), 2);
        assert!(artifacts.classpath_jars[0].ends_with("classes.jar"));
        assert!(artifacts.classpath_jars[1].ends_with("unique.jar"));

        Ok(())
    }

    #[test]
    fn test_android_mvn_clamps_kotlin_stdlib_to_compiler_version() -> Result<()> {
        let config = AndroidPlatformConfig {
            kotlin_version: Some("2.1.0".to_string()),
            dependencies: vec![
                "org.jetbrains.kotlin:kotlin-stdlib:1.8.10".to_string(),
                "androidx.core:core-ktx:1.10.1".to_string(),
            ],
            ..Default::default()
        };

        let root_deps = AndroidMvnResolver::build_root_dependencies(&config)?;
        let kotlin_dep = root_deps
            .iter()
            .find(|d| {
                d.group.as_ref() == "org.jetbrains.kotlin" && d.name.as_ref() == "kotlin-stdlib"
            })
            .expect("kotlin-stdlib must be present");

        assert_eq!(kotlin_dep.version.as_deref(), Some("[2.1.0]"));
        Ok(())
    }

    #[test]
    fn test_android_mvn_injects_kotlin_stdlib_when_absent() -> Result<()> {
        let config = AndroidPlatformConfig {
            kotlin_version: Some("2.1.0".to_string()),
            dependencies: vec!["androidx.core:core-ktx:1.10.1".to_string()],
            ..Default::default()
        };

        let root_deps = AndroidMvnResolver::build_root_dependencies(&config)?;
        let kotlin_dep = root_deps
            .iter()
            .find(|d| {
                d.group.as_ref() == "org.jetbrains.kotlin" && d.name.as_ref() == "kotlin-stdlib"
            })
            .expect("kotlin-stdlib must be injected");

        assert_eq!(kotlin_dep.version.as_deref(), Some("[2.1.0]"));
        Ok(())
    }
}
