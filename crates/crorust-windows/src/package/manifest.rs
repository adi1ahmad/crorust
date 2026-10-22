use anyhow::Result;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

fn windows_appx_default_namespace() -> String {
    String::from("http://schemas.microsoft.com/appx/manifest/foundation/windows10")
}

fn windows_appx_uap_namespace() -> String {
    String::from("http://schemas.microsoft.com/appx/manifest/uap/windows10")
}

fn windows_appx_rescap_namespace() -> String {
    String::from(
        "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities",
    )
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename = "Package")]
pub struct WindowsPackageManifestDocument {
    #[serde(
        rename(serialize = "@xmlns"),
        default = "windows_appx_default_namespace"
    )]
    pub ns: String,
    #[serde(
        rename(serialize = "@xmlns:uap"),
        default = "windows_appx_uap_namespace"
    )]
    pub ns_uap: String,
    #[serde(
        rename(serialize = "@xmlns:rescap"),
        default = "windows_appx_rescap_namespace"
    )]
    pub ns_rescap: String,
    #[serde(rename(serialize = "Identity"), default)]
    pub identity: PackageIdentityDescriptor,
    #[serde(rename(serialize = "Properties"), default)]
    pub properties: PackageDisplayProperties,
    #[serde(rename(serialize = "Resources"), default)]
    pub resources: ManifestResourceCollection,
    #[serde(rename(serialize = "Dependencies"), default)]
    pub dependencies: ManifestDependencyCollection,
    #[serde(
        rename(serialize = "Capabilities"),
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_manifest_capabilities"
    )]
    pub capabilities: Vec<ManifestCapability>,
    #[serde(rename(serialize = "Applications"), default)]
    pub applications: ManifestApplicationCollection,
}

impl Default for WindowsPackageManifestDocument {
    fn default() -> Self {
        Self {
            ns: windows_appx_default_namespace(),
            ns_uap: windows_appx_uap_namespace(),
            ns_rescap: windows_appx_rescap_namespace(),
            identity: PackageIdentityDescriptor::default(),
            properties: PackageDisplayProperties::default(),
            resources: ManifestResourceCollection::default(),
            dependencies: ManifestDependencyCollection::default(),
            capabilities: Vec::new(),
            applications: ManifestApplicationCollection::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PackageIdentityDescriptor {
    #[serde(rename(serialize = "@Name"), skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@Version"),
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<String>,
    #[serde(
        rename(serialize = "@Publisher"),
        skip_serializing_if = "Option::is_none"
    )]
    pub publisher: Option<String>,
    #[serde(
        rename(serialize = "@ProcessorArchitecture"),
        skip_serializing_if = "Option::is_none"
    )]
    pub processor_architecture: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PackageDisplayProperties {
    #[serde(rename(serialize = "DisplayName"))]
    pub display_name: Option<String>,
    #[serde(rename(serialize = "PublisherDisplayName"))]
    pub publisher_display_name: Option<String>,
    #[serde(rename(serialize = "Logo"))]
    pub logo: Option<String>,
    #[serde(
        rename(serialize = "Description"),
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestResourceCollection {
    #[serde(
        rename(serialize = "Resource"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub resource: Vec<ManifestResourceLanguage>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestResourceLanguage {
    #[serde(rename(serialize = "@Language"), alias = "@Language")]
    pub language: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestDependencyCollection {
    #[serde(
        rename(serialize = "TargetDeviceFamily"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub target_device_family: Vec<TargetPlatformFamilyConstraint>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TargetPlatformFamilyConstraint {
    #[serde(rename(serialize = "@Name"))]
    pub name: String,
    #[serde(rename(serialize = "@MinVersion"))]
    pub min_version: String,
    #[serde(rename(serialize = "@MaxVersionTested"))]
    pub max_version: String,
}

impl Default for TargetPlatformFamilyConstraint {
    fn default() -> Self {
        Self {
            name: "Windows.Desktop".to_string(),
            min_version: "10.0.0.0".to_string(),
            max_version: "10.0.22621.0".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum ManifestCapability {
    #[serde(rename(deserialize = "capability"))]
    Capability { name: String },
    #[serde(rename(deserialize = "restricted"))]
    Restricted { name: String },
    #[serde(rename(deserialize = "device"))]
    Device { name: String },
}

fn serialize_manifest_capabilities<S>(
    caps: &[ManifestCapability],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct CapItem<'a> {
        #[serde(rename = "@Name")]
        name: &'a str,
    }

    #[derive(Serialize)]
    struct CapGroup<'a> {
        #[serde(rename = "Capability", default, skip_serializing_if = "Vec::is_empty")]
        standard: Vec<CapItem<'a>>,
        #[serde(
            rename = "rescap:Capability",
            default,
            skip_serializing_if = "Vec::is_empty"
        )]
        restricted: Vec<CapItem<'a>>,
        #[serde(
            rename = "DeviceCapability",
            default,
            skip_serializing_if = "Vec::is_empty"
        )]
        device: Vec<CapItem<'a>>,
    }

    let mut standard = Vec::new();
    let mut restricted = Vec::new();
    let mut device = Vec::new();

    for cap in caps {
        match cap {
            ManifestCapability::Capability { name } => standard.push(CapItem { name }),
            ManifestCapability::Restricted { name } => restricted.push(CapItem { name }),
            ManifestCapability::Device { name } => device.push(CapItem { name }),
        }
    }

    CapGroup {
        standard,
        restricted,
        device,
    }
    .serialize(serializer)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestApplicationCollection {
    #[serde(
        rename(serialize = "Application"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub application: Vec<ManifestApplicationEntry>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestApplicationEntry {
    #[serde(rename(serialize = "@Id"), skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(
        rename(serialize = "@Executable"),
        skip_serializing_if = "Option::is_none"
    )]
    pub executable: Option<String>,
    #[serde(
        rename(serialize = "@EntryPoint"),
        skip_serializing_if = "Option::is_none"
    )]
    pub entry_point: Option<String>,
    #[serde(
        rename(serialize = "uap:VisualElements"),
        skip_serializing_if = "Option::is_none"
    )]
    pub visual_elements: Option<ManifestVisualElements>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestVisualElements {
    #[serde(
        rename(serialize = "@DisplayName"),
        skip_serializing_if = "Option::is_none"
    )]
    pub display_name: Option<String>,
    #[serde(
        rename(serialize = "@Description"),
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
    #[serde(
        rename(serialize = "@BackgroundColor"),
        skip_serializing_if = "Option::is_none"
    )]
    pub background_color: Option<String>,
    #[serde(
        rename(serialize = "@Square150x150Logo"),
        skip_serializing_if = "Option::is_none"
    )]
    pub square_150x150_logo: Option<String>,
    #[serde(
        rename(serialize = "@Square44x44Logo"),
        skip_serializing_if = "Option::is_none"
    )]
    pub square_44x44_logo: Option<String>,
}
