use crate::android::mvn::ResolvedArtifacts;
use crate::config::AndroidPlatformConfig;
use crate::core::env::SessionExecutionContext;
use crate::task;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct AndroidBytecodeCompiler<'a> {
    env: &'a SessionExecutionContext,
    classes_out_dir: PathBuf,
    dex_out_dir: PathBuf,
}

impl<'a> AndroidBytecodeCompiler<'a> {
    pub fn new(env: &'a SessionExecutionContext) -> Result<Self> {
        let classes_out_dir = env.target_platform_dir().join("kotlin_classes");
        let dex_out_dir = env.target_platform_dir().join("dex");

        std::fs::create_dir_all(&classes_out_dir)?;
        std::fs::create_dir_all(&dex_out_dir)?;

        Ok(Self {
            env,
            classes_out_dir,
            dex_out_dir,
        })
    }

    pub fn compile(
        &self,
        config: &AndroidPlatformConfig,
        artifacts: &ResolvedArtifacts,
    ) -> Result<(Vec<PathBuf>, Option<crorust_android::ResourceSymbols>)> {
        let kotlin_src_dir = self.env.cargo_session().package_root_path().join("kotlin");
        let mut r_concat = String::new();
        for (_pkg, r_txt) in &artifacts.r_txt_files {
            if let Ok(content) = std::fs::read_to_string(r_txt) {
                r_concat.push_str(&content);
            }
        }
        let min_sdk = config
            .min_sdk
            .or(config.manifest.sdk.min_sdk_version)
            .unwrap_or(23);
        let desugar_enabled = min_sdk < 26;
        let desugar_rules_hash = if desugar_enabled {
            crate::download::catalog::ToolCatalog::get_desugar_jdk_libs_config("2.1.4")
                .map(|a| a.sha256.to_string())
        } else {
            None
        };
        let stamp = crate::packaging_cache::AndroidBytecodeStamp::compute(
            std::slice::from_ref(&kotlin_src_dir),
            &config.dependencies,
            &r_concat,
            min_sdk,
            config.manifest.sdk.target_sdk_version.unwrap_or(34),
            desugar_enabled,
            desugar_rules_hash,
        )?;
        let current_fp = stamp.fingerprint();
        let stamp_file = self.dex_out_dir.join(".dex_stamp");

        if stamp_file.is_file() && self.dex_out_dir.join("classes.dex").is_file() {
            if let Ok(saved_fp) = std::fs::read_to_string(&stamp_file) {
                if saved_fp.trim() == current_fp {
                    let mut cached_dex = Vec::new();
                    for entry in std::fs::read_dir(&self.dex_out_dir)? {
                        let path = entry?.path();
                        if path.is_file() && path.extension().is_some_and(|e| e == "dex") {
                            cached_dex.push(path);
                        }
                    }
                    cached_dex.sort();
                    if !cached_dex.is_empty() {
                        let symbols = if !artifacts.r_txt_files.is_empty() {
                            let generator = RClassGenerator::parse(&artifacts.r_txt_files)?;
                            Some(generator.export_resource_symbols())
                        } else {
                            None
                        };
                        tracing::debug!(
                            "AndroidBytecodeCompiler cache hit ({}); reusing {} DEX file(s)",
                            &current_fp[..8],
                            cached_dex.len()
                        );
                        return Ok((cached_dex, symbols));
                    }
                }
            }
        }

        if self.classes_out_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.classes_out_dir);
        }
        std::fs::create_dir_all(&self.classes_out_dir)?;

        if self.dex_out_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.dex_out_dir);
        }
        std::fs::create_dir_all(&self.dex_out_dir)?;

        let root_package = config.manifest.package.as_deref().unwrap_or("app");

        let symbols = if !artifacts.r_txt_files.is_empty() {
            Some(self.generate_and_compile_r_classes(artifacts, root_package)?)
        } else {
            None
        };

        let has_user_kotlin = kotlin_src_dir.exists()
            && kotlin_src_dir.read_dir()?.filter_map(|e| e.ok()).any(|e| {
                let p = e.path();
                p.is_file()
                    && p.extension()
                        .is_some_and(|ext| ext == "kt" || ext == "java")
            });

        if has_user_kotlin {
            self.compile_kotlin(&kotlin_src_dir, config, artifacts)?;
        }

        let dex_files = self.compile_dex(config, artifacts)?;
        let _ = std::fs::write(&stamp_file, &current_fp);
        Ok((dex_files, symbols))
    }

    pub fn generate_and_compile_r_classes(
        &self,
        artifacts: &ResolvedArtifacts,
        root_package: &str,
    ) -> Result<crorust_android::ResourceSymbols> {
        let mut generator = RClassGenerator::parse(&artifacts.r_txt_files)?;
        let symbols = generator.export_resource_symbols();

        generator.emit_bytecode(&self.classes_out_dir, root_package)?;
        tracing::info!("Synthesized R.class bytecode in pure Rust (bypassing javac)");

        Ok(symbols)
    }

    fn locate_jdk(&self) -> Result<PathBuf> {
        let host_platform = crate::TargetPlatform::detect_host()?;
        let host_arch = crate::TargetArch::detect_host()?;

        if let Some(local_jdk) = self.env.locate_java_jdk() {
            Ok(local_jdk)
        } else if let Some(jdk_artifact) =
            crate::download::catalog::ToolCatalog::get_jdk(host_platform, host_arch)
        {
            let tool_mgr = crate::download::manager::ToolManager::new(self.env)?;
            tool_mgr.ensure_tool(&jdk_artifact)
        } else {
            anyhow::bail!("No compatible Java JDK 17 found on host or available in ToolCatalog");
        }
    }

    fn synthesize_java_env(&self, cmd: &mut Command) -> Result<()> {
        let jdk_dir = self.locate_jdk()?;
        let sdk_dir = self.env.locate_android_sdk();

        let mut synth = crate::download::env::EnvSynthesizer::new();
        synth
            .prepend_path(jdk_dir.join("bin"))
            .set_var("JAVA_HOME", &jdk_dir)
            .set_var("ANDROID_HOME", &sdk_dir)
            .set_var("ANDROID_SDK_ROOT", &sdk_dir);

        synth.apply(cmd)?;
        Ok(())
    }

    fn compile_kotlin(
        &self,
        kotlin_src_dir: &Path,
        config: &AndroidPlatformConfig,
        artifacts: &ResolvedArtifacts,
    ) -> Result<()> {
        let kotlinc_bin = self.locate_kotlinc(config)?;
        let android_jar = self.env.resolve_android_jar();

        let mut classpath_elements = vec![android_jar];
        if self.classes_out_dir.exists() {
            classpath_elements.push(self.classes_out_dir.clone());
        }
        classpath_elements.extend(artifacts.classpath_jars.clone());

        let sep = if cfg!(target_os = "windows") {
            ";"
        } else {
            ":"
        };
        let classpath_str = classpath_elements
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(sep);

        let mut source_files = Vec::new();
        for entry in walkdir::WalkDir::new(kotlin_src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "kt" || ext == "java")
            {
                source_files.push(entry.into_path());
            }
        }

        if source_files.is_empty() {
            return Ok(());
        }

        let jvm_target = config.jvm_target();

        // Write compiler arguments and source files into a response file (@argfile).
        // This avoids Windows cmd.exe /C quote-stripping issues that corrupt semicolons in -cp,
        // and avoids the Windows 8,191-character command line buffer limit.
        let argfile_path = self.classes_out_dir.join("kotlinc_args.txt");
        let mut argfile_content = String::new();
        argfile_content.push_str("-jvm-target\n");
        argfile_content.push_str(jvm_target);
        argfile_content.push('\n');
        argfile_content.push_str("-cp\n");
        argfile_content.push_str(&classpath_str);
        argfile_content.push('\n');
        argfile_content.push_str("-d\n");
        argfile_content.push_str(&crorust_common::sanitize_path(&self.classes_out_dir));
        argfile_content.push('\n');

        for src in source_files {
            argfile_content.push_str(&crorust_common::sanitize_path(&src));
            argfile_content.push('\n');
        }

        std::fs::write(&argfile_path, &argfile_content)
            .context("Failed writing kotlinc argument file")?;

        let mut cmd = crorust_common::create_platform_command(&kotlinc_bin);
        self.synthesize_java_env(&mut cmd)?;
        cmd.arg(format!("@{}", argfile_path.display()));

        let run_result =
            task::run(&mut cmd).context("Failed while compiling Kotlin source files with kotlinc");
        let _ = std::fs::remove_file(&argfile_path);
        run_result?;
        Ok(())
    }

    fn ensure_desugar_tools(&self) -> Result<(PathBuf, PathBuf)> {
        let tool_mgr = crate::download::manager::ToolManager::new(self.env)?;

        let config_artifact =
            crate::download::catalog::ToolCatalog::get_desugar_jdk_libs_config("2.1.4")
                .context("desugar_jdk_libs_configuration 2.1.4 not found in catalog")?;
        let config_dir = tool_mgr.ensure_tool(&config_artifact)?;
        let desugar_json = config_dir
            .join("META-INF")
            .join("desugar")
            .join("d8")
            .join("desugar.json");

        anyhow::ensure!(
            desugar_json.is_file(),
            "Expected desugar.json at `{}` but file was missing from extracted configuration archive",
            desugar_json.display()
        );

        let libs_artifact = crate::download::catalog::ToolCatalog::get_desugar_jdk_libs("2.1.4")
            .context("desugar_jdk_libs 2.1.4 not found in catalog")?;
        let libs_dir = tool_mgr.ensure_tool(&libs_artifact)?;
        let desugar_jdk_libs_jar = libs_dir.join("desugar_jdk_libs.jar");

        anyhow::ensure!(
            desugar_jdk_libs_jar.is_file(),
            "Expected desugar_jdk_libs.jar at `{}` but file was missing",
            desugar_jdk_libs_jar.display()
        );

        Ok((desugar_json, desugar_jdk_libs_jar))
    }

    fn compile_dex(
        &self,
        config: &AndroidPlatformConfig,
        artifacts: &ResolvedArtifacts,
    ) -> Result<Vec<PathBuf>> {
        let android_jar = self.env.resolve_android_jar();
        let min_sdk_val = config
            .min_sdk
            .or(config.manifest.sdk.min_sdk_version)
            .unwrap_or(23);
        let min_sdk = min_sdk_val.to_string();

        let desugar_tools = if min_sdk_val < 26 {
            Some(self.ensure_desugar_tools()?)
        } else {
            None
        };

        let (r8_jar, use_r8_jar) = if desugar_tools.is_some() {
            let artifact = crate::download::catalog::ToolCatalog::get_r8("9.5.26")
                .context("r8 9.5.26 not found in catalog for standalone desugaring")?;
            let tool_mgr = crate::download::manager::ToolManager::new(self.env)?;
            let r8_dir = tool_mgr.ensure_tool(&artifact)?;
            (r8_dir.join("r8.jar"), true)
        } else if let Some(artifact) = crate::download::catalog::ToolCatalog::get_r8("9.5.26") {
            let tool_mgr = crate::download::manager::ToolManager::new(self.env)?;
            if let Ok(r8_dir) = tool_mgr.ensure_tool(&artifact) {
                (r8_dir.join("r8.jar"), true)
            } else {
                (PathBuf::new(), false)
            }
        } else {
            (PathBuf::new(), false)
        };

        let mut cmd = if use_r8_jar {
            let mut c = Command::new("java");
            self.synthesize_java_env(&mut c)?;
            c.arg("-Xmx2G");
            c.arg("-cp").arg(&r8_jar).arg("com.android.tools.r8.D8");
            c
        } else {
            let d8_bin = self.env.locate_d8()?;
            let mut c = crorust_common::create_platform_command(&d8_bin);
            self.synthesize_java_env(&mut c)?;
            c
        };

        cmd.arg("--lib").arg(&android_jar);
        cmd.arg("--min-api").arg(&min_sdk);
        cmd.arg("--output").arg(&self.dex_out_dir);

        // Remap missing types required only for interface desugaring from warnings to none.
        // Multiplatform libraries (such as coroutines AgentPremain or optional ASM in commons-compress)
        // contain desktop JVM code paths that reference classes that do not exist on Android.
        cmd.arg("--map-diagnostics:InterfaceDesugarMissingTypeDiagnostic")
            .arg("warning")
            .arg("none");

        if let Some((ref desugar_json, _)) = desugar_tools {
            cmd.arg("--desugared-lib").arg(desugar_json);
        }

        for entry in walkdir::WalkDir::new(&self.classes_out_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "class") {
                cmd.arg(path);
            }
        }

        for jar in &artifacts.classpath_jars {
            cmd.arg(jar);
        }

        task::run(&mut cmd).context("Failed while compiling DEX bytecode with d8")?;

        if let Some((ref desugar_json, ref desugar_jar)) = desugar_tools {
            let desugar_temp = self.env.target_platform_dir().join("desugar_dex");
            if desugar_temp.exists() {
                let _ = std::fs::remove_dir_all(&desugar_temp);
            }
            std::fs::create_dir_all(&desugar_temp)?;

            let mut l8_cmd = Command::new("java");
            self.synthesize_java_env(&mut l8_cmd)?;
            l8_cmd.arg("-Xmx2G");
            l8_cmd
                .arg("-cp")
                .arg(&r8_jar)
                .arg("com.android.tools.r8.L8");

            l8_cmd.arg("--lib").arg(&android_jar);
            l8_cmd.arg("--min-api").arg(&min_sdk);
            l8_cmd.arg("--desugared-lib").arg(desugar_json);
            l8_cmd.arg("--output").arg(&desugar_temp);
            l8_cmd.arg(desugar_jar);

            task::run(&mut l8_cmd)
                .context("Failed while compiling desugared library bytecode with L8")?;

            let mut existing_dex_count = std::fs::read_dir(&self.dex_out_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "dex"))
                .count();

            let mut desugar_entries = Vec::new();
            for entry in std::fs::read_dir(&desugar_temp)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "dex") {
                    desugar_entries.push(path);
                }
            }
            desugar_entries.sort();

            for desugar_dex in desugar_entries {
                existing_dex_count += 1;
                let target_name = format!("classes{}.dex", existing_dex_count);
                std::fs::copy(&desugar_dex, self.dex_out_dir.join(target_name))?;
            }
        }

        let mut dex_files = Vec::new();
        for entry in std::fs::read_dir(&self.dex_out_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "dex") {
                dex_files.push(path);
            }
        }
        dex_files.sort();

        anyhow::ensure!(
            !dex_files.is_empty(),
            "d8 completed successfully but no `.dex` files were found in `{}`",
            self.dex_out_dir.display()
        );

        Ok(dex_files)
    }

    fn locate_kotlinc(&self, config: &AndroidPlatformConfig) -> Result<PathBuf> {
        if let Ok(bin) = which::which(crorust_common::host_executable_name("kotlinc")) {
            return Ok(bin);
        }

        let kotlin_ver = config.kotlin_version.as_deref().unwrap_or("2.1.0");
        if let Some(artifact) = crate::download::catalog::ToolCatalog::get_kotlinc(kotlin_ver) {
            let tool_mgr = crate::download::manager::ToolManager::new(self.env)?;
            let root = tool_mgr.ensure_tool(&artifact)?;
            let bin = if cfg!(target_os = "windows") {
                root.join("bin").join("kotlinc.bat")
            } else {
                root.join("bin").join("kotlinc")
            };
            if bin.exists() {
                return Ok(bin);
            }
        }

        anyhow::bail!(
            "kotlinc not found on system PATH and not available in ToolCatalog for version `{}`",
            kotlin_ver
        );
    }
}

