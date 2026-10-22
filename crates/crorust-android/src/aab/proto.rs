use crate::manifest::{
    ActivityComponentDescriptor, AndroidIntentFilterDescriptor, AndroidMetaDataDescriptor,
    AndroidProviderDescriptor, AndroidXmlManifestDescriptor,
};
use crate::resources::ResourceSymbols;
use anyhow::Result;
use std::collections::BTreeMap;

const ANDROID_NS_URI: &str = "http://schemas.android.com/apk/res/android";

const ATTR_THEME: u32 = 0x01010000;
const ATTR_LABEL: u32 = 0x01010001;
const ATTR_ICON: u32 = 0x01010002;
const ATTR_NAME: u32 = 0x01010003;
const ATTR_HAS_CODE: u32 = 0x0101000c;
const ATTR_DEBUGGABLE: u32 = 0x0101000f;
const ATTR_EXPORTED: u32 = 0x01010010;
const ATTR_AUTHORITIES: u32 = 0x01010018;
const ATTR_LAUNCH_MODE: u32 = 0x0101001d;
const ATTR_SCREEN_ORIENTATION: u32 = 0x0101001e;
const ATTR_CONFIG_CHANGES: u32 = 0x0101001f;
const ATTR_VALUE: u32 = 0x01010024;
const ATTR_SCHEME: u32 = 0x01010027;
const ATTR_HOST: u32 = 0x01010028;
const ATTR_PORT: u32 = 0x01010029;
const ATTR_PATH: u32 = 0x0101002a;
const ATTR_PATH_PREFIX: u32 = 0x0101002b;
const ATTR_PATH_PATTERN: u32 = 0x0101002c;
const ATTR_MIME_TYPE: u32 = 0x0101002d;
const ATTR_MIN_SDK_VERSION: u32 = 0x0101020c;
const ATTR_MAX_SDK_VERSION: u32 = 0x01010271;
const ATTR_VERSION_CODE: u32 = 0x0101021b;
const ATTR_VERSION_NAME: u32 = 0x0101021c;
const ATTR_WINDOW_SOFT_INPUT_MODE: u32 = 0x0101022b;
const ATTR_TARGET_SDK_VERSION: u32 = 0x01010270;
const ATTR_GLES_VERSION: u32 = 0x01010281;
const ATTR_REQUIRED: u32 = 0x0101028e;
const ATTR_HARDWARE_ACCELERATED: u32 = 0x010102d3;
const ATTR_EXTRACT_NATIVE_LIBS: u32 = 0x010104ea;
const ATTR_VERSION: u32 = 0x01010519;
const ATTR_COLOR_MODE: u32 = 0x01010565;
const ATTR_APP_COMPONENT_FACTORY: u32 = 0x0101057a;

#[derive(Default, Debug, Clone)]
pub struct ProtoWriter {
    buf: Vec<u8>,
}

impl ProtoWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn write_varint(&mut self, mut val: u64) {
        while val >= 0x80 {
            self.buf.push(((val as u8) & 0x7f) | 0x80);
            val >>= 7;
        }
        self.buf.push(val as u8);
    }

    pub fn write_tag(&mut self, field_number: u32, wire_type: u8) {
        self.write_varint(((field_number as u64) << 3) | (wire_type as u64));
    }

    pub fn write_uint32(&mut self, field_number: u32, val: u32) {
        self.write_tag(field_number, 0);
        self.write_varint(val as u64);
    }

    pub fn write_bool(&mut self, field_number: u32, val: bool) {
        self.write_tag(field_number, 0);
        self.write_varint(if val { 1 } else { 0 });
    }

    pub fn write_string(&mut self, field_number: u32, val: &str) {
        self.write_tag(field_number, 2);
        self.write_varint(val.len() as u64);
        self.buf.extend_from_slice(val.as_bytes());
    }

    pub fn write_bytes(&mut self, field_number: u32, bytes: &[u8]) {
        self.write_tag(field_number, 2);
        self.write_varint(bytes.len() as u64);
        self.buf.extend_from_slice(bytes);
    }

    pub fn write_message(&mut self, field_number: u32, submsg: &ProtoWriter) {
        self.write_bytes(field_number, submsg.as_bytes());
    }
}

