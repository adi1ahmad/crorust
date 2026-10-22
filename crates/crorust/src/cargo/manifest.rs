use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum WorkspaceInheritable<T> {
    Value(T),
    Inherited { workspace: bool },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CargoBinTarget {
    pub name: String,
    pub path: Option<PathBuf>,
    #[serde(default, rename = "required-features")]
    pub required_features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CargoLibTarget {
    pub name: Option<String>,
    pub path: Option<PathBuf>,
    #[serde(default, rename = "crate-type")]
    pub crate_type: Vec<String>,
    #[serde(default, rename = "required-features")]
    pub required_features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoPackageManifest {
    pub workspace: Option<CargoWorkspaceSection>,
    pub package: Option<CargoPackageSection>,
    #[serde(default)]
    pub bin: Vec<CargoBinTarget>,
    pub lib: Option<CargoLibTarget>,
}

impl CargoPackageManifest {
    pub fn deserialize_from_toml_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest at {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to deserialize TOML from {}", path.display()))
    }

    pub fn resolve_workspace_members(
        &self,
        workspace_root: &Path,
    ) -> Result<HashMap<PathBuf, (PathBuf, CargoPackageManifest)>> {
        let ws = self
            .workspace
            .as_ref()
            .context("Manifest does not contain a `[workspace]` configuration")?;
        let root = super::utils::canonicalize_safe_path(workspace_root)?;
        let mut resolved = HashMap::new();

        for glob_pat in &ws.members {
            let full_glob = root.join(glob_pat);
            let glob_str = full_glob
                .to_str()
                .context("Workspace member glob contains invalid UTF-8")?;

            for entry in glob::glob(glob_str)? {
                let member_dir = entry?;
                let toml_path = member_dir.join("Cargo.toml");
                if toml_path.is_file() {
                    let member_manifest =
                        CargoPackageManifest::deserialize_from_toml_file(&toml_path)?;
                    if member_manifest.workspace.is_some() {
                        bail!(
                            "Nested workspaces are not permitted at {}",
                            toml_path.display()
                        );
                    }
                    if member_manifest.package.is_none() {
                        bail!(
                            "Workspace member at {} must contain a [package] section",
                            toml_path.display()
                        );
                    }
                    resolved.insert(member_dir, (toml_path, member_manifest));
                }
            }
        }

        Ok(resolved)
    }

    pub fn resolve_nonvirtual_package(
        self,
        manifest_path: PathBuf,
        requested_name: Option<&str>,
    ) -> Result<(PathBuf, Self)> {
        if self.workspace.is_some() && self.package.is_none() {
            bail!(
                "Cannot build virtual workspace root directly at {}. Specify a package via -p.",
                manifest_path.display()
            );
        }

        if let Some(pkg) = &self.package {
            if let Some(target_name) = requested_name {
                let is_match = pkg.name == target_name
                    || pkg.name.replace('-', "_") == target_name.replace('-', "_");
                if is_match {
                    Ok((manifest_path, self))
                } else {
                    bail!(
                        "Package '{}' does not match manifest at {}",
                        target_name,
                        manifest_path.display()
                    );
                }
            } else {
                Ok((manifest_path, self))
            }
        } else {
            bail!(
                "Manifest at {} is missing a [package] section",
                manifest_path.display()
            );
        }
    }

    pub fn primary_bin_name(&self) -> Option<&str> {
        if let Some(pkg) = &self.package {
            if let Some(def_run) = &pkg.default_run {
                return Some(def_run.as_str());
            }
            if let Some(target) = self.bin.iter().find(|t| t.name == pkg.name) {
                return Some(&target.name);
            }
        }
        self.bin.first().map(|t| t.name.as_str())
    }

    pub fn find_bin(&self, name: &str) -> Option<&CargoBinTarget> {
        self.bin.iter().find(|t| t.name == name)
    }

    pub fn has_cdylib(&self) -> bool {
        self.lib
            .as_ref()
            .map(|l| {
                l.crate_type
                    .iter()
                    .any(|ct| ct == "cdylib" || ct == "dylib")
            })
            .unwrap_or(false)
    }

    pub fn is_excluded(&self, workspace_root: &Path, package_dir: &Path) -> Result<bool> {
        let Some(ws) = &self.workspace else {
            return Ok(false);
        };
        let ws_root = super::utils::canonicalize_safe_path(workspace_root)?;
        let pkg_root = super::utils::canonicalize_safe_path(package_dir)?;

        for pat in &ws.exclude {
            let full_glob = ws_root.join(pat);
            if let Some(glob_str) = full_glob.to_str() {
                for candidate in glob::glob(glob_str)?.flatten() {
                    if let Ok(canon) = super::utils::canonicalize_safe_path(&candidate) {
                        if canon == pkg_root {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoWorkspaceSection {
    #[serde(default)]
    pub default_members: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    pub package: Option<CargoWorkspacePackageMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoWorkspacePackageMetadata {
    pub description: Option<String>,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoPackageSection {
    pub name: String,
    pub version: WorkspaceInheritable<String>,
    pub description: Option<WorkspaceInheritable<String>>,
    #[serde(rename = "default-run")]
    pub default_run: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bins_and_default_run() {
        let toml_str = r#"
            [package]
            name = "my-crate"
            version = "0.1.0"
            default-run = "custom-client"

            [[bin]]
            name = "custom-server"
            path = "src/bin/server.rs"

            [[bin]]
            name = "custom-client"
            path = "src/bin/client.rs"
        "#;
        let manifest: CargoPackageManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.bin.len(), 2);
        assert_eq!(manifest.bin[0].name, "custom-server");
        assert_eq!(manifest.bin[1].name, "custom-client");
        assert_eq!(manifest.primary_bin_name(), Some("custom-client"));
        assert!(manifest.find_bin("custom-server").is_some());
        assert!(manifest.find_bin("nonexistent").is_none());
    }

    #[test]
    fn test_primary_bin_fallback_to_matching_name() {
        let toml_str = r#"
            [package]
            name = "my-app"
            version = "0.1.0"

            [[bin]]
            name = "auxiliary"
            path = "src/aux.rs"

            [[bin]]
            name = "my-app"
            path = "src/main.rs"
        "#;
        let manifest: CargoPackageManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.primary_bin_name(), Some("my-app"));
    }

    #[test]
    fn test_primary_bin_fallback_to_first() {
        let toml_str = r#"
            [package]
            name = "core-crate"
            version = "0.1.0"

            [[bin]]
            name = "standalone"
            path = "src/main.rs"
        "#;
        let manifest: CargoPackageManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.primary_bin_name(), Some("standalone"));
    }
}
