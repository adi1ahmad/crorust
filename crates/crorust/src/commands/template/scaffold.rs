use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

const TEMPLATE_ARCHIVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/template.tar.zst"));

#[derive(Debug, Clone)]
pub struct ProjectNames {
    pub raw: String,

    pub kebab: String,

    pub snake: String,

    pub pascal: String,

    pub title: String,

    pub package_id: String,

    pub jni_path: String,

    pub jni_symbol_prefix: String,
}

impl ProjectNames {
    pub fn new(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            bail!("Project name cannot be empty");
        }

        for c in trimmed.chars() {
            if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
                bail!(
                    "Invalid character '{}' in project name '{}'. Use only ASCII letters, digits, hyphens, and underscores.",
                    c,
                    trimmed
                );
            }
        }

        if trimmed.chars().next().unwrap().is_ascii_digit() {
            bail!("Project name '{}' cannot start with a digit", trimmed);
        }

        let snake = trimmed.to_ascii_lowercase().replace('-', "_");
        let kebab = trimmed.to_ascii_lowercase().replace('_', "-");

        Self::check_reserved_keywords(&snake)?;

        Self::check_windows_reserved(&snake)?;

        let mut pascal = String::new();
        let mut title = String::new();

        for word in snake.split('_') {
            if word.is_empty() {
                continue;
            }
            let mut chars = word.chars();
            let first = chars.next().unwrap().to_ascii_uppercase();
            let rest: String = chars.collect();

            pascal.push(first);
            pascal.push_str(&rest);

            if !title.is_empty() {
                title.push(' ');
            }
            title.push(first);
            title.push_str(&rest);
        }

        let package_id = format!("com.example.{}", snake);
        let jni_path = format!("com/example/{}", snake);

        let mangled_snake = snake.replace('_', "_1");
        let jni_symbol_prefix = format!("com_example_{}", mangled_snake);

        Ok(Self {
            raw: trimmed.to_string(),
            kebab,
            snake,
            pascal,
            title,
            package_id,
            jni_path,
            jni_symbol_prefix,
        })
    }

    fn check_reserved_keywords(ident: &str) -> Result<()> {
        let reserved: HashSet<&'static str> = [
            "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
            "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become",
            "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual",
            "yield", "try",
        ]
        .into_iter()
        .collect();

        if reserved.contains(ident) {
            bail!(
                "Project identifier '{}' is a reserved Rust keyword and cannot be used as a package name.",
                ident
            );
        }
        Ok(())
    }

    fn check_windows_reserved(ident: &str) -> Result<()> {
        let reserved: HashSet<&'static str> = [
            "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
            "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
        ]
        .into_iter()
        .collect();

        if reserved.contains(ident) {
            bail!(
                "Project identifier '{}' is a reserved device name on Windows and cannot be used.",
                ident
            );
        }
        Ok(())
    }
}

pub fn extract_raw_template_archive(dest_dir: &Path) -> Result<()> {
    if TEMPLATE_ARCHIVE.is_empty() {
        bail!("Embedded template archive is empty. Ensure templates/crorust.ruxt was present during crorust build.");
    }
    let zstd_reader = zstd::stream::Decoder::new(TEMPLATE_ARCHIVE)?;
    let mut archive = Archive::new(zstd_reader);
    archive.unpack(dest_dir)?;
    Ok(())
}

pub fn init(path_or_name: &str) -> Result<()> {
    let target_dir = PathBuf::from(path_or_name);
    let project_name = target_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid project path: missing directory name"))?;

    let names = ProjectNames::new(project_name)?;

    if target_dir.exists() {
        bail!(
            "Target directory '{}' already exists. Aborting to prevent overwriting existing files.",
            target_dir.display()
        );
    }

    println!(
        "Scaffolding modern cross-platform application '{}' from templates/crorust.ruxt...",
        names.title
    );
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create directory '{}'", target_dir.display()))?;

    ScaffoldEngine::new(&names, &target_dir).execute()?;

    println!(
        "\nProject created successfully at: {}",
        target_dir.display()
    );
    println!("To start development:");
    println!("  cd {}", path_or_name);
    println!("  crorust build --platform host     # Build native desktop executable");
    println!("  crorust run                       # Run on host or connected Android device");
    println!("  crorust build --platform android  # Build signed Android APK");
    println!("  nix develop                       # Enter hermetic development shell (optional)");

    Ok(())
}