fn write_plain_attr(elem: &mut ProtoWriter, name: &str, value: &str) {
    let mut attr = ProtoWriter::new();
    attr.write_string(2, name);
    attr.write_string(3, value);
    elem.write_message(3, &attr);
}

fn write_android_attr(elem: &mut ProtoWriter, name: &str, value: &str, resource_id: u32) {
    let mut attr = ProtoWriter::new();
    attr.write_string(1, ANDROID_NS_URI);
    attr.write_string(2, name);
    attr.write_string(3, value);
    attr.write_uint32(5, resource_id);
    elem.write_message(3, &attr);
}

fn wrap_element_as_child(parent_elem: &mut ProtoWriter, child_elem: &ProtoWriter) {
    let mut child_node = ProtoWriter::new();
    child_node.write_message(1, child_elem);
    parent_elem.write_message(4, &child_node);
}

fn encode_meta_data_node(parent_elem: &mut ProtoWriter, md: &AndroidMetaDataDescriptor) {
    let mut md_elem = ProtoWriter::new();
    md_elem.write_string(2, "meta-data");
    if let Some(ref name) = md.name {
        write_android_attr(&mut md_elem, "name", name, ATTR_NAME);
    }
    if let Some(ref value) = md.value {
        write_android_attr(&mut md_elem, "value", value, ATTR_VALUE);
    }
    wrap_element_as_child(parent_elem, &md_elem);
}

fn encode_provider_node(parent_elem: &mut ProtoWriter, prov: &AndroidProviderDescriptor) {
    let mut prov_elem = ProtoWriter::new();
    prov_elem.write_string(2, "provider");
    if let Some(ref name) = prov.name {
        write_android_attr(&mut prov_elem, "name", name, ATTR_NAME);
    }
    if let Some(ref auth) = prov.authorities {
        write_android_attr(&mut prov_elem, "authorities", auth, ATTR_AUTHORITIES);
    }
    if let Some(exported) = prov.exported {
        write_android_attr(
            &mut prov_elem,
            "exported",
            if exported { "true" } else { "false" },
            ATTR_EXPORTED,
        );
    }
    for md in &prov.meta_data {
        encode_meta_data_node(&mut prov_elem, md);
    }
    wrap_element_as_child(parent_elem, &prov_elem);
}

fn encode_intent_filter_node(
    parent_elem: &mut ProtoWriter,
    filter: &AndroidIntentFilterDescriptor,
) {
    let mut filter_elem = ProtoWriter::new();
    filter_elem.write_string(2, "intent-filter");

    for action in &filter.actions {
        let mut action_elem = ProtoWriter::new();
        action_elem.write_string(2, "action");
        write_android_attr(&mut action_elem, "name", action, ATTR_NAME);
        wrap_element_as_child(&mut filter_elem, &action_elem);
    }

    for category in &filter.categories {
        let mut cat_elem = ProtoWriter::new();
        cat_elem.write_string(2, "category");
        write_android_attr(&mut cat_elem, "name", category, ATTR_NAME);
        wrap_element_as_child(&mut filter_elem, &cat_elem);
    }

    for data in &filter.data {
        let mut data_elem = ProtoWriter::new();
        data_elem.write_string(2, "data");
        if let Some(ref scheme) = data.scheme {
            write_android_attr(&mut data_elem, "scheme", scheme, ATTR_SCHEME);
        }
        if let Some(ref host) = data.host {
            write_android_attr(&mut data_elem, "host", host, ATTR_HOST);
        }
        if let Some(ref port) = data.port {
            write_android_attr(&mut data_elem, "port", port, ATTR_PORT);
        }
        if let Some(ref path) = data.path {
            write_android_attr(&mut data_elem, "path", path, ATTR_PATH);
        }
        if let Some(ref prefix) = data.path_prefix {
            write_android_attr(&mut data_elem, "pathPrefix", prefix, ATTR_PATH_PREFIX);
        }
        if let Some(ref pattern) = data.path_pattern {
            write_android_attr(&mut data_elem, "pathPattern", pattern, ATTR_PATH_PATTERN);
        }
        if let Some(ref mime) = data.mime_type {
            write_android_attr(&mut data_elem, "mimeType", mime, ATTR_MIME_TYPE);
        }
        wrap_element_as_child(&mut filter_elem, &data_elem);
    }

    wrap_element_as_child(parent_elem, &filter_elem);
}