struct ConstantPool {
    entries: Vec<Vec<u8>>,
}

impl ConstantPool {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn count(&self) -> u16 {
        (self.entries.len() + 1) as u16
    }

    fn add_utf8(&mut self, s: &str) -> u16 {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.len() >= 3 && entry[0] == 1 {
                let len = u16::from_be_bytes([entry[1], entry[2]]) as usize;
                if len == s.len() && &entry[3..3 + len] == s.as_bytes() {
                    return (i + 1) as u16;
                }
            }
        }
        let bytes = s.as_bytes();
        let mut entry = Vec::with_capacity(3 + bytes.len());
        entry.push(1u8); // CONSTANT_Utf8
        entry.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        entry.extend_from_slice(bytes);
        self.entries.push(entry);
        self.entries.len() as u16
    }

    fn add_class(&mut self, name: &str) -> u16 {
        let name_idx = self.add_utf8(name);
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.len() == 3 && entry[0] == 7 {
                let idx = u16::from_be_bytes([entry[1], entry[2]]);
                if idx == name_idx {
                    return (i + 1) as u16;
                }
            }
        }
        let mut entry = Vec::with_capacity(3);
        entry.push(7u8); // CONSTANT_Class
        entry.extend_from_slice(&name_idx.to_be_bytes());
        self.entries.push(entry);
        self.entries.len() as u16
    }

    fn add_integer(&mut self, val: i32) -> u16 {
        let val_bytes = val.to_be_bytes();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.len() == 5 && entry[0] == 3 && entry[1..5] == val_bytes {
                return (i + 1) as u16;
            }
        }
        let mut entry = Vec::with_capacity(5);
        entry.push(3u8); // CONSTANT_Integer
        entry.extend_from_slice(&val_bytes);
        self.entries.push(entry);
        self.entries.len() as u16
    }

    fn add_name_and_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let name_idx = self.add_utf8(name);
        let desc_idx = self.add_utf8(descriptor);
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.len() == 5 && entry[0] == 12 {
                let n = u16::from_be_bytes([entry[1], entry[2]]);
                let d = u16::from_be_bytes([entry[3], entry[4]]);
                if n == name_idx && d == desc_idx {
                    return (i + 1) as u16;
                }
            }
        }
        let mut entry = Vec::with_capacity(5);
        entry.push(12u8); // CONSTANT_NameAndType
        entry.extend_from_slice(&name_idx.to_be_bytes());
        entry.extend_from_slice(&desc_idx.to_be_bytes());
        self.entries.push(entry);
        self.entries.len() as u16
    }

    fn add_methodref(&mut self, class_name: &str, name: &str, descriptor: &str) -> u16 {
        let class_idx = self.add_class(class_name);
        let nt_idx = self.add_name_and_type(name, descriptor);
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.len() == 5 && entry[0] == 10 {
                let c = u16::from_be_bytes([entry[1], entry[2]]);
                let nt = u16::from_be_bytes([entry[3], entry[4]]);
                if c == class_idx && nt == nt_idx {
                    return (i + 1) as u16;
                }
            }
        }
        let mut entry = Vec::with_capacity(5);
        entry.push(10u8); // CONSTANT_Methodref
        entry.extend_from_slice(&class_idx.to_be_bytes());
        entry.extend_from_slice(&nt_idx.to_be_bytes());
        self.entries.push(entry);
        self.entries.len() as u16
    }

    fn add_fieldref(&mut self, class_name: &str, name: &str, descriptor: &str) -> u16 {
        let class_idx = self.add_class(class_name);
        let nt_idx = self.add_name_and_type(name, descriptor);
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.len() == 5 && entry[0] == 9 {
                let c = u16::from_be_bytes([entry[1], entry[2]]);
                let nt = u16::from_be_bytes([entry[3], entry[4]]);
                if c == class_idx && nt == nt_idx {
                    return (i + 1) as u16;
                }
            }
        }
        let mut entry = Vec::with_capacity(5);
        entry.push(9u8); // CONSTANT_Fieldref
        entry.extend_from_slice(&class_idx.to_be_bytes());
        entry.extend_from_slice(&nt_idx.to_be_bytes());
        self.entries.push(entry);
        self.entries.len() as u16
    }
}