pub struct ScaffoldEngine<'a> {
    pub names: &'a ProjectNames,
    pub root: &'a Path,
    pub is_in_tree: bool,
}

impl<'a> ScaffoldEngine<'a> {
    pub fn new(names: &'a ProjectNames, root: &'a Path) -> Self {
        let is_in_tree = Self::detect_in_tree_workspace(root);
        Self {
            names,
            root,
            is_in_tree,
        }
    }

    fn detect_in_tree_workspace(start_dir: &Path) -> bool {
        let mut cur = start_dir.parent();
        while let Some(dir) = cur {
            if dir.join("Cargo.toml").is_file() && dir.join("crates").join("crorust").is_dir() {
                return true;
            }
            cur = dir.parent();
        }
        false
    }

    pub fn execute(&self) -> Result<()> {
        if TEMPLATE_ARCHIVE.is_empty() {
            bail!("Embedded template archive is empty. Ensure templates/crorust.ruxt was present during crorust build.");
        }

        let zstd_reader = zstd::stream::Decoder::new(TEMPLATE_ARCHIVE)?;
        let mut archive = Archive::new(zstd_reader);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            let mode = entry.header().mode().unwrap_or(0o644);
            let mut content_bytes = Vec::new();
            entry.read_to_end(&mut content_bytes)?;

            self.process_template_entry(&path, content_bytes, mode)?;
        }

        scaffold_project_hygiene(self.root)?;