fn encode_activity_node(
    parent_elem: &mut ProtoWriter,
    act: &ActivityComponentDescriptor,
    default_act_name: &str,
) {
    let mut act_elem = ProtoWriter::new();
    act_elem.write_string(2, "activity");

    let act_name = act.name.as_deref().unwrap_or(default_act_name);
    write_android_attr(&mut act_elem, "name", act_name, ATTR_NAME);

    if let Some(ref label) = act.label {
        write_android_attr(&mut act_elem, "label", label, ATTR_LABEL);
    }
    if let Some(ref config_changes) = act.config_changes {
        write_android_attr(
            &mut act_elem,
            "configChanges",
            config_changes,
            ATTR_CONFIG_CHANGES,
        );
    }
    if let Some(ref launch_mode) = act.launch_mode {
        write_android_attr(&mut act_elem, "launchMode", launch_mode, ATTR_LAUNCH_MODE);
    }
    if let Some(ref orientation) = act.orientation {
        write_android_attr(
            &mut act_elem,
            "screenOrientation",
            orientation,
            ATTR_SCREEN_ORIENTATION,
        );
    }
    if let Some(ref soft_input) = act.window_soft_input_mode {
        write_android_attr(
            &mut act_elem,
            "windowSoftInputMode",
            soft_input,
            ATTR_WINDOW_SOFT_INPUT_MODE,
        );
    }
    let exported = act.exported.unwrap_or(true);
    write_android_attr(
        &mut act_elem,
        "exported",
        if exported { "true" } else { "false" },
        ATTR_EXPORTED,
    );
    if let Some(hw_accel) = act.hardware_accelerated {
        write_android_attr(
            &mut act_elem,
            "hardwareAccelerated",
            if hw_accel { "true" } else { "false" },
            ATTR_HARDWARE_ACCELERATED,
        );
    }
    if let Some(ref color_mode) = act.color_mode {
        write_android_attr(&mut act_elem, "colorMode", color_mode, ATTR_COLOR_MODE);
    }
    if let Some(ref theme) = act.theme {
        write_android_attr(&mut act_elem, "theme", theme, ATTR_THEME);
    }

    for md in &act.meta_data {
        encode_meta_data_node(&mut act_elem, md);
    }

    if act.intent_filters.is_empty() {
        let default_filter = AndroidIntentFilterDescriptor {
            actions: vec!["android.intent.action.MAIN".to_string()],
            categories: vec!["android.intent.category.LAUNCHER".to_string()],
            data: Vec::new(),
        };
        encode_intent_filter_node(&mut act_elem, &default_filter);
    } else {
        for filter in &act.intent_filters {
            encode_intent_filter_node(&mut act_elem, filter);
        }
    }

    wrap_element_as_child(parent_elem, &act_elem);
}

pub fn encode_bundle_config() -> Result<Vec<u8>> {
    let mut config = ProtoWriter::new();

    let mut btool = ProtoWriter::new();
    btool.write_string(1, "1.15.6");
    config.write_message(1, &btool);

    let mut optimizations = ProtoWriter::new();
    let mut splits_config = ProtoWriter::new();

    for dim in [1u32, 2, 3] {
        let mut split_dim = ProtoWriter::new();
        split_dim.write_uint32(1, dim);
        split_dim.write_bool(2, false);
        splits_config.write_message(1, &split_dim);
    }
    optimizations.write_message(1, &splits_config);
    config.write_message(2, &optimizations);

    let mut compression = ProtoWriter::new();
    compression.write_string(1, "lib/**");
    compression.write_string(1, "**/*.so");
    config.write_message(3, &compression);

    Ok(config.into_bytes())
}

