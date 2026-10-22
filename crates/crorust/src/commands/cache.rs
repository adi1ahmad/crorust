use crate::core::env::SessionExecutionContext;
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};

#[derive(Subcommand, Clone, Debug)]
pub enum CacheSubcommand {
    Info {
        #[clap(long)]
        project: Option<PathBuf>,
    },

    Clean(CacheCleanArgs),

    Prune {
        #[clap(long)]
        project: Option<PathBuf>,
    },

    /// Import Apple SDKs (iPhoneOS.sdk, iPhoneSimulator.sdk, MacOSX.sdk) from an Xcode .xip archive or .app bundle
    #[clap(
        name = "import-xcode",
        after_help = "EXAMPLES:
  crorust cache import-xcode /path/to/Xcode_15.xip
  crorust cache import-xcode /Applications/Xcode.app
  crorust cache import-xcode Xcode_15.xip --global"
    )]
    ImportXcode {
        /// Path to Xcode .xip archive, .pkg installer, or extracted Xcode.app directory
        path: PathBuf,

        /// Import into machine-wide global cache (~/.cache/crorust/sysroots/) instead of project-local cache
        #[clap(long)]
        global: bool,

        /// Path to project root (defaults to current directory)
        #[clap(short = 'C', long)]
        project: Option<PathBuf>,
    },
}

#[derive(clap::Args, Clone, Debug, Default)]
pub struct CacheCleanArgs {
    /// Clean machine-wide global cache (~/.cache/crorust/)
    #[clap(long)]
    pub global: bool,

    /// Clean project-local cache (.cache/ and packaging targets)
    #[clap(long)]
    pub project: bool,

    /// Clean all caches: project-local (.cache/), packaging outputs, and machine-wide global cache
    #[clap(long, conflicts_with_all = ["global", "project"])]
    pub all: bool,

    /// Purge only raw downloaded archives (keeps unpacked SDKs and tools)
    #[clap(long)]
    pub downloads_only: bool,

    /// Clean only Cargo compilation cache (.cache/target/ and target/)
    #[clap(long)]
    pub cargo: bool,

    /// Clean only Web static compilation output (.cache/web-static/ and web dist)
    #[clap(long)]
    pub web: bool,

    /// Clean only hermetic toolchains, sysroots, and Maven packages (.cache/crorust/tools, sysroots, maven)
    #[clap(long)]
    pub tools: bool,

    /// Clean only packaging build artifacts (target/crorust/)
    #[clap(long)]
    pub build: bool,

    /// Explicit path to project root directory
    #[clap(short = 'C', long)]
    pub project_path: Option<PathBuf>,
}

pub fn run(subcommand: CacheSubcommand) -> Result<()> {
    match subcommand {
        CacheSubcommand::Info { project } => run_info(project.as_deref()),
        CacheSubcommand::Clean(clean_args) => run_clean(clean_args),
        CacheSubcommand::Prune { project } => run_prune(project.as_deref()),
        CacheSubcommand::ImportXcode {
            path,
            global,
            project,
        } => run_import_xcode(&path, global, project.as_deref()),
    }
}

