use anyhow::Result;
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::BTreeMap,
    env::VarError,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CargoTomlConfig {
    pub env: Option<BTreeMap<String, CargoEnvVarSpec>>,
    pub build: Option<CargoBuildConfig>,
}

impl CargoTomlConfig {
    pub fn deserialize_cargo_toml(path: impl AsRef<Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn anchor_to_dir(&mut self, directory: &Path) {
        if let Some(b) = self.build.as_mut() {
            if b.target_dir.is_some() && b.config_dir.is_none() {
                b.config_dir = Some(directory.to_path_buf());
            }
        }
        if let Some(table) = self.env.as_mut() {
            for entry in table.values_mut() {
                if let CargoEnvVarSpec::Value {
                    relative: true,
                    config_dir: slot,
                    ..
                } = entry
                {
                    if slot.is_none() {
                        *slot = Some(directory.to_path_buf());
                    }
                }
            }
        }
    }

    pub fn merge_child(&mut self, child: CargoTomlConfig) {
        if let Some(child_b) = child.build {
            let self_b = self.build.get_or_insert_with(CargoBuildConfig::default);
            if child_b.target_dir.is_some() {
                self_b.target_dir = child_b.target_dir;
                self_b.config_dir = child_b.config_dir;
            }
        }
        if let Some(child_env) = child.env {
            let self_env = self.env.get_or_insert_with(BTreeMap::new);
            for (k, v) in child_env {
                self_env.insert(k, v);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoWorkspaceConfig {
    pub workspace: PathBuf,
    pub config: CargoTomlConfig,
}

impl CargoWorkspaceConfig {
    pub fn config_file_in_dir(dir: &Path) -> Option<PathBuf> {
        let base = dir.join(".cargo");
        let preferred = base.join("config.toml");
        if preferred.is_file() {
            return Some(preferred);
        }
        let legacy = base.join("config");
        if legacy.is_file() {
            return Some(legacy);
        }
        None
    }

    fn config_file_in_cargo_home(cargo_home: &Path) -> Option<PathBuf> {
        let preferred = cargo_home.join("config.toml");
        if preferred.is_file() {
            return Some(preferred);
        }
        let legacy = cargo_home.join("config");
        if legacy.is_file() {
            return Some(legacy);
        }
        None
    }

    pub fn new(workspace: PathBuf) -> Result<Self> {
        let manifest = Self::config_file_in_dir(&workspace)
            .unwrap_or_else(|| workspace.join(".cargo/config.toml"));
        let mut config = CargoTomlConfig::deserialize_cargo_toml(manifest)?;
        config.anchor_to_dir(&workspace);
        Ok(Self { config, workspace })
    }

    pub fn locate_workspace_cargo_config(workspace: impl AsRef<Path>) -> Result<Option<Self>> {
        let home = std::env::var_os("CARGO_HOME").map(PathBuf::from);
        Self::find_cargo_config_with_home(workspace, home.as_deref())
    }

    pub fn find_cargo_config_with_home(
        workspace: impl AsRef<Path>,
        cargo_home: Option<&Path>,
    ) -> Result<Option<Self>> {
        let canonical_ws = dunce::canonicalize(workspace.as_ref())?;

        let mut discovered = Vec::new();
        for dir in canonical_ws.ancestors() {
            if let Some(file) = Self::config_file_in_dir(dir) {
                discovered.push((dir.to_path_buf(), file));
            }
        }

        let mut home_entry = None;
        if let Some(ch) = cargo_home {
            if let Ok(canon_ch) = dunce::canonicalize(ch) {
                if let Some(file) = Self::config_file_in_cargo_home(&canon_ch) {
                    let present = discovered.iter().any(|(_, f)| f == &file);
                    if !present {
                        let anchor = canon_ch.parent().unwrap_or(&canon_ch).to_path_buf();
                        home_entry = Some((anchor, file));
                    }
                }
            }
        }

        if discovered.is_empty() && home_entry.is_none() {
            return Ok(None);
        }

        let mut aggregated = CargoTomlConfig::default();
        let mut target_dir = canonical_ws.clone();

        if let Some((anchor, file)) = home_entry {
            let mut cfg = CargoTomlConfig::deserialize_cargo_toml(&file)?;
            cfg.anchor_to_dir(&anchor);
            aggregated.merge_child(cfg);
            target_dir = anchor;
        }

        for (dir, file) in discovered.into_iter().rev() {
            let mut layer = CargoTomlConfig::deserialize_cargo_toml(&file)?;
            layer.anchor_to_dir(&dir);
            aggregated.merge_child(layer);
            target_dir = dir;
        }

        Ok(Some(Self {
            config: aggregated,
            workspace: target_dir,
        }))
    }

    pub fn resolved_target_dir(&self) -> Option<PathBuf> {
        let b = self.config.build.as_ref()?;
        let raw = b.target_dir.as_ref()?;
        let p = PathBuf::from(raw);
        if p.is_absolute() {
            Some(p)
        } else {
            let origin = b.config_dir.as_deref().unwrap_or(&self.workspace);
            Some(origin.join(p))
        }
    }

    pub fn apply_cargo_environment_overrides(&self) -> Result<()> {
        if let Some(table) = &self.config.env {
            for (var_name, spec) in table {
                if !matches!(spec, CargoEnvVarSpec::Value { force: true, .. })
                    && std::env::var_os(var_name).is_some()
                {
                    continue;
                }
                let resolved = spec.resolve_variable_expression(&self.workspace)?;
                std::env::set_var(var_name, resolved.as_ref());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct CargoBuildConfig {
    pub target_dir: Option<String>,
    #[serde(skip)]
    pub config_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CargoEnvVarSpec {
    String(String),
    Value {
        value: String,
        #[serde(default)]
        force: bool,
        #[serde(default)]
        relative: bool,
        #[serde(skip)]
        config_dir: Option<PathBuf>,
    },
}

impl CargoEnvVarSpec {
    pub fn resolve_variable_expression(
        &self,
        default_parent: impl AsRef<Path>,
    ) -> Result<Cow<'_, str>> {
        match self {
            Self::Value {
                value,
                relative: true,
                config_dir,
                ..
            } => {
                let base = config_dir
                    .as_deref()
                    .unwrap_or_else(|| default_parent.as_ref());
                let full = base.join(value);
                let stringified = full
                    .into_os_string()
                    .into_string()
                    .map_err(VarError::NotUnicode)?;
                Ok(Cow::Owned(stringified))
            }
            Self::String(raw) | Self::Value { value: raw, .. } => Ok(Cow::Borrowed(raw.as_str())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cargo_env_spec_deserialization_and_resolution() {
        let sample = r#"
[env]
CRORUST_STATIC_FLAG = "enabled"
CRORUST_FORCED_VAR = { value = "immutable", force = true }
CRORUST_REL_ASSET = { value = "res/assets", relative = true }
"#;
        let parsed: CargoTomlConfig = toml::from_str(sample).expect("valid toml");
        let env_table = parsed.env.expect("env section present");

        assert_eq!(
            env_table.get("CRORUST_STATIC_FLAG"),
            Some(&CargoEnvVarSpec::String("enabled".to_string()))
        );

        let resolved_rel = env_table
            .get("CRORUST_REL_ASSET")
            .unwrap()
            .resolve_variable_expression(Path::new("/workspace/root"))
            .unwrap();
        assert_eq!(
            Path::new(resolved_rel.as_ref()),
            Path::new("/workspace/root/res/assets")
        );
    }

    #[test]
    fn verify_cargo_process_env_injection_precedence() {
        let manifest = r#"
[env]
CRORUST_TEST_REGULAR = "initial_val"
CRORUST_TEST_FORCED = { value = "forced_val", force = true }
"#;
        let mut cfg =
            CargoTomlConfig::deserialize_cargo_toml(tempfile::NamedTempFile::new().unwrap().path())
                .unwrap_or_default();
        cfg.merge_child(toml::from_str(manifest).unwrap());

        let localized = CargoWorkspaceConfig {
            config: cfg,
            workspace: PathBuf::from("/mock/ws"),
        };

        std::env::set_var("CRORUST_TEST_REGULAR", "process_existing");
        std::env::set_var("CRORUST_TEST_FORCED", "process_existing");

        localized
            .apply_cargo_environment_overrides()
            .expect("env vars set");

        assert_eq!(
            std::env::var("CRORUST_TEST_REGULAR").unwrap(),
            "process_existing"
        );

        assert_eq!(std::env::var("CRORUST_TEST_FORCED").unwrap(), "forced_val");
    }

    #[test]
    fn verify_relative_path_variable_resolution() {
        let spec = CargoEnvVarSpec::Value {
            value: "submodules/core".to_string(),
            force: false,
            relative: true,
            config_dir: Some(PathBuf::from("/opt/project")),
        };
        let resolved = spec
            .resolve_variable_expression(Path::new("/fallback"))
            .unwrap();
        assert_eq!(
            Path::new(resolved.as_ref()),
            Path::new("/opt/project/submodules/core")
        );
    }

    #[test]
    fn test_hierarchical_cargo_config_merge_and_relative_anchoring() {
        let temp = tempfile::TempDir::new().unwrap();
        let root_dir = dunce::canonicalize(temp.path()).unwrap();
        let ws_dir = root_dir.join("workspace");
        let crate_dir = ws_dir.join("crates").join("mobile");
        std::fs::create_dir_all(root_dir.join(".cargo")).unwrap();
        std::fs::create_dir_all(ws_dir.join(".cargo")).unwrap();
        std::fs::create_dir_all(crate_dir.join(".cargo")).unwrap();

        std::fs::write(
            root_dir.join(".cargo/config.toml"),
            r#"
[build]
target-dir = "custom-root-target"

[env]
SHARED_VAR = "from_root"
OVERRIDDEN_VAR = "root_value"
ROOT_REL_PATH = { value = "tools/root_bin", relative = true, force = true }
"#,
        )
        .unwrap();

        std::fs::write(
            ws_dir.join(".cargo/config"),
            r#"
[env]
OVERRIDDEN_VAR = "ws_value"
WS_REL_PATH = { value = "sdk/ws_ndk", relative = true, force = true }
"#,
        )
        .unwrap();

        std::fs::write(
            crate_dir.join(".cargo/config.toml"),
            r#"
[env]
OVERRIDDEN_VAR = { value = "crate_value", force = true }
CRATE_REL_PATH = { value = "assets/icon.png", relative = true, force = true }
"#,
        )
        .unwrap();

        let localized = CargoWorkspaceConfig::find_cargo_config_with_home(&crate_dir, None)
            .unwrap()
            .expect("Should find merged config");

        assert_eq!(
            localized.resolved_target_dir().unwrap(),
            root_dir.join("custom-root-target")
        );

        let env_map = localized.config.env.as_ref().unwrap();
        assert_eq!(
            env_map
                .get("SHARED_VAR")
                .unwrap()
                .resolve_variable_expression(&localized.workspace)
                .unwrap(),
            "from_root"
        );
        assert_eq!(
            env_map
                .get("OVERRIDDEN_VAR")
                .unwrap()
                .resolve_variable_expression(&localized.workspace)
                .unwrap(),
            "crate_value"
        );
        assert_eq!(
            PathBuf::from(
                env_map
                    .get("ROOT_REL_PATH")
                    .unwrap()
                    .resolve_variable_expression(&localized.workspace)
                    .unwrap()
                    .as_ref()
            ),
            root_dir.join("tools/root_bin")
        );
        assert_eq!(
            PathBuf::from(
                env_map
                    .get("WS_REL_PATH")
                    .unwrap()
                    .resolve_variable_expression(&localized.workspace)
                    .unwrap()
                    .as_ref()
            ),
            ws_dir.join("sdk/ws_ndk")
        );
        assert_eq!(
            PathBuf::from(
                env_map
                    .get("CRATE_REL_PATH")
                    .unwrap()
                    .resolve_variable_expression(&localized.workspace)
                    .unwrap()
                    .as_ref()
            ),
            crate_dir.join("assets/icon.png")
        );
    }
}