pub fn encode_manifest_xml(manifest: &AndroidXmlManifestDescriptor) -> Result<Vec<u8>> {
    let mut root_node = ProtoWriter::new();
    let mut root_elem = ProtoWriter::new();

    root_elem.write_string(2, "manifest");

    let mut ns = ProtoWriter::new();
    ns.write_string(1, "android");
    ns.write_string(2, ANDROID_NS_URI);
    root_elem.write_message(5, &ns);

    let pkg_name = manifest.package.as_deref().unwrap_or("app");
    if let Some(ref pkg) = manifest.package {
        write_plain_attr(&mut root_elem, "package", pkg);
    }

    if let Some(vc) = manifest.version_code {
        write_android_attr(
            &mut root_elem,
            "versionCode",
            &vc.to_string(),
            ATTR_VERSION_CODE,
        );
    }

    if let Some(ref vn) = manifest.version_name {
        write_android_attr(&mut root_elem, "versionName", vn, ATTR_VERSION_NAME);
    }

    let mut uses_sdk_elem = ProtoWriter::new();
    uses_sdk_elem.write_string(2, "uses-sdk");

    if let Some(min_sdk) = manifest.sdk.min_sdk_version {
        write_android_attr(
            &mut uses_sdk_elem,
            "minSdkVersion",
            &min_sdk.to_string(),
            ATTR_MIN_SDK_VERSION,
        );
    }
    if let Some(target_sdk) = manifest.sdk.target_sdk_version {
        write_android_attr(
            &mut uses_sdk_elem,
            "targetSdkVersion",
            &target_sdk.to_string(),
            ATTR_TARGET_SDK_VERSION,
        );
    }
    if let Some(max_sdk) = manifest.sdk.max_sdk_version {
        write_android_attr(
            &mut uses_sdk_elem,
            "maxSdkVersion",
            &max_sdk.to_string(),
            ATTR_MAX_SDK_VERSION,
        );
    }
    wrap_element_as_child(&mut root_elem, &uses_sdk_elem);

    for feat in &manifest.uses_feature {
        let mut feat_elem = ProtoWriter::new();
        feat_elem.write_string(2, "uses-feature");
        if let Some(ref name) = feat.name {
            write_android_attr(&mut feat_elem, "name", name, ATTR_NAME);
        }
        if let Some(req) = feat.required {
            write_android_attr(
                &mut feat_elem,
                "required",
                if req { "true" } else { "false" },
                ATTR_REQUIRED,
            );
        }
        if let Some(ver) = feat.version {
            write_android_attr(&mut feat_elem, "version", &ver.to_string(), ATTR_VERSION);
        }
        if let Some((maj, min)) = feat.opengles_version {
            let gles = format!("0x{:04x}{:04x}", maj, min);
            write_android_attr(&mut feat_elem, "glEsVersion", &gles, ATTR_GLES_VERSION);
        }
        wrap_element_as_child(&mut root_elem, &feat_elem);
    }

    for perm in &manifest.uses_permission {
        let mut perm_elem = ProtoWriter::new();
        perm_elem.write_string(2, "uses-permission");
        if let Some(ref name) = perm.name {
            write_android_attr(&mut perm_elem, "name", name, ATTR_NAME);
        }
        if let Some(max_sdk) = perm.max_sdk_version {
            write_android_attr(
                &mut perm_elem,
                "maxSdkVersion",
                &max_sdk.to_string(),
                ATTR_MAX_SDK_VERSION,
            );
        }
        wrap_element_as_child(&mut root_elem, &perm_elem);
    }

    let mut app_elem = ProtoWriter::new();
    app_elem.write_string(2, "application");

    if let Some(ref label) = manifest.application.label {
        write_android_attr(&mut app_elem, "label", label, ATTR_LABEL);
    }
    if let Some(ref icon) = manifest.application.icon {
        write_android_attr(&mut app_elem, "icon", icon, ATTR_ICON);
    }
    if let Some(ref theme) = manifest.application.theme {
        write_android_attr(&mut app_elem, "theme", theme, ATTR_THEME);
    }
    if let Some(debuggable) = manifest.application.debuggable {
        write_android_attr(
            &mut app_elem,
            "debuggable",
            if debuggable { "true" } else { "false" },
            ATTR_DEBUGGABLE,
        );
    }
    if let Some(ref factory) = manifest.application.app_component_factory {
        write_android_attr(
            &mut app_elem,
            "appComponentFactory",
            factory,
            ATTR_APP_COMPONENT_FACTORY,
        );
    }

    let has_code = manifest.application.has_code.unwrap_or(true);
    write_android_attr(
        &mut app_elem,
        "hasCode",
        if has_code { "true" } else { "false" },
        ATTR_HAS_CODE,
    );

    let extract_native_libs = manifest.application.extract_native_libs.unwrap_or(false);
    write_android_attr(
        &mut app_elem,
        "extractNativeLibs",
        if extract_native_libs { "true" } else { "false" },
        ATTR_EXTRACT_NATIVE_LIBS,
    );

    for md in &manifest.application.meta_data {
        encode_meta_data_node(&mut app_elem, md);
    }

    for prov in &manifest.application.providers {
        encode_provider_node(&mut app_elem, prov);
    }

    let fallback_act_name = format!("{}.MainActivity", pkg_name);
    if manifest.application.activities.is_empty() {
        let default_act = ActivityComponentDescriptor {
            name: Some(fallback_act_name.clone()),
            exported: Some(true),
            ..Default::default()
        };
        encode_activity_node(&mut app_elem, &default_act, &fallback_act_name);
    } else {
        for act in &manifest.application.activities {
            encode_activity_node(&mut app_elem, act, &fallback_act_name);
        }
    }

    wrap_element_as_child(&mut root_elem, &app_elem);
    root_node.write_message(1, &root_elem);

    Ok(root_node.into_bytes())
}