fn push_int(code: &mut Vec<u8>, cp: &mut ConstantPool, val: i32) {
    if (-1..=5).contains(&val) {
        code.push((0x03 + val) as u8);
    } else if (-128..=127).contains(&val) {
        code.push(0x10u8); // bipush
        code.push(val as u8);
    } else if (-32768..=32767).contains(&val) {
        code.push(0x11u8); // sipush
        code.extend_from_slice(&(val as i16).to_be_bytes());
    } else {
        let idx = cp.add_integer(val);
        if idx <= 255 {
            code.push(0x12u8); // ldc
            code.push(idx as u8);
        } else {
            code.push(0x13u8); // ldc_w
            code.extend_from_slice(&idx.to_be_bytes());
        }
    }
}

pub struct RClassBytecodeWriter;

impl RClassBytecodeWriter {
    pub fn emit_empty_class(internal_class_name: &str) -> Vec<u8> {
        let mut cp = ConstantPool::new();
        let this_class = cp.add_class(internal_class_name);
        let super_class = cp.add_class("java/lang/Object");
        let init_methodref = cp.add_methodref("java/lang/Object", "<init>", "()V");
        let code_attr_name = cp.add_utf8("Code");
        let init_name = cp.add_utf8("<init>");
        let init_sig = cp.add_utf8("()V");

        let mut out = Vec::new();
        out.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x34]); // Java 8

        out.extend_from_slice(&cp.count().to_be_bytes());
        for entry in &cp.entries {
            out.extend_from_slice(entry);
        }

        out.extend_from_slice(&0x0031u16.to_be_bytes()); // ACC_PUBLIC | ACC_FINAL | ACC_SUPER
        out.extend_from_slice(&this_class.to_be_bytes());
        out.extend_from_slice(&super_class.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes()); // interfaces_count = 0
        out.extend_from_slice(&0u16.to_be_bytes()); // fields_count = 0

        // Methods count = 1 (<init>)
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&0x0001u16.to_be_bytes()); // ACC_PUBLIC
        out.extend_from_slice(&init_name.to_be_bytes());
        out.extend_from_slice(&init_sig.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes()); // attributes_count = 1

        out.extend_from_slice(&code_attr_name.to_be_bytes());
        let code_length = 5u32;
        let attr_length = 2 + 2 + 4 + code_length + 2 + 2;
        out.extend_from_slice(&attr_length.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes()); // max_stack = 1
        out.extend_from_slice(&1u16.to_be_bytes()); // max_locals = 1
        out.extend_from_slice(&code_length.to_be_bytes());
        out.push(0x2Au8); // aload_0
        out.push(0xB7u8); // invokespecial
        out.extend_from_slice(&init_methodref.to_be_bytes());
        out.push(0xB1u8); // return
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());

        out.extend_from_slice(&0u16.to_be_bytes()); // class attributes_count = 0
        out
    }

    pub fn emit_int_fields_class(internal_class_name: &str, fields: &[(String, u32)]) -> Vec<u8> {
        let mut cp = ConstantPool::new();
        let this_class = cp.add_class(internal_class_name);
        let super_class = cp.add_class("java/lang/Object");
        let init_methodref = cp.add_methodref("java/lang/Object", "<init>", "()V");
        let code_attr_name = cp.add_utf8("Code");
        let init_name = cp.add_utf8("<init>");
        let init_sig = cp.add_utf8("()V");
        let int_sig = cp.add_utf8("I");
        let const_val_attr_name = cp.add_utf8("ConstantValue");

        let mut field_indices = Vec::with_capacity(fields.len());
        for (name, val) in fields {
            let name_idx = cp.add_utf8(name);
            let val_idx = cp.add_integer(*val as i32);
            field_indices.push((name_idx, val_idx));
        }

        let mut out = Vec::new();
        out.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x34]);

        out.extend_from_slice(&cp.count().to_be_bytes());
        for entry in &cp.entries {
            out.extend_from_slice(entry);
        }

        out.extend_from_slice(&0x0031u16.to_be_bytes()); // ACC_PUBLIC | ACC_FINAL | ACC_SUPER
        out.extend_from_slice(&this_class.to_be_bytes());
        out.extend_from_slice(&super_class.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());

        out.extend_from_slice(&(field_indices.len() as u16).to_be_bytes());
        for (name_idx, val_idx) in field_indices {
            out.extend_from_slice(&0x0009u16.to_be_bytes()); // ACC_PUBLIC | ACC_STATIC
            out.extend_from_slice(&name_idx.to_be_bytes());
            out.extend_from_slice(&int_sig.to_be_bytes());
            out.extend_from_slice(&1u16.to_be_bytes());
            out.extend_from_slice(&const_val_attr_name.to_be_bytes());
            out.extend_from_slice(&2u32.to_be_bytes());
            out.extend_from_slice(&val_idx.to_be_bytes());
        }

        // Methods count = 1 (<init>)
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&0x0001u16.to_be_bytes());
        out.extend_from_slice(&init_name.to_be_bytes());
        out.extend_from_slice(&init_sig.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());

        out.extend_from_slice(&code_attr_name.to_be_bytes());
        let code_length = 5u32;
        let attr_length = 2 + 2 + 4 + code_length + 2 + 2;
        out.extend_from_slice(&attr_length.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&code_length.to_be_bytes());
        out.push(0x2Au8); // aload_0
        out.push(0xB7u8); // invokespecial
        out.extend_from_slice(&init_methodref.to_be_bytes());
        out.push(0xB1u8); // return
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());

        out.extend_from_slice(&0u16.to_be_bytes());
        out
    }

    pub fn emit_styleable_class(
        internal_class_name: &str,
        int_fields: &[(String, u32)],
        array_fields: &[(String, Vec<u32>)],
    ) -> Vec<u8> {
        let mut cp = ConstantPool::new();
        let this_class = cp.add_class(internal_class_name);
        let super_class = cp.add_class("java/lang/Object");
        let init_methodref = cp.add_methodref("java/lang/Object", "<init>", "()V");
        let code_attr_name = cp.add_utf8("Code");
        let init_name = cp.add_utf8("<init>");
        let clinit_name = cp.add_utf8("<clinit>");
        let void_sig = cp.add_utf8("()V");
        let int_sig = cp.add_utf8("I");
        let array_sig = cp.add_utf8("[I");
        let const_val_attr_name = cp.add_utf8("ConstantValue");

        let mut int_field_indices = Vec::with_capacity(int_fields.len());
        for (name, val) in int_fields {
            let name_idx = cp.add_utf8(name);
            let val_idx = cp.add_integer(*val as i32);
            int_field_indices.push((name_idx, val_idx));
        }

        let mut array_field_indices = Vec::with_capacity(array_fields.len());
        for (name, vals) in array_fields {
            let name_idx = cp.add_utf8(name);
            let fieldref = cp.add_fieldref(internal_class_name, name, "[I");
            array_field_indices.push((name_idx, fieldref, vals.clone()));
        }

        let mut clinit_bytecode = Vec::new();
        if !array_fields.is_empty() {
            for (_, fieldref, vals) in &array_field_indices {
                push_int(&mut clinit_bytecode, &mut cp, vals.len() as i32);
                clinit_bytecode.push(0xBCu8); // newarray
                clinit_bytecode.push(10u8); // T_INT
                for (i, val) in vals.iter().enumerate() {
                    clinit_bytecode.push(0x59u8); // dup
                    push_int(&mut clinit_bytecode, &mut cp, i as i32);
                    push_int(&mut clinit_bytecode, &mut cp, *val as i32);
                    clinit_bytecode.push(0x4Fu8); // iastore
                }
                clinit_bytecode.push(0xB3u8); // putstatic
                clinit_bytecode.extend_from_slice(&fieldref.to_be_bytes());
            }
            clinit_bytecode.push(0xB1u8); // return
        }

        let mut out = Vec::new();
        out.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        out.extend_from_slice(&[0x00, 0x00, 0x00, 0x34]);

        out.extend_from_slice(&cp.count().to_be_bytes());
        for entry in &cp.entries {
            out.extend_from_slice(entry);
        }

        out.extend_from_slice(&0x0031u16.to_be_bytes()); // ACC_PUBLIC | ACC_FINAL | ACC_SUPER
        out.extend_from_slice(&this_class.to_be_bytes());
        out.extend_from_slice(&super_class.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());

        let total_fields = (int_field_indices.len() + array_field_indices.len()) as u16;
        out.extend_from_slice(&total_fields.to_be_bytes());

        for (name_idx, val_idx) in int_field_indices {
            out.extend_from_slice(&0x0009u16.to_be_bytes());
            out.extend_from_slice(&name_idx.to_be_bytes());
            out.extend_from_slice(&int_sig.to_be_bytes());
            out.extend_from_slice(&1u16.to_be_bytes());
            out.extend_from_slice(&const_val_attr_name.to_be_bytes());
            out.extend_from_slice(&2u32.to_be_bytes());
            out.extend_from_slice(&val_idx.to_be_bytes());
        }

        for (name_idx, _, _) in &array_field_indices {
            out.extend_from_slice(&0x0009u16.to_be_bytes());
            out.extend_from_slice(&name_idx.to_be_bytes());
            out.extend_from_slice(&array_sig.to_be_bytes());
            out.extend_from_slice(&0u16.to_be_bytes());
        }

        let methods_count = if array_fields.is_empty() { 1u16 } else { 2u16 };
        out.extend_from_slice(&methods_count.to_be_bytes());

        // <init>()V
        out.extend_from_slice(&0x0001u16.to_be_bytes());
        out.extend_from_slice(&init_name.to_be_bytes());
        out.extend_from_slice(&void_sig.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());

        out.extend_from_slice(&code_attr_name.to_be_bytes());
        let init_code_len = 5u32;
        let init_attr_len = 2 + 2 + 4 + init_code_len + 2 + 2;
        out.extend_from_slice(&init_attr_len.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&init_code_len.to_be_bytes());
        out.push(0x2Au8); // aload_0
        out.push(0xB7u8); // invokespecial
        out.extend_from_slice(&init_methodref.to_be_bytes());
        out.push(0xB1u8); // return
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());

        // <clinit>()V
        if !array_fields.is_empty() {
            out.extend_from_slice(&0x0008u16.to_be_bytes()); // ACC_STATIC
            out.extend_from_slice(&clinit_name.to_be_bytes());
            out.extend_from_slice(&void_sig.to_be_bytes());
            out.extend_from_slice(&1u16.to_be_bytes());

            out.extend_from_slice(&code_attr_name.to_be_bytes());
            let clinit_code_len = clinit_bytecode.len() as u32;
            let clinit_attr_len = 2 + 2 + 4 + clinit_code_len + 2 + 2;
            out.extend_from_slice(&clinit_attr_len.to_be_bytes());
            out.extend_from_slice(&4u16.to_be_bytes()); // max_stack = 4
            out.extend_from_slice(&0u16.to_be_bytes()); // max_locals = 0
            out.extend_from_slice(&clinit_code_len.to_be_bytes());
            out.extend_from_slice(&clinit_bytecode);
            out.extend_from_slice(&0u16.to_be_bytes());
            out.extend_from_slice(&0u16.to_be_bytes());
        }

        out.extend_from_slice(&0u16.to_be_bytes());
        out
    }
}

