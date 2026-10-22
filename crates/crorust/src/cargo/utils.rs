use super::config::CargoTomlConfig;
use super::manifest::CargoPackageManifest;
use anyhow::{bail, Context, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn discover_rust_source_stems(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            if p.is_file() && p.extension() == Some(OsStr::new("rs")) {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    Ok(names)
}

pub fn canonicalize_safe_path(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        std::env::current_dir().context("Failed to obtain current working directory")
    } else {
        dunce::canonicalize(path).with_context(|| {
            format!(
                "Failed to canonicalize filesystem path '{}'",
                path.display()
            )
        })
    }
}

pub fn locate_package_manifest_in_workspace(
    (ws_path, ws_manifest): &(PathBuf, CargoPackageManifest),
    (candidate_path, candidate_manifest): (PathBuf, CargoPackageManifest),
    requested_package: Option<&str>,
) -> Result<(PathBuf, CargoPackageManifest)> {
    let candidate_dir = candidate_path
        .parent()
        .context("Candidate manifest path has no parent directory")?;
    let ws_dir = ws_path
        .parent()
        .context("Workspace root manifest path has no parent directory")?;

    let subpath = match candidate_dir.strip_prefix(ws_dir) {
        Ok(sub) => sub,
        Err(_) => return Ok((candidate_path, candidate_manifest)),
    };

    let members = ws_manifest.resolve_workspace_members(ws_dir)?;
    if ws_manifest.is_excluded(ws_dir, candidate_dir)? {
        return Ok((candidate_path, candidate_manifest));
    }

    if ws_path != &candidate_path && !members.contains_key(candidate_dir) {
        bail!(
            "Package at '{}' is inside workspace directory '{}' but is not listed in workspace members.\n\
             Add '{}' to workspace.members or workspace.exclude in {}",
            candidate_path.display(),
            ws_path.display(),
            subpath.display(),
            ws_path.display()
        );
    }

    match requested_package {
        Some(target_name) => {
            let is_match = |name: &str| -> bool {
                name == target_name || name.replace('-', "_") == target_name.replace('-', "_")
            };

            if let Some(pkg) = &ws_manifest.package {
                if is_match(&pkg.name) {
                    return Ok((ws_path.clone(), ws_manifest.clone()));
                }
            }

            for (_, (m_path, m_manifest)) in members {
                if let Some(pkg) = &m_manifest.package {
                    if is_match(&pkg.name) {
                        return Ok((m_path, m_manifest));
                    }
                }
            }

            bail!(
                "Requested package '{}' not found among members in workspace '{}'",
                target_name,
                ws_path.display()
            )
        }
        None => {
            if candidate_manifest.package.is_some() {
                Ok((candidate_path, candidate_manifest))
            } else {
                let ws_section = ws_manifest
                    .workspace
                    .as_ref()
                    .context("Missing [workspace] section in root manifest")?;

                if ws_section.default_members.len() == 1 {
                    let default_pkg = &ws_section.default_members[0];
                    for (path, m) in members.values() {
                        if m.package.as_ref().map(|p| &p.name) == Some(default_pkg) {
                            return Ok((path.clone(), m.clone()));
                        }
                    }
                }

                if members.len() == 1 {
                    let (_, (path, m)) = members.into_iter().next().unwrap();
                    return Ok((path, m));
                }

                let mut names: Vec<String> = members
                    .values()
                    .filter_map(|(_, m)| m.package.as_ref().map(|p| p.name.clone()))
                    .collect();
                names.sort();

                bail!(
                    "Virtual workspace root detected at '{}'. Please select a target package with -p.\n\
                     Available workspace packages: {}",
                    ws_path.display(),
                    if names.is_empty() {
                        "none found".to_string()
                    } else {
                        names.join(", ")
                    }
                )
            }
        }
    }
}

pub fn discover_available_binaries(
    package_root: &Path,
    manifest: &CargoPackageManifest,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for b in &manifest.bin {
        if !names.contains(&b.name) {
            names.push(b.name.clone());
        }
    }

    if let Some(pkg) = &manifest.package {
        if package_root.join("src").join("main.rs").is_file() && !names.contains(&pkg.name) {
            names.push(pkg.name.clone());
        }
    }

    let bin_subdir = package_root.join("src").join("bin");
    if bin_subdir.is_dir() {
        for file_stem in discover_rust_source_stems(&bin_subdir)? {
            if !names.contains(&file_stem) {
                names.push(file_stem);
            }
        }
    }

    Ok(names)
}

pub fn locate_nearest_manifest(start_dir: &Path) -> Result<(PathBuf, CargoPackageManifest)> {
    let canonical = canonicalize_safe_path(start_dir)?;
    let found = canonical
        .ancestors()
        .map(|dir| dir.join("Cargo.toml"))
        .find(|candidate| candidate.is_file())
        .context("Could not find Cargo.toml in current directory or any parent directories")?;

    let manifest = CargoPackageManifest::deserialize_from_toml_file(&found)?;
    Ok((found, manifest))
}

pub fn locate_nearest_workspace_root(
    start_dir: &Path,
) -> Result<Option<(PathBuf, CargoPackageManifest)>> {
    let canonical = canonicalize_safe_path(start_dir)?;
    for candidate in canonical.ancestors().map(|dir| dir.join("Cargo.toml")) {
        if candidate.is_file() {
            let manifest = CargoPackageManifest::deserialize_from_toml_file(&candidate)?;
            if manifest.workspace.is_some() {
                return Ok(Some((candidate, manifest)));
            }
        }
    }
    Ok(None)
}

pub fn resolve_target_dir_name(config: Option<&CargoTomlConfig>) -> Result<String> {
    if let Some(cfg) = config {
        if let Some(build) = &cfg.build {
            if let Some(custom_dir) = &build.target_dir {
                return Ok(custom_dir.clone());
            }
        }
    }
    Ok(".cache/target".to_string())
}