        Ok(())
    }

    pub fn is_binary_asset(path: &Path, bytes: &[u8]) -> bool {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext.to_ascii_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "ico" | "icns" | "jar" | "aar" | "so" | "dylib"
            | "dll" | "exe" | "tar" | "zip" | "zst" => true,
            _ => std::str::from_utf8(bytes).is_err(),
        }
    }

    fn process_template_entry(&self, rel_path: &Path, bytes: Vec<u8>, mode: u32) -> Result<()> {
        let dest_path = self.root.join(rel_path);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if Self::is_binary_asset(rel_path, &bytes) {
            fs::write(&dest_path, bytes)?;
            self.apply_entry_permissions(&dest_path, mode)?;
            return Ok(());
        }

        let mut text = String::from_utf8(bytes)
            .with_context(|| format!("Failed to read UTF-8 string for '{}'", rel_path.display()))?;

        self.apply_token_substitutions(rel_path, &mut text);

        fs::write(&dest_path, text.as_bytes())?;
        self.apply_entry_permissions(&dest_path, mode)?;
        Ok(())
    }

    fn apply_entry_permissions(&self, path: &Path, mode: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if mode & 0o111 != 0 {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).with_context(
                    || {
                        format!(
                            "Failed to set executable permissions on '{}'",
                            path.display()
                        )
                    },
                )?;
            }
        }
        let _ = path;
        let _ = mode;
        Ok(())
    }

    pub fn apply_token_substitutions(&self, rel_path: &Path, text: &mut String) {
        if text.contains("\r\n") {
            *text = text.replace("\r\n", "\n");
        }

        let file_name = rel_path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        if file_name == "Cargo.toml" {
            *text = text.replace(
                "[package]\nname = \"ruxt\"",
                &format!("[package]\nname = \"{}\"", self.names.kebab),
            );
            *text = text.replace(
                "[lib]\nname = \"ruxt\"",
                &format!("[lib]\nname = \"{}\"", self.names.snake),
            );
            *text = text.replace(
                "[[bin]]\nname = \"ruxt-app\"",
                &format!("[[bin]]\nname = \"{}\"", self.names.snake),
            );
            *text = text.replace(
                "[[bin]]\nname = \"ruxt\"",
                &format!("[[bin]]\nname = \"{}\"", self.names.snake),
            );
            *text = text.replace(
                "default-run = \"ruxt-app\"",
                &format!("default-run = \"{}\"", self.names.snake),
            );
            *text = text.replace(
                "default-run = \"ruxt\"",
                &format!("default-run = \"{}\"", self.names.snake),
            );
        } else if file_name == "Cargo.lock" {
            *text = text.replace(
                "[[package]]\nname = \"ruxt\"",
                &format!("[[package]]\nname = \"{}\"", self.names.kebab),
            );
        } else if file_name == "manifest.toml" {
            *text = text.replace(
                "package = \"ruxt\"",
                &format!("package = \"{}\"", self.names.kebab),
            );
            *text = text.replace(
                "name = \"ruxt\"",
                &format!("name = \"{}\"", self.names.snake),
            );
            *text = text.replace(
                "bin = \"ruxt-app\"",
                &format!("bin = \"{}\"", self.names.snake),
            );
            *text = text.replace("bin = \"ruxt\"", &format!("bin = \"{}\"", self.names.snake));
            *text = text.replace(
                "label = \"ruxt\"",
                &format!("label = \"{}\"", self.names.title),
            );
        } else {
            *text = text.replace(
                "package = \"ruxt\"",
                &format!("package = \"{}\"", self.names.kebab),
            );
            *text = text.replace(
                "bin = \"ruxt-app\"",
                &format!("bin = \"{}\"", self.names.snake),
            );
            *text = text.replace("bin = \"ruxt\"", &format!("bin = \"{}\"", self.names.snake));
            *text = text.replace(
                "data-bin=\"ruxt-app\"",
                &format!("data-bin=\"{}\"", self.names.snake),
            );
            *text = text.replace(
                "data-bin=\"ruxt\"",
                &format!("data-bin=\"{}\"", self.names.snake),
            );
            *text = text.replace(
                "System.loadLibrary(\"ruxt\")",
                &format!("System.loadLibrary(\"{}\")", self.names.snake),
            );
        }

        if file_name.ends_with(".rs") {
            *text = text.replace("ruxt::", &format!("{}::", self.names.snake));
        }

        if file_name == "main.rs" {
            *text = text.replace(
                ".with_title(\"crorust.ruxt\")",
                &format!(".with_title(\"{}\")", self.names.title),
            );
            *text = text.replace(
                "\"ruxt\",\n        options",
                &format!("\"{}\",\n        options", self.names.title),
            );
        }

        *text = text.replace("crorust.ruxt", &self.names.package_id);
        *text = text.replace("crorust/ruxt", &self.names.jni_path);
        *text = text.replace(
            "Java_crorust_ruxt_",
            &format!("Java_{}_", self.names.jni_symbol_prefix),
        );

        let app_type_name = if self.names.pascal.ends_with("App") {
            self.names.pascal.clone()
        } else {
            format!("{}App", self.names.pascal)
        };
        *text = text.replace("RuxtApp", &app_type_name);
        *text = text.replace("CN=crorust", &format!("CN={}", self.names.pascal));
        *text = text.replace(
            "crorust.ruxt — cross-platform egui application",
            &format!("{} — cross-platform application", self.names.title),
        );
        *text = text.replace(
            "crorust.ruxt — ruxt app by crorust",
            &format!("{} application", self.names.title),
        );

        if file_name == "flake.nix" && !self.is_in_tree {
            *text = text.replace(
                "url = \"path:../..\";",
                "url = \"github:adi1ahmad/crorust\"; # Official CroRust upstream repository",
            );
        }
    }
}

