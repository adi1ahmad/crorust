use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DarwinPropertyList {
    pub bundle_identifier: Option<String>,

    pub bundle_name: Option<String>,

    pub display_name: Option<String>,

    pub executable_name: Option<String>,

    pub package_type: Option<String>,

    pub short_version_string: Option<String>,

    pub bundle_version: Option<String>,

    pub icon_file: Option<String>,

    pub icon_files: Vec<String>,

    pub supported_platforms: Vec<String>,

    pub development_region: Option<String>,

    pub min_macos_version: Option<String>,

    pub min_ios_version: Option<String>,

    pub requires_ios: Option<bool>,

    pub high_resolution_capable: Option<bool>,

    pub custom_keys: BTreeMap<String, plist::Value>,
}

impl DarwinPropertyList {
    pub fn new_macos_app(
        bundle_id: impl Into<String>,
        name: impl Into<String>,
        executable: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        let v = version.into();
        Self {
            bundle_identifier: Some(bundle_id.into()),
            bundle_name: Some(name.into()),
            display_name: None,
            executable_name: Some(executable.into()),
            package_type: Some("APPL".to_string()),
            short_version_string: Some(v.clone()),
            bundle_version: Some(v),
            icon_file: Some("AppIcon".to_string()),
            icon_files: Vec::new(),
            supported_platforms: vec!["MacOSX".to_string()],
            development_region: Some("en".to_string()),
            min_macos_version: Some("11.0".to_string()),
            min_ios_version: None,
            requires_ios: None,
            high_resolution_capable: Some(true),
            custom_keys: BTreeMap::new(),
        }
    }

    pub fn new_ios_app(
        bundle_id: impl Into<String>,
        name: impl Into<String>,
        executable: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        let v = version.into();
        let mut custom = BTreeMap::new();
        custom.insert(
            "UIDeviceFamily".to_string(),
            plist::Value::Array(vec![
                plist::Value::Integer(1.into()),
                plist::Value::Integer(2.into()),
            ]),
        );
        custom.insert(
            "UILaunchStoryboardName".to_string(),
            plist::Value::String(String::new()),
        );
        custom.insert(
            "UIRequiredDeviceCapabilities".to_string(),
            plist::Value::Array(vec![plist::Value::String("arm64".to_string())]),
        );

        Self {
            bundle_identifier: Some(bundle_id.into()),
            bundle_name: Some(name.into()),
            display_name: None,
            executable_name: Some(executable.into()),
            package_type: Some("APPL".to_string()),
            short_version_string: Some(v.clone()),
            bundle_version: Some(v),
            icon_file: None,
            icon_files: Vec::new(),
            supported_platforms: vec!["iPhoneOS".to_string()],
            development_region: Some("en".to_string()),
            min_macos_version: None,
            min_ios_version: Some("14.0".to_string()),
            requires_ios: Some(true),
            high_resolution_capable: None,
            custom_keys: custom,
        }
    }