fn write_string_entry(type_msg: &mut ProtoWriter, entry_idx: u32, name: &str, value: &str) {
    let mut entry = ProtoWriter::new();
    let mut entry_id = ProtoWriter::new();
    entry_id.write_uint32(1, entry_idx);
    entry.write_message(1, &entry_id);
    entry.write_string(2, name);

    let mut config_value = ProtoWriter::new();
    let mut val = ProtoWriter::new();
    let mut item = ProtoWriter::new();
    let mut str_val = ProtoWriter::new();
    str_val.write_string(1, value);
    item.write_message(2, &str_val);
    val.write_message(1, &item);
    config_value.write_message(2, &val);
    entry.write_message(3, &config_value);

    type_msg.write_message(3, &entry);
}

fn write_file_ref_entry(
    type_msg: &mut ProtoWriter,
    entry_idx: u32,
    name: &str,
    res_path: &str,
    file_type: u32,
) {
    let mut entry = ProtoWriter::new();
    let mut entry_id = ProtoWriter::new();
    entry_id.write_uint32(1, entry_idx);
    entry.write_message(1, &entry_id);
    entry.write_string(2, name);

    let mut config_val = ProtoWriter::new();
    let mut v = ProtoWriter::new();
    let mut it = ProtoWriter::new();
    let mut file_ref = ProtoWriter::new();
    file_ref.write_string(1, res_path);
    file_ref.write_uint32(2, file_type);
    it.write_message(4, &file_ref);
    v.write_message(1, &it);
    config_val.write_message(2, &v);
    entry.write_message(3, &config_val);

    type_msg.write_message(3, &entry);
}

