use anyhow::{bail, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceSymbols {
    pub types: Vec<String>,
    pub id_map: BTreeMap<(String, String), u32>,
    pub string_values: BTreeMap<String, String>,
    pub file_resources: BTreeMap<(String, String), String>,
}

impl ResourceSymbols {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_symbol(&mut self, res_type: &str, name: &str, id: u32) {
        if !self.types.contains(&res_type.to_string()) {
            self.types.push(res_type.to_string());
        }
        self.id_map
            .insert((res_type.to_string(), name.to_string()), id);
    }

    pub fn get_attribute_id(&self, name: &str) -> Option<u32> {
        self.id_map
            .get(&("attr".to_string(), name.to_string()))
            .copied()
    }

    pub fn get_resource_id(&self, res_type: &str, name: &str) -> Option<u32> {
        self.id_map
            .get(&(res_type.to_string(), name.to_string()))
            .copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceIdentifierReference<'a> {
    pub name: &'a str,
    pub ty: &'a str,
    pub package: Option<&'a str>,
}

impl<'a> ResourceIdentifierReference<'a> {
    pub fn attr(name: &'a str) -> Self {
        Self {
            package: Some("android"),
            ty: "attr",
            name,
        }
    }

    pub fn id(name: &'a str) -> Self {
        Self {
            package: Some("android"),
            ty: "id",
            name,
        }
    }

    pub fn parse(orig: &'a str) -> Result<Self> {
        let is_attr = orig.starts_with('?');
        let s = if let Some(stripped) = orig.strip_prefix('@') {
            stripped.strip_prefix('+').unwrap_or(stripped)
        } else if let Some(stripped) = orig.strip_prefix('?') {
            stripped
        } else {
            bail!("invalid reference {orig}: expected `@` or `?`");
        };

        let (descr, name) = if let Some((d, n)) = s.split_once('/') {
            (d, n)
        } else if is_attr {
            if let Some((pkg, name)) = s.split_once(':') {
                return Ok(Self {
                    package: Some(pkg),
                    ty: "attr",
                    name,
                });
            } else {
                return Ok(Self {
                    package: None,
                    ty: "attr",
                    name: s,
                });
            }
        } else {
            bail!("invalid reference {orig}: expected `/`");
        };

        let (package, ty) = if let Some((package, ty)) = descr.split_once(':') {
            (Some(package), ty)
        } else {
            (None, descr)
        };

        Ok(Self { name, ty, package })
    }
}
