use crate::config::WebPlatformConfig;
use crate::core::env::SessionExecutionContext;
use crate::core::target::{TargetOpt, TargetPlatform};
use anyhow::{Context, Result};
use std::path::Path;

pub fn resolve_web_dist_dir(
    config: &WebPlatformConfig,
    project_root: &Path,
    default_platform_dist: &Path,
) -> std::path::PathBuf {
    match &config.dist {
        Some(custom_dist) => {
            if custom_dist.is_relative() {
                project_root.join(custom_dist)
            } else {
                custom_dist.clone()
            }
        }
        None => default_platform_dist.to_path_buf(),
    }
}

pub fn build_trunk_serve_args(
    config: &WebPlatformConfig,
    is_release: bool,
    project_root: &Path,
    dist_dir: &Path,
) -> Vec<String> {
    let mut args = vec!["serve".to_string()];

    let address = config.address.as_deref().unwrap_or("127.0.0.1");
    args.push("--address".to_string());
    args.push(address.to_string());

    // Suppress reverse DNS lookups on loopback which produce trailing dots like "localhost." on Linux systemd-resolved
    // or Bonjour/NetBIOS anomalies on macOS and Windows.
    args.push("--disable-address-lookup".to_string());

    let port = config.port.unwrap_or(8080);
    args.push("--port".to_string());
    args.push(port.to_string());

    // Explicitly set the distribution directory so dev server and packaging outputs are unified
    args.push("--dist".to_string());
    args.push(dist_dir.to_string_lossy().to_string());

    let open_browser = config.open.unwrap_or(true);
    if open_browser {
        args.push("--open".to_string());
    }

    if is_release {
        args.push("--release".to_string());
        args.push("--no-autoreload".to_string());
    } else {
        // Explicitly watch source directories to prevent inotify storms over .cache and target
        if let Some(explicit_watches) = &config.watch {
            for watch_path in explicit_watches {
                let full_path = if watch_path.is_relative() {
                    project_root.join(watch_path)
                } else {
                    watch_path.clone()
                };
                if full_path.exists() {
                    args.push("--watch".to_string());
                    args.push(full_path.to_string_lossy().to_string());
                }
            }
        } else {
            let default_watch_targets = ["src", "index.html", "Cargo.toml", "assets", "static"];
            for target in default_watch_targets {
                let path = project_root.join(target);
                if path.exists() {
                    args.push("--watch".to_string());
                    args.push(path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Always ignore the internal compiler cache directory to prevent inotify/FSEvents storms across all OSes.
    // Trunk matches against absolute file event paths, so canonicalize ensures prefix matching succeeds.
    let cache_dir = project_root.join(".cache");
    if cache_dir.exists() {
        let abs_cache = dunce::canonicalize(&cache_dir).unwrap_or(cache_dir);
        args.push("--ignore".to_string());
        args.push(abs_cache.to_string_lossy().to_string());
    }

    let target_dir = project_root.join("target");
    if target_dir.exists() {
        let abs_target = dunce::canonicalize(&target_dir).unwrap_or(target_dir);
        args.push("--ignore".to_string());
        args.push(abs_target.to_string_lossy().to_string());
    }

    if let Some(extra_ignores) = &config.watch_ignore {
        for ignore_path in extra_ignores {
            let full_path = if ignore_path.is_relative() {
                project_root.join(ignore_path)
            } else {
                ignore_path.clone()
            };
            if full_path.exists() {
                let abs_path = dunce::canonicalize(&full_path).unwrap_or(full_path);
                args.push("--ignore".to_string());
                args.push(abs_path.to_string_lossy().to_string());
            }
        }
    }

    args
}

pub fn merge_trunk_args(serve_args: &mut Vec<String>, launch_args: &[String]) {
    for flag in &["--port", "--address", "--dist"] {
        if launch_args
            .iter()
            .any(|a| a == flag || a.starts_with(&format!("{}=", flag)))
        {
            if let Some(pos) = serve_args.iter().position(|a| a == flag) {
                serve_args.remove(pos);
                if pos < serve_args.len() {
                    serve_args.remove(pos);
                }
            }
        }
    }
    for flag in &[
        "--open",
        "--no-autoreload",
        "--disable-address-lookup",
        "--release",
    ] {
        if launch_args
            .iter()
            .any(|a| a == flag || a.starts_with(&format!("{}=", flag)))
        {
            if let Some(pos) = serve_args.iter().position(|a| a == flag) {
                serve_args.remove(pos);
            }
        }
    }
    serve_args.extend(launch_args.iter().cloned());
}

pub fn run(env: &SessionExecutionContext, launch_args: &[String]) -> Result<()> {
    if env.build_target().target_platform() == TargetPlatform::Web {
        let has_trunk = env.project_root_dir().join("Trunk.toml").exists()
            || env.project_root_dir().join("index.html").exists();

        if has_trunk {
            let trunk_bin = which::which("trunk").map_err(|_| {
                anyhow::anyhow!(
                    "Running web applications requires `trunk` to be installed on PATH.\n\
                     Install via:\n\
                       cargo install trunk\n\
                     Or via your host package manager:\n\
                       Linux:   cargo install trunk\n\
                       macOS:   brew install trunk\n\
                       Windows: winget install trunk.trunk\n\
                     Or ensure Trunk.toml / index.html exists in the project root."
                )
            })?;

            // Pre-create .cache directory before starting Trunk so Trunk's internal canonicalize
            // never errors on a missing ignore directory across Linux, macOS, and Windows.
            let cache_dir = env.project_root_dir().join(".cache");
            let _ = std::fs::create_dir_all(&cache_dir);

            let web_cfg = &env.manifest_config().web;
            let address = web_cfg.address.as_deref().unwrap_or("127.0.0.1");
            let port = web_cfg.port.unwrap_or(8080);
            let is_release = env.build_target().optimization_profile() == TargetOpt::Release;

            let default_platform_dist = env
                .project_root_dir()
                .join(".cache")
                .join("web-static")
                .join("dist");
            let dist_dir =
                resolve_web_dist_dir(web_cfg, env.project_root_dir(), &default_platform_dist);
            let _ = std::fs::create_dir_all(&dist_dir);

            println!("Serving web application on http://{}:{}...", address, port);

            let mut serve_args =
                build_trunk_serve_args(web_cfg, is_release, env.project_root_dir(), &dist_dir);
            merge_trunk_args(&mut serve_args, launch_args);

            let mut trunk = std::process::Command::new(trunk_bin);
            trunk.current_dir(env.project_root_dir());
            trunk.env_remove("RUSTUP_TOOLCHAIN");
            trunk.args(&serve_args);

            let status = trunk
                .status()
                .context("Failed to spawn trunk serve process")?;
            anyhow::ensure!(
                status.success(),
                "trunk serve exited with nonzero exit code {:?}",
                status.code()
            );
            return Ok(());
        } else {
            anyhow::bail!(
                "Running web applications requires a Trunk.toml or index.html in the project root."
            );
        }
    }

    let out = env.executable_path();
    if let Some(device) = env.build_target().target_device() {
        device.run(env, &out, launch_args)?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_trunk_serve_args_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_dir = temp_dir.path().join(".cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let canonical_cache = dunce::canonicalize(&cache_dir).unwrap();

        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let dist_dir = temp_dir.path().join("dist");

        let config = WebPlatformConfig::default();
        let args = build_trunk_serve_args(&config, false, temp_dir.path(), &dist_dir);

        assert_eq!(args[0], "serve");
        assert!(args.contains(&"--address".to_string()));
        assert!(args.contains(&"127.0.0.1".to_string()));
        assert!(args.contains(&"--disable-address-lookup".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"8080".to_string()));
        assert!(args.contains(&"--dist".to_string()));
        assert!(args.contains(&dist_dir.to_string_lossy().to_string()));
        assert!(args.contains(&"--open".to_string()));
        assert!(!args.contains(&"--release".to_string()));
        assert!(!args.contains(&"--no-autoreload".to_string()));
        assert!(args.contains(&"--watch".to_string()));
        assert!(args.contains(&src_dir.to_string_lossy().to_string()));
        assert!(args.contains(&"--ignore".to_string()));
        assert!(args.contains(&canonical_cache.to_string_lossy().to_string()));
    }

    #[test]
    fn test_build_trunk_serve_args_release_and_custom_options() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_dir = temp_dir.path().join(".cache");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let custom_dir = temp_dir.path().join("assets_raw");
        std::fs::create_dir_all(&custom_dir).unwrap();
        let canonical_custom = dunce::canonicalize(&custom_dir).unwrap();

        let dist_dir = temp_dir.path().join("custom_dist");

        let config = WebPlatformConfig {
            port: Some(9090),
            open: Some(false),
            dist: None,
            address: Some("0.0.0.0".to_string()),
            watch: Some(vec![PathBuf::from("assets_raw")]),
            watch_ignore: Some(vec![PathBuf::from("assets_raw")]),
        };

        let args = build_trunk_serve_args(&config, true, temp_dir.path(), &dist_dir);

        assert_eq!(args[0], "serve");
        assert!(args.contains(&"--address".to_string()));
        assert!(args.contains(&"0.0.0.0".to_string()));
        assert!(args.contains(&"--disable-address-lookup".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"9090".to_string()));
        assert!(args.contains(&"--dist".to_string()));
        assert!(args.contains(&dist_dir.to_string_lossy().to_string()));
        assert!(!args.contains(&"--open".to_string()));
        assert!(args.contains(&"--release".to_string()));
        assert!(args.contains(&"--no-autoreload".to_string()));
        assert!(args.contains(&canonical_custom.to_string_lossy().to_string()));
    }

    #[test]
    fn test_resolve_web_dist_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let default_dist = temp_dir
            .path()
            .join(".cache")
            .join("web-static")
            .join("dist");

        let empty_cfg = WebPlatformConfig::default();
        assert_eq!(
            resolve_web_dist_dir(&empty_cfg, temp_dir.path(), &default_dist),
            default_dist
        );

        let custom_rel = WebPlatformConfig {
            dist: Some(PathBuf::from("custom_dist")),
            ..Default::default()
        };
        assert_eq!(
            resolve_web_dist_dir(&custom_rel, temp_dir.path(), &default_dist),
            temp_dir.path().join("custom_dist")
        );
    }

    #[test]
    fn test_merge_trunk_args_overrides_port_and_flags() {
        let mut base_args = vec![
            "serve".to_string(),
            "--address".to_string(),
            "127.0.0.1".to_string(),
            "--disable-address-lookup".to_string(),
            "--port".to_string(),
            "8080".to_string(),
            "--open".to_string(),
        ];

        let overrides = vec![
            "--port".to_string(),
            "8089".to_string(),
            "--no-autoreload".to_string(),
        ];

        merge_trunk_args(&mut base_args, &overrides);

        assert_eq!(base_args[0], "serve");
        assert!(base_args.contains(&"--disable-address-lookup".to_string()));
        assert!(!base_args.contains(&"8080".to_string()));
        assert!(base_args.contains(&"8089".to_string()));
        assert!(base_args.contains(&"--no-autoreload".to_string()));
    }
}