pub fn encode_resource_table(
    manifest: &AndroidXmlManifestDescriptor,
    has_icon: bool,
    symbols: Option<&ResourceSymbols>,
) -> Result<Vec<u8>> {
    let mut table = ProtoWriter::new();
    let mut pkg = ProtoWriter::new();

    let mut pkg_id = ProtoWriter::new();
    pkg_id.write_uint32(1, 0x7f);
    pkg.write_message(1, &pkg_id);

    let pkg_name = manifest.package.as_deref().unwrap_or("app");
    pkg.write_string(2, pkg_name);

    let app_name = manifest.application.label.as_deref().unwrap_or("App");

    if let Some(sym) = symbols {
        let mut grouped: BTreeMap<(u32, String), BTreeMap<u32, String>> = BTreeMap::new();
        for ((res_type, res_name), &full_id) in &sym.id_map {
            let type_id = (full_id >> 16) & 0xff;
            let entry_idx = full_id & 0xffff;
            if type_id > 0 {
                grouped
                    .entry((type_id, res_type.clone()))
                    .or_default()
                    .insert(entry_idx, res_name.clone());
            }
        }

        let has_string_type = grouped.keys().any(|(_, t)| t == "string");
        if !has_string_type {
            let next_type_id = grouped.keys().map(|(id, _)| *id).max().unwrap_or(0) + 1;
            grouped
                .entry((next_type_id, "string".to_string()))
                .or_default()
                .insert(0, "app_name".to_string());
        }

        if has_icon && !grouped.keys().any(|(_, t)| t == "mipmap") {
            let next_type_id = grouped.keys().map(|(id, _)| *id).max().unwrap_or(0) + 1;
            grouped
                .entry((next_type_id, "mipmap".to_string()))
                .or_default()
                .insert(0, "icon".to_string());
        }

        for ((type_id, res_type), entries) in grouped {
            let mut type_msg = ProtoWriter::new();
            let mut type_id_msg = ProtoWriter::new();
            type_id_msg.write_uint32(1, type_id);
            type_msg.write_message(1, &type_id_msg);
            type_msg.write_string(2, &res_type);

            for (entry_idx, res_name) in entries {
                if res_type == "string" {
                    let val = sym
                        .string_values
                        .get(&res_name)
                        .map(|s| s.as_str())
                        .unwrap_or(if res_name == "app_name" {
                            app_name
                        } else {
                            &res_name
                        });
                    write_string_entry(&mut type_msg, entry_idx, &res_name, val);
                } else if res_type == "mipmap" && res_name == "icon" {
                    write_file_ref_entry(
                        &mut type_msg,
                        entry_idx,
                        &res_name,
                        "res/icon/icon192.png",
                        1,
                    );
                } else if let Some(rel_path) = sym
                    .file_resources
                    .get(&(res_type.clone(), res_name.clone()))
                {
                    let file_type = if rel_path.ends_with(".xml") { 2 } else { 1 };
                    write_file_ref_entry(&mut type_msg, entry_idx, &res_name, rel_path, file_type);
                } else {
                    write_string_entry(&mut type_msg, entry_idx, &res_name, "");
                }
            }

            pkg.write_message(3, &type_msg);
        }
    } else {
        let mut string_type = ProtoWriter::new();
        let mut string_type_id = ProtoWriter::new();
        string_type_id.write_uint32(1, 5);
        string_type.write_message(1, &string_type_id);
        string_type.write_string(2, "string");
        write_string_entry(&mut string_type, 0, "app_name", app_name);
        pkg.write_message(3, &string_type);

        if has_icon {
            let mut mipmap_type = ProtoWriter::new();
            let mut mipmap_type_id = ProtoWriter::new();
            mipmap_type_id.write_uint32(1, 3);
            mipmap_type.write_message(1, &mipmap_type_id);
            mipmap_type.write_string(2, "mipmap");
            write_file_ref_entry(&mut mipmap_type, 0, "icon", "res/icon/icon192.png", 1);
            pkg.write_message(3, &mipmap_type);
        }
    }

    table.write_message(1, &pkg);
    Ok(table.into_bytes())
}