pub struct RClassGenerator {
    all_entries: BTreeMap<String, BTreeSet<String>>,
    styleable_arrays: BTreeMap<String, Vec<u32>>,
    styleable_indices: BTreeMap<String, BTreeMap<String, u32>>,
    pkg_entries: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    pkg_styleable_arrays: BTreeMap<String, BTreeSet<String>>,
    pkg_styleable_indices: BTreeMap<String, BTreeMap<String, u32>>,
}

impl RClassGenerator {
    pub fn parse(r_txt_files: &[(String, PathBuf)]) -> Result<Self> {
        let mut all_entries: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut styleable_arrays: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        let mut styleable_indices: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut pkg_entries: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
        let mut pkg_styleable_arrays: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut pkg_styleable_indices: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();

        for (pkg, r_txt_path) in r_txt_files {
            let content = match std::fs::read_to_string(r_txt_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Failed reading R.txt at `{}`: {}", r_txt_path.display(), e);
                    continue;
                }
            };

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                if let Some(rest) = trimmed.strip_prefix("int[] styleable ") {
                    if let Some((name, vals_str)) = rest.split_once('{') {
                        let name = name.trim();
                        let vals_str = vals_str.trim().trim_end_matches('}').trim();
                        let mut values = Vec::new();
                        if !vals_str.is_empty() {
                            for part in vals_str.split(',') {
                                let part = part.trim();
                                if part.is_empty() {
                                    continue;
                                }
                                let val = if let Some(hex) = part.strip_prefix("0x") {
                                    u32::from_str_radix(hex, 16).unwrap_or(0)
                                } else {
                                    part.parse::<u32>().unwrap_or(0)
                                };
                                values.push(val);
                            }
                        }
                        styleable_arrays.entry(name.to_string()).or_insert(values);
                        pkg_styleable_arrays
                            .entry(pkg.clone())
                            .or_default()
                            .insert(name.to_string());
                    }
                } else if let Some(rest) = trimmed.strip_prefix("int styleable ") {
                    let mut parts = rest.split_whitespace();
                    if let (Some(field_name), Some(idx_str)) = (parts.next(), parts.next()) {
                        let idx = if let Some(hex) = idx_str.strip_prefix("0x") {
                            u32::from_str_radix(hex, 16).unwrap_or(0)
                        } else {
                            idx_str.parse::<u32>().unwrap_or(0)
                        };
                        let styleable_base = if let Some((base, _)) = field_name.split_once('_') {
                            base.to_string()
                        } else {
                            field_name.to_string()
                        };
                        styleable_indices
                            .entry(styleable_base)
                            .or_default()
                            .insert(field_name.to_string(), idx);
                        pkg_styleable_indices
                            .entry(pkg.clone())
                            .or_default()
                            .insert(field_name.to_string(), idx);
                    }
                } else if let Some(rest) = trimmed.strip_prefix("int ") {
                    let mut parts = rest.split_whitespace();
                    if let (Some(res_type), Some(name), Some(_val)) =
                        (parts.next(), parts.next(), parts.next())
                    {
                        all_entries
                            .entry(res_type.to_string())
                            .or_default()
                            .insert(name.to_string());
                        pkg_entries
                            .entry(pkg.clone())
                            .or_default()
                            .entry(res_type.to_string())
                            .or_default()
                            .insert(name.to_string());
                    }
                }
            }
        }

        Ok(Self {
            all_entries,
            styleable_arrays,
            styleable_indices,
            pkg_entries,
            pkg_styleable_arrays,
            pkg_styleable_indices,
        })
    }

    pub fn assign_resource_ids(&self) -> BTreeMap<(String, String), u32> {
        let mut id_map = BTreeMap::new();

        for (type_idx, (res_type, names)) in (1_u32..).zip(self.all_entries.iter()) {
            for (entry_idx, name) in (1_u32..).zip(names.iter()) {
                let id = (0x7f << 24) | (type_idx << 16) | entry_idx;
                id_map.insert((res_type.clone(), name.clone()), id);
            }
        }

        id_map
    }

    pub fn export_resource_symbols(&self) -> crorust_android::ResourceSymbols {
        let types: Vec<String> = self.all_entries.keys().cloned().collect();
        let id_map = self.assign_resource_ids();
        crorust_android::ResourceSymbols {
            types,
            id_map,
            string_values: BTreeMap::new(),
            file_resources: BTreeMap::new(),
        }
    }

    pub fn resolve_styleables(&mut self, id_map: &BTreeMap<(String, String), u32>) {
        for (styleable_name, values) in &mut self.styleable_arrays {
            if let Some(indices) = self.styleable_indices.get(styleable_name) {
                for (field_name, idx) in indices {
                    let idx = *idx as usize;
                    if idx < values.len() && values[idx] == 0 {
                        if let Some(suffix) =
                            field_name.strip_prefix(&format!("{}_", styleable_name))
                        {
                            if !suffix.starts_with("android_") {
                                if let Some(attr_id) =
                                    id_map.get(&("attr".to_string(), suffix.to_string()))
                                {
                                    values[idx] = *attr_id;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn write_sources(&mut self, out_dir: &Path, root_package: &str) -> Result<Vec<PathBuf>> {
        let id_map = self.assign_resource_ids();
        self.resolve_styleables(&id_map);

        let mut java_files = Vec::new();

        for (pkg, types) in &self.pkg_entries {
            let file_path = self.write_single_r_java(
                out_dir,
                pkg,
                types,
                self.pkg_styleable_arrays.get(pkg),
                self.pkg_styleable_indices.get(pkg),
                &id_map,
            )?;
            java_files.push(file_path);
        }

        if !self.pkg_entries.contains_key(root_package) {
            let all_arrays: BTreeSet<String> = self.styleable_arrays.keys().cloned().collect();
            let mut all_indices = BTreeMap::new();
            for indices in self.styleable_indices.values() {
                for (k, v) in indices {
                    all_indices.insert(k.clone(), *v);
                }
            }

            let file_path = self.write_single_r_java(
                out_dir,
                root_package,
                &self.all_entries,
                Some(&all_arrays),
                Some(&all_indices),
                &id_map,
            )?;
            java_files.push(file_path);
        }

        Ok(java_files)
    }

    fn write_single_r_java(
        &self,
        out_dir: &Path,
        pkg: &str,
        types: &BTreeMap<String, BTreeSet<String>>,
        styleable_arrays: Option<&BTreeSet<String>>,
        styleable_indices: Option<&BTreeMap<String, u32>>,
        id_map: &BTreeMap<(String, String), u32>,
    ) -> Result<PathBuf> {
        let pkg_rel = pkg.replace('.', "/");
        let pkg_dir = out_dir.join(&pkg_rel);
        std::fs::create_dir_all(&pkg_dir)?;
        let r_file = pkg_dir.join("R.java");

        let mut s = String::new();
        s.push_str(&format!("package {};\n\n", pkg));
        s.push_str("public final class R {\n");

        for (res_type, names) in types {
            s.push_str(&format!("    public static final class {} {{\n", res_type));
            for name in names {
                let id = id_map
                    .get(&(res_type.clone(), name.clone()))
                    .copied()
                    .unwrap_or(0);
                s.push_str(&format!(
                    "        public static int {} = 0x{:08x};\n",
                    name, id
                ));
            }
            s.push_str("    }\n");
        }

        let has_arrays = styleable_arrays.is_some_and(|a| !a.is_empty());
        let has_indices = styleable_indices.is_some_and(|i| !i.is_empty());

        if has_arrays || has_indices {
            s.push_str("    public static final class styleable {\n");
            if let Some(arrays) = styleable_arrays {
                for name in arrays {
                    if let Some(values) = self.styleable_arrays.get(name) {
                        let vals_joined = values
                            .iter()
                            .map(|v| format!("0x{:08x}", v))
                            .collect::<Vec<_>>()
                            .join(", ");
                        s.push_str(&format!(
                            "        public static int[] {} = {{ {} }};\n",
                            name, vals_joined
                        ));
                    }
                }
            }
            if let Some(indices) = styleable_indices {
                for (field_name, idx) in indices {
                    s.push_str(&format!(
                        "        public static int {} = {};\n",
                        field_name, idx
                    ));
                }
            }
            s.push_str("    }\n");
        }

        s.push_str("}\n");

        std::fs::write(&r_file, s)?;
        Ok(r_file)
    }

    pub fn emit_bytecode(&mut self, out_classes_dir: &Path, root_package: &str) -> Result<()> {
        let id_map = self.assign_resource_ids();
        self.resolve_styleables(&id_map);

        for (pkg, types) in &self.pkg_entries {
            self.emit_single_package_bytecode(
                out_classes_dir,
                pkg,
                types,
                self.pkg_styleable_arrays.get(pkg),
                self.pkg_styleable_indices.get(pkg),
                &id_map,
            )?;
        }

        if !self.pkg_entries.contains_key(root_package) {
            let all_arrays: BTreeSet<String> = self.styleable_arrays.keys().cloned().collect();
            let mut all_indices = BTreeMap::new();
            for indices in self.styleable_indices.values() {
                for (k, v) in indices {
                    all_indices.insert(k.clone(), *v);
                }
            }

            self.emit_single_package_bytecode(
                out_classes_dir,
                root_package,
                &self.all_entries,
                Some(&all_arrays),
                Some(&all_indices),
                &id_map,
            )?;
        }

        Ok(())
    }

    fn emit_single_package_bytecode(
        &self,
        out_classes_dir: &Path,
        pkg: &str,
        types: &BTreeMap<String, BTreeSet<String>>,
        styleable_arrays: Option<&BTreeSet<String>>,
        styleable_indices: Option<&BTreeMap<String, u32>>,
        id_map: &BTreeMap<(String, String), u32>,
    ) -> Result<()> {
        let pkg_rel = pkg.replace('.', "/");
        let pkg_dir = out_classes_dir.join(&pkg_rel);
        std::fs::create_dir_all(&pkg_dir)?;

        let r_class_name = format!("{}/R", pkg_rel);
        let r_bytes = RClassBytecodeWriter::emit_empty_class(&r_class_name);
        std::fs::write(pkg_dir.join("R.class"), r_bytes)?;

        for (res_type, names) in types {
            let inner_class_name = format!("{}/R${}", pkg_rel, res_type);
            let mut fields = Vec::with_capacity(names.len());
            for name in names {
                let id = id_map
                    .get(&(res_type.clone(), name.clone()))
                    .copied()
                    .unwrap_or(0);
                fields.push((name.clone(), id));
            }
            let bytes = RClassBytecodeWriter::emit_int_fields_class(&inner_class_name, &fields);
            std::fs::write(pkg_dir.join(format!("R${}.class", res_type)), bytes)?;
        }

        let has_arrays = styleable_arrays.is_some_and(|a| !a.is_empty());
        let has_indices = styleable_indices.is_some_and(|i| !i.is_empty());

        if has_arrays || has_indices {
            let inner_class_name = format!("{}/R$styleable", pkg_rel);
            let mut int_fields = Vec::new();
            if let Some(indices) = styleable_indices {
                for (field_name, idx) in indices {
                    int_fields.push((field_name.clone(), *idx));
                }
            }

            let mut array_fields = Vec::new();
            if let Some(arrays) = styleable_arrays {
                for name in arrays {
                    if let Some(values) = self.styleable_arrays.get(name) {
                        array_fields.push((name.clone(), values.clone()));
                    }
                }
            }

            let bytes = RClassBytecodeWriter::emit_styleable_class(
                &inner_class_name,
                &int_fields,
                &array_fields,
            );
            std::fs::write(pkg_dir.join("R$styleable.class"), bytes)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_r_class_generator_parsing() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let r_txt_path = tmp.path().join("R.txt");
        let r_txt_content = r#"
int anim abc_fade_in 0x0
int attr actionBarSize 0x0
int drawable abc_btn_check_material 0x0
int[] styleable ActionBar { 0x10100da, 0x0 }
int styleable ActionBar_android_focusable 0
int styleable ActionBar_actionBarSize 1
"#;
        std::fs::write(&r_txt_path, r_txt_content)?;

        let r_txt_files = vec![("androidx.appcompat".to_string(), r_txt_path)];
        let mut generator = RClassGenerator::parse(&r_txt_files)?;

        let id_map = generator.assign_resource_ids();
        assert!(id_map.contains_key(&("anim".to_string(), "abc_fade_in".to_string())));
        assert!(id_map.contains_key(&("attr".to_string(), "actionBarSize".to_string())));
        assert!(
            id_map.contains_key(&("drawable".to_string(), "abc_btn_check_material".to_string()))
        );

        generator.resolve_styleables(&id_map);
        let action_bar_values = generator.styleable_arrays.get("ActionBar").unwrap();
        assert_eq!(action_bar_values[0], 0x10100da);

        let attr_id = *id_map
            .get(&("attr".to_string(), "actionBarSize".to_string()))
            .unwrap();
        assert_eq!(action_bar_values[1], attr_id);

        let out_dir = tmp.path().join("r_src");
        let files = generator.write_sources(&out_dir, "com.example.app")?;
        assert_eq!(files.len(), 2);

        let appcompat_r = std::fs::read_to_string(out_dir.join("androidx/appcompat/R.java"))?;
        assert!(appcompat_r.contains("package androidx.appcompat;"));
        assert!(appcompat_r.contains("public static final class drawable"));
        assert!(appcompat_r.contains("public static int abc_btn_check_material"));

        Ok(())
    }

    #[test]
    fn test_r_class_bytecode_synthesis() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let r_txt_path = tmp.path().join("R.txt");
        let r_txt_content = r#"
int anim abc_fade_in 0x0
int attr actionBarSize 0x0
int drawable abc_btn_check_material 0x0
int[] styleable ActionBar { 0x10100da, 0x0 }
int styleable ActionBar_android_focusable 0
int styleable ActionBar_actionBarSize 1
"#;
        std::fs::write(&r_txt_path, r_txt_content)?;

        let r_txt_files = vec![("androidx.appcompat".to_string(), r_txt_path)];
        let mut generator = RClassGenerator::parse(&r_txt_files)?;

        let classes_dir = tmp.path().join("classes");
        generator.emit_bytecode(&classes_dir, "com.example.app")?;

        let app_r = classes_dir.join("com/example/app/R.class");
        let app_r_drawable = classes_dir.join("com/example/app/R$drawable.class");
        let app_r_styleable = classes_dir.join("com/example/app/R$styleable.class");
        assert!(app_r.is_file());
        assert!(app_r_drawable.is_file());
        assert!(app_r_styleable.is_file());

        let compat_r = classes_dir.join("androidx/appcompat/R.class");
        let compat_r_anim = classes_dir.join("androidx/appcompat/R$anim.class");
        assert!(compat_r.is_file());
        assert!(compat_r_anim.is_file());

        for class_path in [
            &app_r,
            &app_r_drawable,
            &app_r_styleable,
            &compat_r,
            &compat_r_anim,
        ] {
            let bytes = std::fs::read(class_path)?;
            assert!(bytes.len() >= 8);
            assert_eq!(&bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
            assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x00, 0x34]);
        }

        Ok(())
    }
}
