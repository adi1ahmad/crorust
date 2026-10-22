pub use crate::utils::AndroidVersionCode;
use anyhow::Result;
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

fn android_manifest_default_namespace() -> String {
    String::from("http://schemas.android.com/apk/res/android")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename = "manifest")]
pub struct AndroidXmlManifestDescriptor {
    #[serde(
        rename(serialize = "@xmlns:android"),
        default = "android_manifest_default_namespace"
    )]
    pub ns_android: String,
    #[serde(
        rename(serialize = "@package"),
        skip_serializing_if = "Option::is_none"
    )]
    pub package: Option<String>,
    #[serde(
        rename(serialize = "@android:versionCode"),
        skip_serializing_if = "Option::is_none"
    )]
    pub version_code: Option<u32>,
    #[serde(
        rename(serialize = "@android:versionName"),
        skip_serializing_if = "Option::is_none"
    )]
    pub version_name: Option<String>,
    #[serde(
        rename(serialize = "@android:compileSdkVersion"),
        skip_serializing_if = "Option::is_none"
    )]
    pub compile_sdk_version: Option<u32>,
    #[serde(
        rename(serialize = "@android:compileSdkVersionCodename"),
        skip_serializing_if = "Option::is_none"
    )]
    pub compile_sdk_version_codename: Option<u32>,
    #[serde(
        rename(serialize = "@platformBuildVersionCode"),
        skip_serializing_if = "Option::is_none"
    )]
    pub platform_build_version_code: Option<u32>,
    #[serde(
        rename(serialize = "@platformBuildVersionName"),
        skip_serializing_if = "Option::is_none"
    )]
    pub platform_build_version_name: Option<u32>,
    #[serde(rename(serialize = "uses-sdk"), default)]
    pub sdk: AndroidSdkDescriptor,
    #[serde(
        rename(serialize = "permission"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub permission: Vec<AndroidCustomPermissionDescriptor>,
    #[serde(
        rename(serialize = "uses-feature"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub uses_feature: Vec<AndroidFeatureDescriptor>,
    #[serde(
        rename(serialize = "uses-permission"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub uses_permission: Vec<AndroidPermissionDescriptor>,
    #[serde(rename(serialize = "application"), default)]
    pub application: AndroidApplicationDescriptor,
}

impl Default for AndroidXmlManifestDescriptor {
    fn default() -> Self {
        Self {
            ns_android: android_manifest_default_namespace(),
            package: None,
            version_code: None,
            version_name: None,
            compile_sdk_version: None,
            compile_sdk_version_codename: None,
            platform_build_version_code: None,
            platform_build_version_name: None,
            sdk: AndroidSdkDescriptor::default(),
            permission: Vec::new(),
            uses_feature: Vec::new(),
            uses_permission: Vec::new(),
            application: AndroidApplicationDescriptor::default(),
        }
    }
}

impl std::fmt::Display for AndroidXmlManifestDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let xml = quick_xml::se::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", xml)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidSdkDescriptor {
    #[serde(
        rename(serialize = "@android:minSdkVersion"),
        skip_serializing_if = "Option::is_none"
    )]
    pub min_sdk_version: Option<u32>,
    #[serde(
        rename(serialize = "@android:targetSdkVersion"),
        skip_serializing_if = "Option::is_none"
    )]
    pub target_sdk_version: Option<u32>,
    #[serde(
        rename(serialize = "@android:maxSdkVersion"),
        skip_serializing_if = "Option::is_none"
    )]
    pub max_sdk_version: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidFeatureDescriptor {
    #[serde(
        rename(serialize = "@android:name"),
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@android:required"),
        skip_serializing_if = "Option::is_none"
    )]
    pub required: Option<bool>,
    #[serde(
        rename(serialize = "@android:version"),
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<u32>,
    #[serde(
        rename(serialize = "@android:glEsVersion"),
        skip_serializing_if = "Option::is_none"
    )]
    pub opengles_version: Option<(u16, u16)>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidCustomPermissionDescriptor {
    #[serde(
        rename(serialize = "@android:name"),
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@android:protectionLevel"),
        skip_serializing_if = "Option::is_none"
    )]
    pub protection_level: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidPermissionDescriptor {
    #[serde(
        rename(serialize = "@android:name"),
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@android:maxSdkVersion"),
        skip_serializing_if = "Option::is_none"
    )]
    pub max_sdk_version: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidApplicationDescriptor {
    #[serde(
        rename(serialize = "@android:debuggable"),
        skip_serializing_if = "Option::is_none"
    )]
    pub debuggable: Option<bool>,
    #[serde(
        rename(serialize = "@android:theme"),
        skip_serializing_if = "Option::is_none"
    )]
    pub theme: Option<String>,
    #[serde(
        rename(serialize = "@android:hasCode"),
        skip_serializing_if = "Option::is_none"
    )]
    pub has_code: Option<bool>,
    #[serde(
        rename(serialize = "@android:icon"),
        skip_serializing_if = "Option::is_none"
    )]
    pub icon: Option<String>,
    #[serde(
        rename(serialize = "@android:label"),
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<String>,
    #[serde(
        rename(serialize = "@android:appComponentFactory"),
        skip_serializing_if = "Option::is_none"
    )]
    pub app_component_factory: Option<String>,
    #[serde(
        rename(serialize = "@android:extractNativeLibs"),
        skip_serializing_if = "Option::is_none"
    )]
    pub extract_native_libs: Option<bool>,
    #[serde(
        rename(serialize = "meta-data"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub meta_data: Vec<AndroidMetaDataDescriptor>,
    #[serde(
        rename(serialize = "provider"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub providers: Vec<AndroidProviderDescriptor>,
    #[serde(
        rename(serialize = "activity"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub activities: Vec<ActivityComponentDescriptor>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ActivityComponentDescriptor {
    #[serde(
        rename(serialize = "@android:name"),
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@android:label"),
        skip_serializing_if = "Option::is_none"
    )]
    pub label: Option<String>,
    #[serde(
        rename(serialize = "@android:theme"),
        skip_serializing_if = "Option::is_none"
    )]
    pub theme: Option<String>,
    #[serde(
        rename(serialize = "@android:configChanges"),
        skip_serializing_if = "Option::is_none"
    )]
    pub config_changes: Option<String>,
    #[serde(
        rename(serialize = "@android:launchMode"),
        skip_serializing_if = "Option::is_none"
    )]
    pub launch_mode: Option<String>,
    #[serde(
        rename(serialize = "@android:screenOrientation"),
        skip_serializing_if = "Option::is_none"
    )]
    pub orientation: Option<String>,
    #[serde(
        rename(serialize = "@android:windowSoftInputMode"),
        skip_serializing_if = "Option::is_none"
    )]
    pub window_soft_input_mode: Option<String>,
    #[serde(
        rename(serialize = "@android:exported"),
        skip_serializing_if = "Option::is_none"
    )]
    pub exported: Option<bool>,
    #[serde(
        rename(serialize = "@android:hardwareAccelerated"),
        skip_serializing_if = "Option::is_none"
    )]
    pub hardware_accelerated: Option<bool>,
    #[serde(
        rename(serialize = "@android:colorMode"),
        skip_serializing_if = "Option::is_none"
    )]
    pub color_mode: Option<String>,
    #[serde(
        rename(serialize = "meta-data"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub meta_data: Vec<AndroidMetaDataDescriptor>,
    #[serde(
        rename(serialize = "intent-filter"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub intent_filters: Vec<AndroidIntentFilterDescriptor>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidIntentFilterDescriptor {
    #[serde(
        rename(serialize = "action"),
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_manifest_action_list"
    )]
    pub actions: Vec<String>,
    #[serde(
        rename(serialize = "category"),
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "serialize_manifest_category_list"
    )]
    pub categories: Vec<String>,
    #[serde(
        rename(serialize = "data"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub data: Vec<AndroidIntentFilterDataDescriptor>,
}