    pub fn to_dictionary(&self) -> plist::Dictionary {
        let mut dict = plist::Dictionary::new();

        if let Some(val) = &self.bundle_identifier {
            dict.insert(
                "CFBundleIdentifier".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.bundle_name {
            dict.insert("CFBundleName".into(), plist::Value::String(val.clone()));
        }
        if let Some(val) = &self.display_name {
            dict.insert(
                "CFBundleDisplayName".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.executable_name {
            dict.insert(
                "CFBundleExecutable".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.package_type {
            dict.insert(
                "CFBundlePackageType".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.short_version_string {
            dict.insert(
                "CFBundleShortVersionString".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.bundle_version {
            dict.insert("CFBundleVersion".into(), plist::Value::String(val.clone()));
        }
        if let Some(val) = &self.icon_file {
            dict.insert("CFBundleIconFile".into(), plist::Value::String(val.clone()));
        }
        if !self.icon_files.is_empty() {
            let files = self
                .icon_files
                .iter()
                .map(|s| plist::Value::String(s.clone()))
                .collect();
            dict.insert("CFBundleIconFiles".into(), plist::Value::Array(files));
        }
        if !self.supported_platforms.is_empty() {
            let platforms = self
                .supported_platforms
                .iter()
                .map(|s| plist::Value::String(s.clone()))
                .collect();
            dict.insert(
                "CFBundleSupportedPlatforms".into(),
                plist::Value::Array(platforms),
            );
        }
        if let Some(val) = &self.development_region {
            dict.insert(
                "CFBundleDevelopmentRegion".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.min_macos_version {
            dict.insert(
                "LSMinimumSystemVersion".into(),
                plist::Value::String(val.clone()),
            );
        }
        if let Some(val) = &self.min_ios_version {
            dict.insert("MinimumOSVersion".into(), plist::Value::String(val.clone()));
        }
        if let Some(val) = self.requires_ios {
            dict.insert("LSRequiresIPhoneOS".into(), plist::Value::Boolean(val));
        }
        if let Some(val) = self.high_resolution_capable {
            dict.insert("NSHighResolutionCapable".into(), plist::Value::Boolean(val));
        }

        for (k, v) in &self.custom_keys {
            dict.insert(k.clone(), v.clone());
        }

        dict
    }

    pub fn from_dictionary(dict: &plist::Dictionary) -> Self {
        let mut props = Self::default();
        for (k, v) in dict {
            match k.as_str() {
                "CFBundleIdentifier" => {
                    props.bundle_identifier = v.as_string().map(|s| s.to_string())
                }
                "CFBundleName" => props.bundle_name = v.as_string().map(|s| s.to_string()),
                "CFBundleDisplayName" => props.display_name = v.as_string().map(|s| s.to_string()),
                "CFBundleExecutable" => {
                    props.executable_name = v.as_string().map(|s| s.to_string())
                }
                "CFBundlePackageType" => props.package_type = v.as_string().map(|s| s.to_string()),
                "CFBundleShortVersionString" => {
                    props.short_version_string = v.as_string().map(|s| s.to_string())
                }
                "CFBundleVersion" => props.bundle_version = v.as_string().map(|s| s.to_string()),
                "CFBundleIconFile" => props.icon_file = v.as_string().map(|s| s.to_string()),
                "CFBundleIconFiles" => {
                    if let Some(arr) = v.as_array() {
                        props.icon_files = arr
                            .iter()
                            .filter_map(|x| x.as_string().map(|s| s.to_string()))
                            .collect();
                    }
                }
                "CFBundleSupportedPlatforms" => {
                    if let Some(arr) = v.as_array() {
                        props.supported_platforms = arr
                            .iter()
                            .filter_map(|x| x.as_string().map(|s| s.to_string()))
                            .collect();
                    }
                }
                "CFBundleDevelopmentRegion" => {
                    props.development_region = v.as_string().map(|s| s.to_string())
                }
                "LSMinimumSystemVersion" => {
                    props.min_macos_version = v.as_string().map(|s| s.to_string())
                }
                "MinimumOSVersion" => props.min_ios_version = v.as_string().map(|s| s.to_string()),
                "LSRequiresIPhoneOS" => props.requires_ios = v.as_boolean(),
                "NSHighResolutionCapable" => props.high_resolution_capable = v.as_boolean(),
                _ => {
                    props.custom_keys.insert(k.clone(), v.clone());
                }
            }
        }
        props
    }

    pub fn is_ios(&self) -> bool {
        self.requires_ios.unwrap_or(false)
            || self
                .supported_platforms
                .iter()
                .any(|p| p.eq_ignore_ascii_case("iphoneos"))
    }

    pub fn set_default(&mut self, key: impl Into<String>, value: impl Into<plist::Value>) {
        let k = key.into();
        self.custom_keys.entry(k).or_insert_with(|| value.into());
    }

    pub fn set_custom_property(&mut self, key: impl Into<String>, value: impl Into<plist::Value>) {
        self.custom_keys.insert(key.into(), value.into());
    }

    pub fn to_xml_bytes(&self) -> Result<Vec<u8>> {
        let dict = self.to_dictionary();
        let mut out = Vec::new();
        plist::to_writer_xml(&mut out, &dict)
            .context("Failed to serialize Darwin property list to XML format")?;
        Ok(out)
    }

    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory for {}", path.display())
            })?;
        }
        let bytes = self.to_xml_bytes()?;
        std::fs::write(path, bytes)
            .with_context(|| format!("Failed to write Info.plist to {}", path.display()))?;
        Ok(())
    }

    pub fn read_from_file(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open Info.plist at {}", path.display()))?;
        let dict: plist::Dictionary = plist::from_reader_xml(file)
            .with_context(|| format!("Failed to parse Info.plist at {}", path.display()))?;
        Ok(Self::from_dictionary(&dict))
    }

    pub fn cf_bundle_identifier(&self) -> Option<&str> {
        self.bundle_identifier.as_deref()
    }

    pub fn cf_bundle_name(&self) -> Option<&str> {
        self.bundle_name.as_deref()
    }

    pub fn cf_bundle_executable(&self) -> Option<&str> {
        self.executable_name.as_deref()
    }
}

impl Serialize for DarwinPropertyList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let dict = self.to_dictionary();
        dict.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DarwinPropertyList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dict = plist::Dictionary::deserialize(deserializer)?;
        Ok(Self::from_dictionary(&dict))
    }
}