pub fn compute_dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub fn detect_project_root(hint: Option<&Path>) -> Option<PathBuf> {
    let start_dir = if let Some(h) = hint {
        if h.is_dir() {
            h.to_path_buf()
        } else {
            h.parent()?.to_path_buf()
        }
    } else {
        std::env::current_dir().ok()?
    };

    let mut cur = Some(start_dir.as_path());
    while let Some(dir) = cur {
        if dir.join("manifest.toml").is_file() || dir.join("Cargo.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

pub fn discover_project_cache(hint: Option<&Path>) -> Option<PathBuf> {
    detect_project_root(hint).map(|root| root.join(".cache").join("crorust"))
}

pub fn run_info(project_hint: Option<&Path>) -> Result<()> {
    let global_dir = SessionExecutionContext::global_cache_dir();
    let global_size = compute_dir_size(&global_dir);

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                  crorust Cache Diagnostics                    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    if let Some(project_root) = detect_project_root(project_hint) {
        let dot_cache = project_root.join(".cache");
        let target_crorust = project_root.join("target").join("crorust");
        let has_cache = dot_cache.exists() || target_crorust.exists();
        let dot_cache_size = compute_dir_size(&dot_cache);
        let target_size = compute_dir_size(&target_crorust);
        let total_project_size = dot_cache_size + target_size;

        println!("Project Workspace: {}", project_root.display());
        if has_cache {
            println!("  Cache Directory:  {}", dot_cache.display());
            println!("  Total Cache Size: {}", format_bytes(total_project_size));
            print_project_breakdown(&project_root);
        } else {
            println!("  Project Cache:    Not created yet (clean project state)");
        }
    } else {
        println!("Project Workspace: None detected (running outside a crorust project)");
    }

    println!("\nMachine-Wide Global Cache:");
    println!("  Cache Directory:  {}", global_dir.display());
    println!("  Total Size:       {}", format_bytes(global_size));
    if global_size > 0 {
        print_global_breakdown(&global_dir);
        println!("\n  Recommendation: Run 'crorust cache clean --global' to reclaim space.");
    }

    println!();
    Ok(())
}

fn print_project_breakdown(project_root: &Path) {
    let dot_cache = project_root.join(".cache");
    let crorust_cache = dot_cache.join("crorust");
    let cargo_target = dot_cache.join("target");
    let web_static = dot_cache.join("web-static");
    let target_crorust = cargo_target.join("crorust");
    let target_crorust_root = project_root.join("target").join("crorust");

    let tools = crorust_cache.join("tools");
    let sysroots = crorust_cache.join("sysroots");
    let maven = crorust_cache.join("maven");
    let tasks = crorust_cache.join("tasks");
    let downloads = crorust_cache.join("download");

    if tools.exists() {
        println!(
            "  ├─ Tools & Compilers: {}",
            format_bytes(compute_dir_size(&tools))
        );
    }
    if sysroots.exists() {
        println!(
            "  ├─ Target Sysroots:   {}",
            format_bytes(compute_dir_size(&sysroots))
        );
    }
    if maven.exists() {
        println!(
            "  ├─ Maven Artifacts:   {}",
            format_bytes(compute_dir_size(&maven))
        );
    }
    if cargo_target.exists() {
        println!(
            "  ├─ Cargo Target:      {}",
            format_bytes(compute_dir_size(&cargo_target))
        );
    }
    if web_static.exists() {
        println!(
            "  ├─ Web Static:        {}",
            format_bytes(compute_dir_size(&web_static))
        );
    }
    if tasks.exists() {
        println!(
            "  ├─ Incremental Tasks: {}",
            format_bytes(compute_dir_size(&tasks))
        );
    }
    if downloads.exists() {
        println!(
            "  ├─ Raw Downloads:     {}",
            format_bytes(compute_dir_size(&downloads))
        );
    }
    if target_crorust.exists() {
        println!(
            "  └─ Packaging Outputs: {}",
            format_bytes(compute_dir_size(&target_crorust))
        );
    } else if target_crorust_root.exists() {
        println!(
            "  └─ Packaging Outputs: {}",
            format_bytes(compute_dir_size(&target_crorust_root))
        );
    }
}

fn print_global_breakdown(base: &Path) {
    let tools = base.join("tools");
    let sysroots = base.join("sysroots");
    let maven = base.join("maven");
    let tasks = base.join("tasks");
    let downloads = base.join("download");

    if tools.exists() {
        println!(
            "  ├─ Tools & Compilers: {}",
            format_bytes(compute_dir_size(&tools))
        );
    }
    if sysroots.exists() {
        println!(
            "  ├─ Target Sysroots:   {}",
            format_bytes(compute_dir_size(&sysroots))
        );
    }
    if maven.exists() {
        println!(
            "  ├─ Maven Artifacts:   {}",
            format_bytes(compute_dir_size(&maven))
        );
    }
    if tasks.exists() {
        println!(
            "  ├─ Incremental Tasks: {}",
            format_bytes(compute_dir_size(&tasks))
        );
    }
    if downloads.exists() {
        println!(
            "  └─ Raw Downloads:     {}",
            format_bytes(compute_dir_size(&downloads))
        );
    }
}

pub fn clean_directory(dir: &Path, label: &str) -> Result<u64> {
    if dir.exists() {
        let size = compute_dir_size(dir);
        std::fs::remove_dir_all(dir)
            .with_context(|| format!("Failed to remove {label} at {}", dir.display()))?;
        println!(
            "  [✓] Purged {} ({}) at {}",
            label,
            format_bytes(size),
            dir.display()
        );
        Ok(size)
    } else {
        Ok(0)
    }
}

fn clean_cache_root(root: &Path, label: &str, downloads_only: bool) -> Result<u64> {
    if downloads_only {
        let dl_dir = root.join("download");
        let mut size = 0;
        if dl_dir.exists() {
            size = compute_dir_size(&dl_dir);
            std::fs::remove_dir_all(&dl_dir)
                .with_context(|| format!("Failed to remove {}", dl_dir.display()))?;
            println!(
                "  [✓] Cleared {} download archives ({}) at {}",
                label,
                format_bytes(size),
                dl_dir.display()
            );
        } else {
            println!(
                "  No downloaded archives to clean in {} cache ({}).",
                label,
                root.display()
            );
        }
        Ok(size)
    } else if root.exists() {
        let size = compute_dir_size(root);
        std::fs::remove_dir_all(root)
            .with_context(|| format!("Failed to remove {}", root.display()))?;
        println!(
            "  [✓] Purged {} cache ({}) at {}",
            label,
            format_bytes(size),
            root.display()
        );
        Ok(size)
    } else {
        println!("  {} cache does not exist at {}", label, root.display());
        Ok(0)
    }
}

fn clean_full_project(root: &Path, downloads_only: bool) -> Result<u64> {
    let mut total_reclaimed: u64 = 0;
    let dot_cache = root.join(".cache");

    if downloads_only {
        let pcache = dot_cache.join("crorust");
        let dl = pcache.join("download");
        total_reclaimed += clean_directory(&dl, "Project raw downloads")?;
        return Ok(total_reclaimed);
    }

    // 1. Purge entire .cache directory (removes crorust, target, web-static, etc.)
    if dot_cache.exists() {
        total_reclaimed += clean_directory(&dot_cache, "project cache (.cache)")?;
    } else {
        println!("  Project .cache does not exist at {}", dot_cache.display());
    }

    // 2. Purge target/crorust (packaging artifacts)
    let target_crorust = root.join("target").join("crorust");
    if target_crorust.exists() {
        total_reclaimed +=
            clean_directory(&target_crorust, "packaging artifacts (target/crorust)")?;
    }

    // 3. Purge target directory if it exists and only contains build artifacts
    let target_dir = root.join("target");
    if target_dir.exists() {
        let is_empty = std::fs::read_dir(&target_dir)
            .map(|mut i| i.next().is_none())
            .unwrap_or(false);
        if is_empty {
            let _ = std::fs::remove_dir(&target_dir);
        }
    }

    println!(
        "Project cache clean complete. Total reclaimed: {}",
        format_bytes(total_reclaimed)
    );
    Ok(total_reclaimed)
}

pub fn run_clean(args: CacheCleanArgs) -> Result<()> {
    let global_cache = SessionExecutionContext::global_cache_dir();
    let project_root = detect_project_root(args.project_path.as_deref());

    let is_selective = args.cargo || args.web || args.tools || args.build || args.downloads_only;

    if args.all {
        println!("Purging all project caches, build artifacts, and global cache...");
        if let Some(root) = &project_root {
            clean_full_project(root, args.downloads_only)?;
        }
        clean_cache_root(&global_cache, "global", args.downloads_only)?;
        return Ok(());
    }

    if args.global && !args.project && !is_selective {
        clean_cache_root(&global_cache, "global", args.downloads_only)?;
        return Ok(());
    }

    let root = match project_root {
        Some(r) => r,
        None => {
            if args.global {
                clean_cache_root(&global_cache, "global", args.downloads_only)?;
                return Ok(());
            } else {
                anyhow::bail!(
                    "No crorust project detected in the current directory. Specify --global to clean global cache."
                );
            }
        }
    };

    if is_selective {
        println!(
            "Cleaning selected cache components in {}...",
            root.display()
        );
        let mut total_reclaimed: u64 = 0;

        if args.cargo {
            let cargo_target = root.join(".cache").join("target");
            if cargo_target.exists() {
                total_reclaimed += clean_directory(&cargo_target, "Cargo target cache")?;
            }

            let target_dir = root.join("target");
            if target_dir.exists() {
                let target_crorust = target_dir.join("crorust");
                if target_crorust.exists() && !args.build {
                    if let Ok(entries) = std::fs::read_dir(&target_dir) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p != target_crorust {
                                if p.is_dir() {
                                    total_reclaimed += clean_directory(&p, "Cargo build dir")?;
                                } else {
                                    let s = p.metadata().map(|m| m.len()).unwrap_or(0);
                                    let _ = std::fs::remove_file(&p);
                                    total_reclaimed += s;
                                }
                            }
                        }
                    }
                } else {
                    total_reclaimed += clean_directory(&target_dir, "Cargo target directory")?;
                }
            }
        }

        if args.web {
            let web_static = root.join(".cache").join("web-static");
            total_reclaimed += clean_directory(&web_static, "Web static cache")?;

            let dist_dir = root.join("dist");
            if dist_dir.exists() {
                total_reclaimed += clean_directory(&dist_dir, "Web dist artifacts")?;
            }
        }

        if args.tools {
            let pcache = root.join(".cache").join("crorust");
            for sub in &["tools", "sysroots", "maven", "tasks", "staging", "locks"] {
                let sub_dir = pcache.join(sub);
                total_reclaimed += clean_directory(&sub_dir, &format!("Toolchain {sub}"))?;
            }
        }

        if args.build {
            let target_crorust = root.join(".cache").join("target").join("crorust");
            if target_crorust.exists() {
                total_reclaimed += clean_directory(
                    &target_crorust,
                    "Packaging build artifacts (.cache/target/crorust)",
                )?;
            }
            let target_crorust_root = root.join("target").join("crorust");
            if target_crorust_root.exists() {
                total_reclaimed += clean_directory(
                    &target_crorust_root,
                    "Packaging build artifacts (target/crorust)",
                )?;
            }
        }

        if args.downloads_only {
            let pcache = root.join(".cache").join("crorust");
            let dl = pcache.join("download");
            total_reclaimed += clean_directory(&dl, "Project raw downloads")?;
            if args.global {
                let dl = global_cache.join("download");
                total_reclaimed += clean_directory(&dl, "Global raw downloads")?;
            }
        }

        if args.global && !args.downloads_only {
            clean_cache_root(&global_cache, "global", false)?;
        }

        println!(
            "Selective cache clean complete. Total reclaimed: {}",
            format_bytes(total_reclaimed)
        );
        return Ok(());
    }

    // Default clean behavior: clean the entire project .cache/ directory and target/crorust/
    println!(
        "Cleaning project caches and build artifacts in {}...",
        root.display()
    );
    clean_full_project(&root, args.downloads_only)?;

    if args.global {
        clean_cache_root(&global_cache, "global", args.downloads_only)?;
    }

    Ok(())
}

pub fn run_prune(project_hint: Option<&Path>) -> Result<()> {
    println!("Pruning stale cache locks, interrupted downloads, and staging directories...");
    if let Some(root) = detect_project_root(project_hint) {
        let pcache = root.join(".cache").join("crorust");
        prune_cache_dir(&pcache);
    }
    prune_cache_dir(&SessionExecutionContext::global_cache_dir());
    println!("  [✓] Cache prune complete.");
    Ok(())
}

pub fn prune_cache_dir(base: &Path) {
    if !base.exists() {
        return;
    }

    let staging_root = base.join("staging");
    if staging_root.exists() {
        if let Ok(entries) = std::fs::read_dir(&staging_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with(".staging-") {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }

    let download_root = base.join("download");
    if download_root.exists() {
        if let Ok(entries) = std::fs::read_dir(&download_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = entry.file_name();
                    if name.to_string_lossy().contains(".part-") {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    let lock_root = base.join("locks");
    if lock_root.exists() {
        if let Ok(entries) = std::fs::read_dir(&lock_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("lock") {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    let tasks_dir = base.join("tasks");
    if tasks_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.starts_with(".tmp"))
                        .unwrap_or(false)
                {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }
    }
}

pub fn run_import_xcode(
    xcode_path: &Path,
    global: bool,
    project_hint: Option<&Path>,
) -> Result<()> {
    if !xcode_path.exists() {
        bail!(
            "Specified Xcode path does not exist: {}",
            xcode_path.display()
        );
    }

    let sysroots_dir = if global {
        SessionExecutionContext::global_cache_dir().join("sysroots")
    } else {
        let root = detect_project_root(project_hint).unwrap_or_else(|| PathBuf::from("."));
        root.join(".cache").join("crorust").join("sysroots")
    };

    println!(
        "Importing Apple SDKs from '{}' into '{}'...",
        xcode_path.display(),
        sysroots_dir.display()
    );

    let imported = crate::download::apple_pkg::ApplePkgExtractor::import_xcode_sdks(
        xcode_path,
        &sysroots_dir,
    )?;

    if imported.is_empty() {
        println!(
            "⚠️  No Apple SDKs found in the specified path: {}",
            xcode_path.display()
        );
        println!("   Supported contents: iPhoneOS.sdk, iPhoneSimulator.sdk, MacOSX.sdk");
    } else {
        println!("✅ Successfully imported {} Apple SDK(s):", imported.len());
        for (name, path) in imported {
            println!("   • {} -> {}", name, path.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_prune_cleans_temporary_artifacts_only() {
        let temp = tempdir().unwrap();
        let cache_dir = temp.path();

        let staging_root = cache_dir.join("staging");
        std::fs::create_dir_all(&staging_root).unwrap();
        let orphan_staging = staging_root.join(".staging-jdk-9999");
        std::fs::create_dir_all(&orphan_staging).unwrap();
        std::fs::write(orphan_staging.join("incomplete.bin"), "temp").unwrap();

        let download_root = cache_dir.join("download");
        std::fs::create_dir_all(&download_root).unwrap();
        let complete_archive = download_root.join("Windows.sdk.tar.zst");
        std::fs::write(&complete_archive, "sdk-archive-bytes").unwrap();
        let partial_download = download_root.join("Windows.sdk.tar.zst.part-9999");
        std::fs::write(&partial_download, "partial-bytes").unwrap();

        let lock_root = cache_dir.join("locks");
        std::fs::create_dir_all(&lock_root).unwrap();
        let stale_lock = lock_root.join("jdk.lock");
        std::fs::write(&stale_lock, "").unwrap();

        prune_cache_dir(cache_dir);

        assert!(!staging_root.join(".staging-jdk-9999").exists());
        assert!(!download_root.join("Windows.sdk.tar.zst.part-9999").exists());
        assert!(!lock_root.join("jdk.lock").exists());

        assert!(complete_archive.exists());
    }

    #[test]
    fn test_cache_clean_default_purges_all_project_cache_buckets() {
        let temp = tempdir().unwrap();
        let project_root = temp.path();
        std::fs::write(project_root.join("manifest.toml"), "package = \"test\"").unwrap();

        let dot_cache = project_root.join(".cache");
        let cargo_target = dot_cache.join("target");
        let web_static = dot_cache.join("web-static");
        let crorust_cache = dot_cache.join("crorust");
        let target_crorust = project_root.join("target").join("crorust");

        std::fs::create_dir_all(&cargo_target).unwrap();
        std::fs::write(cargo_target.join("libapp.rlib"), "rlib-bytes").unwrap();

        std::fs::create_dir_all(&web_static).unwrap();
        std::fs::write(web_static.join("index.html"), "<html></html>").unwrap();

        std::fs::create_dir_all(crorust_cache.join("tools")).unwrap();
        std::fs::write(crorust_cache.join("tools").join("kotlinc"), "binary").unwrap();

        std::fs::create_dir_all(&target_crorust).unwrap();
        std::fs::write(target_crorust.join("app.apk"), "apk-bytes").unwrap();

        assert!(cargo_target.exists());
        assert!(web_static.exists());
        assert!(crorust_cache.exists());
        assert!(target_crorust.exists());

        let args = CacheCleanArgs {
            project_path: Some(project_root.to_path_buf()),
            ..Default::default()
        };
        run_clean(args).unwrap();

        assert!(!dot_cache.exists(), ".cache must be completely removed");
        assert!(!cargo_target.exists(), "target must not remain");
        assert!(!web_static.exists(), "web-static must not remain");
        assert!(!target_crorust.exists(), "target/crorust must be removed");
    }

    #[test]
    fn test_cache_clean_selective_cargo_only() {
        let temp = tempdir().unwrap();
        let project_root = temp.path();
        std::fs::write(project_root.join("manifest.toml"), "package = \"test\"").unwrap();

        let dot_cache = project_root.join(".cache");
        let cargo_target = dot_cache.join("target");
        let web_static = dot_cache.join("web-static");

        std::fs::create_dir_all(&cargo_target).unwrap();
        std::fs::write(cargo_target.join("app.rlib"), "bytes").unwrap();

        std::fs::create_dir_all(&web_static).unwrap();
        std::fs::write(web_static.join("index.html"), "html").unwrap();

        let args = CacheCleanArgs {
            cargo: true,
            project_path: Some(project_root.to_path_buf()),
            ..Default::default()
        };
        run_clean(args).unwrap();

        assert!(!cargo_target.exists(), ".cache/target must be removed");
        assert!(web_static.exists(), "web-static must remain untouched");
    }

    #[test]
    fn test_cache_clean_selective_web_only() {
        let temp = tempdir().unwrap();
        let project_root = temp.path();
        std::fs::write(project_root.join("manifest.toml"), "package = \"test\"").unwrap();

        let dot_cache = project_root.join(".cache");
        let cargo_target = dot_cache.join("target");
        let web_static = dot_cache.join("web-static");

        std::fs::create_dir_all(&cargo_target).unwrap();
        std::fs::write(cargo_target.join("app.rlib"), "bytes").unwrap();

        std::fs::create_dir_all(&web_static).unwrap();
        std::fs::write(web_static.join("index.html"), "html").unwrap();

        let args = CacheCleanArgs {
            web: true,
            project_path: Some(project_root.to_path_buf()),
            ..Default::default()
        };
        run_clean(args).unwrap();

        assert!(!web_static.exists(), "web-static must be removed");
        assert!(cargo_target.exists(), ".cache/target must remain untouched");
    }
}