pub fn scaffold_project_hygiene(project_root: &Path) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let mut gitignore_content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut modified = false;
    let has_cache_ignore = gitignore_content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "/.cache/"
            || trimmed == "/.cache"
            || trimmed == ".cache"
            || trimmed == ".cache/"
            || trimmed == "/.cache/*"
    });

    if !has_cache_ignore {
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
            gitignore_content.push('\n');
        }
        gitignore_content.push_str(
            "\n# crorust project-local toolchain & sysroot cache\n/.cache/*\n!/.cache/.gitkeep\n",
        );
        modified = true;
    }

    let has_target_ignore = gitignore_content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "/target/" || trimmed == "/target" || trimmed == "target" || trimmed == "target/"
    });

    if !has_target_ignore {
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
            gitignore_content.push('\n');
        }
        gitignore_content.push_str("\n# Rust build artifacts\n/target/\n");
        modified = true;
    }

    let has_dist_ignore = gitignore_content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "/dist/" || trimmed == "/dist" || trimmed == "dist" || trimmed == "dist/"
    });

    if !has_dist_ignore {
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
            gitignore_content.push('\n');
        }
        gitignore_content.push_str("\n# Web distribution output fallback\n/dist/\n");
        modified = true;
    }

    if modified {
        std::fs::write(&gitignore_path, gitignore_content)?;
    }

    // Ensure .cache directory exists so Trunk canonicalization never fails on fresh projects
    let cache_dir = project_root.join(".cache");
    std::fs::create_dir_all(&cache_dir)?;
    let gitkeep_path = cache_dir.join(".gitkeep");
    if !gitkeep_path.exists() {
        let _ = std::fs::write(&gitkeep_path, b"");
    }

    let vscode_dir = project_root.join(".vscode");
    std::fs::create_dir_all(&vscode_dir)?;
    let settings_path = vscode_dir.join("settings.json");

    if !settings_path.exists() {
        const VSCODE_SETTINGS_CONTENT: &str = r#"{
  "files.watcherExclude": {
    "**/.cache/**": true,
    "**/target/**": true
  },
  "search.exclude": {
    "**/.cache": true,
    "**/target": true
  },
  "rust-analyzer.files.excludeDirs": [
    ".cache",
    "target"
  ]
}
"#;
        std::fs::write(&settings_path, VSCODE_SETTINGS_CONTENT)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_scaffold_project_hygiene_idempotent() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let initial_gitignore = "# Custom user ignore\n*.log\n";
        std::fs::write(root.join(".gitignore"), initial_gitignore).unwrap();

        scaffold_project_hygiene(root).unwrap();

        let content = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(content.contains("*.log"));
        assert!(content.contains("/.cache/"));
        assert!(content.contains("/target/"));
        assert!(content.contains("/dist/"));

        let settings = std::fs::read_to_string(root.join(".vscode").join("settings.json")).unwrap();
        assert!(settings.contains("\"**/.cache/**\": true"));
        assert!(settings.contains("\"**/target/**\": true"));

        scaffold_project_hygiene(root).unwrap();
        let content_second = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(content, content_second);
    }

    #[test]
    fn test_cargo_lock_token_substitution() {
        let names = ProjectNames::new("smoke_test_app").unwrap();
        let engine = ScaffoldEngine::new(&names, Path::new("/dummy"));

        let mut lock_content = String::from(
            "version = 4\n\n[[package]]\nname = \"ruxt\"\nversion = \"0.1.0\"\ndependencies = []\n",
        );
        engine.apply_token_substitutions(Path::new("Cargo.lock"), &mut lock_content);
        assert!(
            lock_content.contains("[[package]]\nname = \"smoke-test-app\""),
            "Cargo.lock package name must be renamed to kebab-case project identifier: {lock_content}"
        );
        assert!(
            !lock_content.contains("name = \"ruxt\""),
            "Cargo.lock must not contain reference to old ruxt package"
        );
    }
}
