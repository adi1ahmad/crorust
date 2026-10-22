use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpcContentTypesManifest {
    #[serde(rename = "Default", default)]
    pub defaults: Vec<DefaultContentType>,
    #[serde(rename = "Override", default)]
    pub overrides: Vec<OverrideContentType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultContentType {
    #[serde(rename = "@Extension")]
    pub extension: String,
    #[serde(rename = "@ContentType")]
    pub content_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverrideContentType {
    #[serde(rename = "@PartName")]
    pub part_name: String,
    #[serde(rename = "@ContentType")]
    pub content_type: String,
}

#[derive(Clone, Debug)]
pub struct OpcContentTypesBuilder {
    extensions: BTreeMap<String, String>,
    overrides: BTreeMap<String, String>,
}

pub fn guess_opc_content_type(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "xml" => "application/xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "exe" | "dll" => "application/x-msdownload",
        "json" => "application/json",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pri" | "bin" | "dat" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

impl Default for OpcContentTypesBuilder {
    fn default() -> Self {
        let mut builder = Self {
            extensions: BTreeMap::new(),
            overrides: BTreeMap::new(),
        };

        builder.register_extension("xml", "application/xml");
        builder.register_extension("png", "image/png");
        builder.register_extension("pri", "application/octet-stream");
        builder.register_extension("exe", "application/x-msdownload");
        builder.register_extension("dll", "application/x-msdownload");

        builder.register_override("/AppxManifest.xml", "application/vnd.ms-appx.manifest+xml");
        builder.register_override("/AppxBlockMap.xml", "application/vnd.ms-appx.blockmap+xml");
        builder.register_override("/AppxSignature.p7x", "application/vnd.ms-appx.signature");

        builder
    }
}

impl OpcContentTypesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_extension(&mut self, extension: &str, content_type: &str) {
        self.extensions
            .insert(extension.to_lowercase(), content_type.to_string());
    }

    pub fn register_override(&mut self, part_name: &str, content_type: &str) {
        let normalized = if part_name.starts_with('/') {
            part_name.to_string()
        } else {
            format!("/{}", part_name)
        };
        self.overrides.insert(normalized, content_type.to_string());
    }

    pub fn register_path(&mut self, path: &Path) {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            self.extensions
                .entry(ext_lower.clone())
                .or_insert_with(|| guess_opc_content_type(&ext_lower).to_string());
        }
    }

    pub fn finish(self) -> OpcContentTypesManifest {
        let defaults = self
            .extensions
            .into_iter()
            .map(|(extension, content_type)| DefaultContentType {
                extension,
                content_type,
            })
            .collect();

        let overrides = self
            .overrides
            .into_iter()
            .map(|(part_name, content_type)| OverrideContentType {
                part_name,
                content_type,
            })
            .collect();

        OpcContentTypesManifest {
            defaults,
            overrides,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guess_opc_content_type_standard_and_case_insensitive() {
        assert_eq!(guess_opc_content_type("xml"), "application/xml");
        assert_eq!(guess_opc_content_type("XML"), "application/xml");
        assert_eq!(guess_opc_content_type("png"), "image/png");
        assert_eq!(guess_opc_content_type("PNG"), "image/png");
        assert_eq!(guess_opc_content_type("jpg"), "image/jpeg");
        assert_eq!(guess_opc_content_type("jpeg"), "image/jpeg");
        assert_eq!(guess_opc_content_type("exe"), "application/x-msdownload");
        assert_eq!(guess_opc_content_type("dll"), "application/x-msdownload");
        assert_eq!(
            guess_opc_content_type("unknown_ext"),
            "application/octet-stream"
        );
    }
}