impl AndroidIntentFilterDescriptor {
    pub fn launcher() -> Self {
        Self {
            actions: vec![String::from("android.intent.action.MAIN")],
            categories: vec![String::from("android.intent.category.LAUNCHER")],
            data: Vec::new(),
        }
    }
}

fn serialize_manifest_action_list<S>(action_tags: &[String], ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct ActionElementItem<'a> {
        #[serde(rename = "@android:name")]
        action_name: &'a str,
    }
    let mut seq_ser = ser.serialize_seq(Some(action_tags.len()))?;
    for item in action_tags {
        seq_ser.serialize_element(&ActionElementItem { action_name: item })?;
    }
    seq_ser.end()
}

fn serialize_manifest_category_list<S>(category_tags: &[String], ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct CategoryElementItem<'a> {
        #[serde(rename = "@android:name")]
        category_name: &'a str,
    }
    let mut seq_ser = ser.serialize_seq(Some(category_tags.len()))?;
    for item in category_tags {
        seq_ser.serialize_element(&CategoryElementItem {
            category_name: item,
        })?;
    }
    seq_ser.end()
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidIntentFilterDataDescriptor {
    #[serde(
        rename(serialize = "@android:scheme"),
        skip_serializing_if = "Option::is_none"
    )]
    pub scheme: Option<String>,
    #[serde(
        rename(serialize = "@android:host"),
        skip_serializing_if = "Option::is_none"
    )]
    pub host: Option<String>,
    #[serde(
        rename(serialize = "@android:port"),
        skip_serializing_if = "Option::is_none"
    )]
    pub port: Option<String>,
    #[serde(
        rename(serialize = "@android:path"),
        skip_serializing_if = "Option::is_none"
    )]
    pub path: Option<String>,
    #[serde(
        rename(serialize = "@android:pathPrefix"),
        skip_serializing_if = "Option::is_none"
    )]
    pub path_prefix: Option<String>,
    #[serde(
        rename(serialize = "@android:pathPattern"),
        skip_serializing_if = "Option::is_none"
    )]
    pub path_pattern: Option<String>,
    #[serde(
        rename(serialize = "@android:mimeType"),
        skip_serializing_if = "Option::is_none"
    )]
    pub mime_type: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidMetaDataDescriptor {
    #[serde(
        rename(serialize = "@android:name"),
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@android:value"),
        skip_serializing_if = "Option::is_none"
    )]
    pub value: Option<String>,
    #[serde(
        rename(serialize = "@android:resource"),
        skip_serializing_if = "Option::is_none"
    )]
    pub resource: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AndroidProviderDescriptor {
    #[serde(
        rename(serialize = "@android:name"),
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        rename(serialize = "@android:authorities"),
        skip_serializing_if = "Option::is_none"
    )]
    pub authorities: Option<String>,
    #[serde(
        rename(serialize = "@android:exported"),
        skip_serializing_if = "Option::is_none"
    )]
    pub exported: Option<bool>,
    #[serde(
        rename(serialize = "@android:grantUriPermissions"),
        skip_serializing_if = "Option::is_none"
    )]
    pub grant_uri_permissions: Option<bool>,
    #[serde(
        rename(serialize = "meta-data"),
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub meta_data: Vec<AndroidMetaDataDescriptor>,
}
