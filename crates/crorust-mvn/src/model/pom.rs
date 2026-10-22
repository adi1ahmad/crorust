use crate::model::dependency::DependencyRequirement;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "project")]
pub struct ProjectObjectModel {
    pub version: Option<String>,
    pub packaging: Option<String>,
    #[serde(default)]
    pub dependencies: DependencyContainer,
    #[serde(rename = "dependencyManagement", default)]
    pub dependency_management: ManagedDependencyCatalog,
}

impl ProjectObjectModel {
    pub fn packaging(&self) -> &str {
        self.packaging.as_deref().unwrap_or("jar")
    }

    fn resolve_version_placeholder(&self, raw: &str) -> String {
        if let Some(proj_ver) = self.version.as_deref() {
            raw.replace("${project.version}", proj_ver)
                .replace("${pom.version}", proj_ver)
        } else {
            raw.to_string()
        }
    }

    pub fn dependencies(&self) -> Vec<DependencyRequirement> {
        let mut managed: HashMap<(Arc<str>, Arc<str>), String> = HashMap::new();
        for dep in &self.dependency_management.dependencies.dependencies {
            if let Some(v) = &dep.version {
                managed.insert(
                    (dep.group.clone(), dep.name.clone()),
                    self.resolve_version_placeholder(v),
                );
            }
        }

        let mut result = Vec::new();
        for mut dep in self.dependencies.dependencies.clone() {
            if dep.is_optional() {
                continue;
            }
            if dep.version.is_none() {
                if let Some(managed_ver) = managed.get(&(dep.group.clone(), dep.name.clone())) {
                    dep.version = Some(managed_ver.clone());
                }
            } else if let Some(v) = &dep.version {
                dep.version = Some(self.resolve_version_placeholder(v));
            }
            result.push(dep);
        }
        result
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "dependencyManagement")]
pub struct ManagedDependencyCatalog {
    #[serde(default)]
    pub dependencies: DependencyContainer,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "dependencies")]
pub struct DependencyContainer {
    #[serde(rename = "$value")]
    #[serde(default)]
    pub dependencies: Vec<DependencyRequirement>,
}
