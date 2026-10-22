use super::super::protocol::chunk_stream::ResourceValueType;
use super::super::protocol::chunks::{
    AndroidChunk, TypedResourceValue, XmlAttributeRecord, XmlEndElementHeader, XmlNamespaceRecord,
    XmlNodeHeader, XmlStartElementHeader,
};
use super::super::protocol::string_pool::StringPoolBuilder;
use super::super::resources::attribute_index::{
    lookup_framework_attribute, lookup_framework_resource,
};
use super::super::resources::symbol_catalog::{ResourceIdentifierReference, ResourceSymbols};
use super::super::resources::table_compiler::ResourceCatalogTable;
use anyhow::Result;
use std::collections::HashSet;

pub const ANDROID_RES_NAMESPACE: &str = "http://schemas.android.com/apk/res/android";
pub const AUTO_RES_NAMESPACE: &str = "http://schemas.android.com/apk/res-auto";

#[derive(Clone, Debug, Default)]
pub struct TagName {
    pub name: String,
    pub namespace: Option<String>,
}

impl TagName {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}

#[derive(Clone, Debug)]
pub struct XmlNamespace {
    pub prefix: Option<String>,
    pub uri: String,
}

impl XmlNamespace {
    pub fn name(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

#[derive(Clone, Debug)]
pub struct XmlAttribute {
    pub name: String,
    pub value: String,
    pub namespace: Option<String>,
}

impl XmlAttribute {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct XmlElement {
    pub tag_name: TagName,
    pub namespaces: Vec<XmlNamespace>,
    pub attributes: Vec<XmlAttribute>,
    pub children: Vec<XmlElement>,
}

impl XmlElement {
    pub fn tag_name(&self) -> &TagName {
        &self.tag_name
    }
    pub fn namespaces(&self) -> &[XmlNamespace] {
        &self.namespaces
    }
    pub fn attributes(&self) -> &[XmlAttribute] {
        &self.attributes
    }
    pub fn children(&self) -> &[XmlElement] {
        &self.children
    }

    pub fn parse(xml: &str) -> Result<Self> {
        use quick_xml::events::Event;
        use quick_xml::reader::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut stack: Vec<XmlElement> = Vec::new();
        let mut ns_stack: Vec<Vec<XmlNamespace>> = Vec::new();
        let mut root: Option<XmlElement> = None;
        let mut buf = Vec::new();

        let resolve_ns = |prefix: Option<&str>, ns_stack: &[Vec<XmlNamespace>]| -> Option<String> {
            for frame in ns_stack.iter().rev() {
                for ns in frame.iter().rev() {
                    if ns.prefix.as_deref() == prefix {
                        return Some(ns.uri.clone());
                    }
                }
            }
            None
        };

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(ref e) => {
                    let (elem, ns_frame) = Self::parse_start_event(e, &ns_stack, &resolve_ns)?;
                    ns_stack.push(ns_frame);
                    stack.push(elem);
                }
                Event::Empty(ref e) => {
                    let (elem, _) = Self::parse_start_event(e, &ns_stack, &resolve_ns)?;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(elem);
                    } else {
                        root = Some(elem);
                    }
                }
                Event::End(_) => {
                    ns_stack.pop();
                    if let Some(elem) = stack.pop() {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(elem);
                        } else {
                            root = Some(elem);
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        root.ok_or_else(|| anyhow::anyhow!("Empty or invalid XML document"))
    }

    fn parse_start_event<F>(
        e: &quick_xml::events::BytesStart<'_>,
        ns_stack: &[Vec<XmlNamespace>],
        resolve_ns: &F,
    ) -> Result<(XmlElement, Vec<XmlNamespace>)>
    where
        F: Fn(Option<&str>, &[Vec<XmlNamespace>]) -> Option<String>,
    {
        let mut namespaces = Vec::new();
        let mut raw_attrs = Vec::new();

        for attr in e.attributes() {
            let attr = attr?;
            let key = std::str::from_utf8(attr.key.into_inner())?;
            let val = attr.unescape_value()?.to_string();

            if let Some(prefix) = key.strip_prefix("xmlns:") {
                namespaces.push(XmlNamespace {
                    prefix: Some(prefix.to_string()),
                    uri: val,
                });
            } else if key == "xmlns" {
                namespaces.push(XmlNamespace {
                    prefix: None,
                    uri: val,
                });
            } else {
                let (prefix, local) = match key.split_once(':') {
                    Some((p, l)) => (Some(p.to_string()), l.to_string()),
                    None => (None, key.to_string()),
                };
                raw_attrs.push((prefix, local, val));
            }
        }

        let mut active_ns = ns_stack.to_vec();
        active_ns.push(namespaces.clone());

        let raw_tag = std::str::from_utf8(e.name().into_inner())?;
        let (tag_prefix, tag_local) = match raw_tag.split_once(':') {
            Some((p, l)) => (Some(p), l),
            None => (None, raw_tag),
        };
        let tag_namespace = resolve_ns(tag_prefix, &active_ns);

        let mut attributes = Vec::with_capacity(raw_attrs.len());
        for (attr_prefix, name, value) in raw_attrs {
            let namespace = match attr_prefix.as_deref() {
                Some(p) => resolve_ns(Some(p), &active_ns),
                None => None,
            };
            attributes.push(XmlAttribute {
                name,
                value,
                namespace,
            });
        }

        let elem = XmlElement {
            tag_name: TagName {
                name: tag_local.to_string(),
                namespace: tag_namespace,
            },
            namespaces: namespaces.clone(),
            attributes,
            children: Vec::new(),
        };

        Ok((elem, namespaces))
    }
}

pub fn compile_manifest(
    manifest: &super::manifest_dom::AndroidXmlManifestDescriptor,
) -> Result<Vec<u8>> {
    let xml = quick_xml::se::to_string(manifest)?;
    let table = ResourceCatalogTable::default();
    let chunk = compile_xml(&xml, &table)?;
    let mut out = Vec::new();
    chunk.write(&mut out)?;
    Ok(out)
}

pub fn compile_xml(xml: &str, table: &ResourceCatalogTable) -> Result<AndroidChunk> {
    compile_xml_with_symbols(xml, table, None)
}

pub fn compile_xml_with_symbols(
    xml: &str,
    table: &ResourceCatalogTable,
    symbols: Option<&ResourceSymbols>,
) -> Result<AndroidChunk> {
    let root = XmlElement::parse(xml)?;

    // Pass 1: Collect unique attribute names that have an associated resource ID.
    // In Android Binary XML (AXML), Android's ResXMLTree runtime binds mResMap such that
    // mResMap[name_string_index] gives the 32-bit resource ID of the attribute.
    // Therefore, all resource-mapped attribute names MUST reside at indices 0..N-1
    // of the string pool, matching attr_res_map 1-to-1.
    let mut res_attr_names = HashSet::new();
    let mut res_attrs = Vec::new();
    collect_resource_attributes(&root, table, symbols, &mut res_attr_names, &mut res_attrs);
    res_attrs.sort_by_key(|&(_, id)| id);

    let mut pool = StringPoolBuilder::new_utf8();
    let mut attr_res_map = Vec::with_capacity(res_attrs.len());
    for (name, res_id) in &res_attrs {
        pool.intern(name);
        attr_res_map.push(*res_id);
    }

    // Pass 2: Intern all remaining non-resource strings into the pool.
    // Since these receive indices >= attr_res_map.len(), Android's getAttributeNameResID
    // bounds check (nameRes < mNumResMap) safely evaluates to false and returns 0.
    index_non_resource_strings(&root, &mut pool);

    let mut body_chunks = Vec::new();

    for ns in root.namespaces() {
        let prefix_id = ns.name().map(|p| pool.intern(p) as i32).unwrap_or(-1);
        let uri_id = pool.intern(ns.uri()) as i32;
        body_chunks.push(AndroidChunk::XmlStartNamespace(
            XmlNodeHeader::default(),
            XmlNamespaceRecord {
                prefix_string_index: prefix_id,
                uri_string_index: uri_id,
            },
        ));
    }

    compile_node(
        &root,
        &mut pool,
        &attr_res_map,
        &mut body_chunks,
        table,
        symbols,
    )?;

    for ns in root.namespaces() {
        let prefix_id = ns.name().map(|p| pool.intern(p) as i32).unwrap_or(-1);
        let uri_id = pool.intern(ns.uri()) as i32;
        body_chunks.push(AndroidChunk::XmlEndNamespace(
            XmlNodeHeader::default(),
            XmlNamespaceRecord {
                prefix_string_index: prefix_id,
                uri_string_index: uri_id,
            },
        ));
    }

    let mut chunks = Vec::with_capacity(2 + body_chunks.len());
    chunks.push(AndroidChunk::StringPool(
        pool.strings().to_vec(),
        Vec::new(),
    ));
    chunks.push(AndroidChunk::XmlResourceMap(attr_res_map));
    chunks.extend(body_chunks);

    Ok(AndroidChunk::Xml(chunks))
}

fn collect_resource_attributes(
    node: &XmlElement,
    table: &ResourceCatalogTable,
    symbols: Option<&ResourceSymbols>,
    seen: &mut HashSet<String>,
    res_attrs: &mut Vec<(String, u32)>,
) {
    for attr in node.attributes() {
        let ns = attr.namespace();
        let name = attr.name();

        let is_android = ns == Some(ANDROID_RES_NAMESPACE);
        let is_auto = ns == Some(AUTO_RES_NAMESPACE);

        if (is_android || is_auto) && !seen.contains(name) {
            seen.insert(name.to_string());
            let res_id = if is_android {
                lookup_framework_attribute(name)
                    .or_else(|| table.lookup_attribute_id(name).ok())
                    .unwrap_or_else(|| {
                        let hash = name
                            .bytes()
                            .fold(0u16, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u16));
                        0x0101_0000 | (hash as u32)
                    })
            } else {
                symbols
                    .and_then(|s| s.get_attribute_id(name))
                    .or_else(|| {
                        table
                            .entry_by_ref(ResourceIdentifierReference {
                                package: None,
                                ty: "attr",
                                name,
                            })
                            .ok()
                            .map(|e| u32::from(e.id()))
                    })
                    .unwrap_or_else(|| {
                        let hash = name
                            .bytes()
                            .fold(0u16, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u16));
                        0x7f04_0000 | (hash as u32)
                    })
            };
            res_attrs.push((name.to_string(), res_id));
        }
    }

    for child in node.children() {
        collect_resource_attributes(child, table, symbols, seen, res_attrs);
    }
}

fn index_non_resource_strings(node: &XmlElement, pool: &mut StringPoolBuilder) {
    pool.intern(node.tag_name().name());

    for ns in node.namespaces() {
        if let Some(prefix) = ns.name() {
            pool.intern(prefix);
        }
        pool.intern(ns.uri());
    }

    for attr in node.attributes() {
        pool.intern(attr.name());
        pool.intern(attr.value());
    }

    for child in node.children() {
        index_non_resource_strings(child, pool);
    }
}

fn compile_node(
    node: &XmlElement,
    pool: &mut StringPoolBuilder,
    res_map: &[u32],
    chunks: &mut Vec<AndroidChunk>,
    table: &ResourceCatalogTable,
    symbols: Option<&ResourceSymbols>,
) -> Result<()> {
    let ns_id = node
        .tag_name()
        .namespace()
        .map(|ns| pool.intern(ns) as i32)
        .unwrap_or(-1);
    let name_id = pool.intern(node.tag_name().name()) as i32;

    let mut compiled_attrs = Vec::with_capacity(node.attributes().len());
    for attr in node.attributes() {
        let attr_ns = attr
            .namespace()
            .map(|ns| pool.intern(ns) as i32)
            .unwrap_or(-1);
        let attr_name = pool.intern(attr.name()) as i32;

        let typed_val =
            compile_attribute_typed_value(attr.name(), attr.value(), table, symbols, pool);

        let raw_val = if typed_val.data_type_tag == ResourceValueType::String as u8 {
            typed_val.data_payload as i32
        } else {
            -1
        };

        compiled_attrs.push(XmlAttributeRecord {
            namespace_uri_index: attr_ns,
            attribute_name_index: attr_name,
            raw_string_index: raw_val,
            typed_value_entry: typed_val,
        });
    }

    // Android runtime (libs/androidfw BackTrackingAttributeFinder) requires attributes
    // within each element to be sorted by resource ID (ascending). Non-resource attributes
    // follow at the end sorted by attribute name string index.
    compiled_attrs.sort_by_key(|attr| {
        let idx = attr.attribute_name_index as usize;
        if idx < res_map.len() {
            (0u8, res_map[idx])
        } else {
            (1u8, attr.attribute_name_index as u32)
        }
    });

    let mut id_index = 0u16;
    let mut class_index = 0u16;
    let mut style_index = 0u16;
    for (i, attr) in compiled_attrs.iter().enumerate() {
        let name = &pool.strings()[attr.attribute_name_index as usize];
        if name == "id" {
            id_index = i as u16 + 1;
        } else if name == "class" {
            class_index = i as u16 + 1;
        } else if name == "style" {
            style_index = i as u16 + 1;
        }
    }

    let start_elem = XmlStartElementHeader {
        namespace_uri_index: ns_id,
        element_name_index: name_id,
        attr_byte_offset: 20,
        attr_record_size: 20,
        attr_record_count: compiled_attrs.len() as u16,
        id_attribute_index: id_index,
        class_attribute_index: class_index,
        style_attribute_index: style_index,
    };

    chunks.push(AndroidChunk::XmlStartElement(
        XmlNodeHeader::default(),
        start_elem,
        compiled_attrs,
    ));

    for child in node.children() {
        compile_node(child, pool, res_map, chunks, table, symbols)?;
    }

    let end_elem = XmlEndElementHeader {
        namespace_uri_index: ns_id,
        element_name_index: name_id,
    };

    chunks.push(AndroidChunk::XmlEndElement(
        XmlNodeHeader::default(),
        end_elem,
    ));
    Ok(())
}

fn compile_attribute_typed_value(
    name: &str,
    value: &str,
    table: &ResourceCatalogTable,
    symbols: Option<&ResourceSymbols>,
    pool: &mut StringPoolBuilder,
) -> TypedResourceValue {
    // 1. Explicit null / empty references
    if value == "@null" || value == "@empty" {
        return TypedResourceValue::new(ResourceValueType::Reference, 0);
    }

    // 2. Resource references (@...) and attribute references (?...)
    let is_attr_ref = value.starts_with('?');
    if value.starts_with('@') || is_attr_ref {
        if let Ok(parsed_ref) = ResourceIdentifierReference::parse(value) {
            let id = if parsed_ref.package == Some("android") {
                table
                    .entry_by_ref(parsed_ref)
                    .ok()
                    .map(|e| u32::from(e.id()))
                    .or_else(|| lookup_framework_resource(parsed_ref.ty, parsed_ref.name))
                    .unwrap_or_else(|| {
                        let ty_id: u32 = match parsed_ref.ty {
                            "attr" => 0x01,
                            "id" => 0x02,
                            "style" => 0x03,
                            "string" => 0x04,
                            "dimen" => 0x05,
                            "color" => 0x06,
                            "array" => 0x07,
                            "drawable" => 0x08,
                            "layout" => 0x09,
                            "anim" => 0x0a,
                            "animator" => 0x0b,
                            "interpolator" => 0x0c,
                            "mipmap" => 0x0d,
                            "integer" => 0x0e,
                            "bool" => 0x11,
                            _ => 0x01,
                        };
                        let hash = value
                            .bytes()
                            .fold(0u16, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u16));
                        (0x01 << 24) | (ty_id << 16) | (hash as u32)
                    })
            } else if let Some(sym) = symbols {
                if let Some(id) = sym.get_resource_id(parsed_ref.ty, parsed_ref.name) {
                    id
                } else if let Ok(entry) = table.entry_by_ref(parsed_ref) {
                    u32::from(entry.id())
                } else {
                    let hash = value
                        .bytes()
                        .fold(0u16, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u16));
                    0x7f08_0000 | (hash as u32)
                }
            } else if let Ok(entry) = table.entry_by_ref(parsed_ref) {
                u32::from(entry.id())
            } else {
                let hash = value
                    .bytes()
                    .fold(0u16, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u16));
                0x7f08_0000 | (hash as u32)
            };

            return TypedResourceValue::new(
                if is_attr_ref {
                    ResourceValueType::Attribute
                } else {
                    ResourceValueType::Reference
                },
                id,
            );
        }
    }

    // 3. Hex color literals
    if value.starts_with('#') {
        if let Some(color) = parse_color(value) {
            return TypedResourceValue::new(ResourceValueType::IntColorArgb8, color);
        }
    }

    // 4. Layout dimension constants
    if value == "match_parent" || value == "fill_parent" {
        return TypedResourceValue::new(ResourceValueType::IntDec, 0xffff_ffff);
    }
    if value == "wrap_content" {
        return TypedResourceValue::new(ResourceValueType::IntDec, 0xffff_fffe);
    }

    // 5. Dimension units (dp, sp, px, pt, in, mm)
    if let Some(dimen) = parse_dimension(value) {
        return TypedResourceValue::new(ResourceValueType::Dimension, dimen);
    }

    // 6. Booleans
    if value == "true" {
        return TypedResourceValue::new(ResourceValueType::IntBoolean, 0xffff_ffff);
    }
    if value == "false" {
        return TypedResourceValue::new(ResourceValueType::IntBoolean, 0);
    }

    // 7. Platform build version integers
    if name == "platformBuildVersionCode" || name == "platformBuildVersionName" {
        if let Ok(num) = value.parse::<i32>() {
            return TypedResourceValue::new(ResourceValueType::IntDec, num as u32);
        }
    }

    // 8. Hex and decimal numbers
    if let Some(hex) = value
        .strip_prefix("0x")
        .and_then(|h| u32::from_str_radix(h, 16).ok())
    {
        return TypedResourceValue::new(ResourceValueType::IntHex, hex);
    }
    if let Ok(num) = value.parse::<i32>() {
        return TypedResourceValue::new(ResourceValueType::IntDec, num as u32);
    }

    // 9. Floating point numbers
    if value.contains('.') {
        if let Ok(f) = value.parse::<f32>() {
            return TypedResourceValue::new(ResourceValueType::Float, f.to_bits());
        }
    }

    // 10. Pipe-delimited flags or enum values
    if value.contains('|') {
        let mut flag_data = 0u32;
        let mut matched = false;
        for part in value.split('|') {
            let flag = part.trim();
            if let Ok(entry) = table.entry_by_ref(ResourceIdentifierReference::id(flag)) {
                if let Ok(attr_entry) = table.entry_by_ref(ResourceIdentifierReference::attr(name))
                {
                    if let Some(val) = attr_entry.lookup_value(entry.id()) {
                        flag_data |= val.data_payload;
                        matched = true;
                        continue;
                    }
                }
            }
            if let Some(val) = lookup_manifest_flag(name, flag) {
                flag_data |= val;
                matched = true;
            }
        }
        if matched {
            return TypedResourceValue::new(ResourceValueType::IntHex, flag_data);
        }
    } else {
        if let Ok(entry) = table.entry_by_ref(ResourceIdentifierReference::id(value)) {
            if let Ok(attr_entry) = table.entry_by_ref(ResourceIdentifierReference::attr(name)) {
                if let Some(val) = attr_entry.lookup_value(entry.id()) {
                    return val;
                }
            }
        }
        if let Some(val) = lookup_manifest_enum(name, value) {
            return TypedResourceValue::new(ResourceValueType::IntDec, val);
        }
        if let Some(val) = lookup_manifest_flag(name, value) {
            return TypedResourceValue::new(ResourceValueType::IntHex, val);
        }
    }

    // 11. Fallback to interned string
    let str_idx = pool.intern(value);
    TypedResourceValue::new(ResourceValueType::String, str_idx)
}

fn parse_color(val: &str) -> Option<u32> {
    let hex = val.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some(
                0xff00_0000
                    | ((r as u32 * 0x11) << 16)
                    | ((g as u32 * 0x11) << 8)
                    | (b as u32 * 0x11),
            )
        }
        4 => {
            let a = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let r = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..3], 16).ok()?;
            let b = u8::from_str_radix(&hex[3..4], 16).ok()?;
            Some(
                ((a as u32 * 0x11) << 24)
                    | ((r as u32 * 0x11) << 16)
                    | ((g as u32 * 0x11) << 8)
                    | (b as u32 * 0x11),
            )
        }
        6 => {
            let rgb = u32::from_str_radix(hex, 16).ok()?;
            Some(0xff00_0000 | rgb)
        }
        8 => u32::from_str_radix(hex, 16).ok(),
        _ => None,
    }
}

fn parse_dimension(val: &str) -> Option<u32> {
    const UNITS: &[(&str, u32)] = &[
        ("dip", 1),
        ("dp", 1),
        ("sp", 2),
        ("pt", 3),
        ("in", 4),
        ("mm", 5),
        ("px", 0),
    ];

    let val = val.trim();
    for &(suffix, unit_code) in UNITS {
        if let Some(num_str) = val.strip_suffix(suffix) {
            if let Ok(num) = num_str.trim().parse::<f32>() {
                let (mantissa, radix) = if (num - num.trunc()).abs() < 1e-5 {
                    (num as i32, 0u32)
                } else if num.abs() < 32768.0 {
                    ((num * 128.0).round() as i32, 1u32)
                } else {
                    ((num * 32768.0).round() as i32, 2u32)
                };
                let packed = (((mantissa as u32) & 0x00ff_ffff) << 8) | (radix << 4) | unit_code;
                return Some(packed);
            }
        }
    }
    None
}

fn lookup_manifest_flag(attr_name: &str, flag_name: &str) -> Option<u32> {
    if attr_name == "protectionLevel" {
        match flag_name {
            "normal" => Some(0x0),
            "dangerous" => Some(0x1),
            "signature" => Some(0x2),
            "signatureOrSystem" => Some(0x3),
            "privileged" | "system" => Some(0x10),
            "development" => Some(0x20),
            "appop" => Some(0x40),
            "pre23" => Some(0x80),
            "installer" => Some(0x100),
            "verifier" => Some(0x200),
            "preinstalled" => Some(0x400),
            "setup" => Some(0x800),
            "instant" => Some(0x1000),
            "runtime" => Some(0x2000),
            "oem" => Some(0x4000),
            "custom" => Some(0x8000),
            "vendorPrivileged" => Some(0x10000),
            "companion" => Some(0x80000),
            "retailDemo" => Some(0x1000000),
            "recents" => Some(0x2000000),
            "role" => Some(0x4000000),
            "knownSigner" => Some(0x8000000),
            _ => None,
        }
    } else if attr_name == "configChanges" {
        match flag_name {
            "mcc" => Some(0x0001),
            "mnc" => Some(0x0002),
            "locale" => Some(0x0004),
            "touchscreen" => Some(0x0008),
            "keyboard" => Some(0x0010),
            "keyboardHidden" => Some(0x0020),
            "navigation" => Some(0x0040),
            "orientation" => Some(0x0080),
            "screenLayout" => Some(0x0800),
            "uiMode" => Some(0x0200),
            "screenSize" => Some(0x0400),
            "smallestScreenSize" => Some(0x1000),
            "density" => Some(0x1000),
            "layoutDirection" => Some(0x2000),
            "colorMode" => Some(0x4000),
            "fontScale" => Some(0x4000_0000),
            _ => None,
        }
    } else if attr_name == "windowSoftInputMode" {
        match flag_name {
            "stateUnspecified" => Some(0),
            "stateUnchanged" => Some(1),
            "stateHidden" => Some(2),
            "stateAlwaysHidden" => Some(3),
            "stateVisible" => Some(4),
            "stateAlwaysVisible" => Some(5),
            "adjustUnspecified" => Some(0x00),
            "adjustResize" => Some(0x10),
            "adjustPan" => Some(0x20),
            "adjustNothing" => Some(0x30),
            _ => None,
        }
    } else {
        None
    }
}

fn lookup_manifest_enum(attr_name: &str, enum_name: &str) -> Option<u32> {
    if attr_name == "protectionLevel" {
        match enum_name {
            "normal" => Some(0),
            "dangerous" => Some(1),
            "signature" => Some(2),
            "signatureOrSystem" => Some(3),
            _ => None,
        }
    } else if attr_name == "launchMode" {
        match enum_name {
            "standard" => Some(0),
            "singleTop" => Some(1),
            "singleTask" => Some(2),
            "singleInstance" => Some(3),
            "singleInstancePerTask" => Some(4),
            _ => None,
        }
    } else if attr_name == "screenOrientation" {
        match enum_name {
            "unspecified" => Some(0xffff_ffff),
            "landscape" => Some(0),
            "portrait" => Some(1),
            "user" => Some(2),
            "behind" => Some(3),
            "sensor" => Some(4),
            "nosensor" => Some(5),
            "sensorLandscape" => Some(6),
            "sensorPortrait" => Some(7),
            "reverseLandscape" => Some(8),
            "reversePortrait" => Some(9),
            "fullSensor" => Some(10),
            "userLandscape" => Some(11),
            "userPortrait" => Some(12),
            "fullUser" => Some(13),
            "locked" => Some(14),
            _ => None,
        }
    } else if attr_name == "windowSoftInputMode" {
        match enum_name {
            "stateUnspecified" => Some(0),
            "stateUnchanged" => Some(1),
            "stateHidden" => Some(2),
            "stateAlwaysHidden" => Some(3),
            "stateVisible" => Some(4),
            "stateAlwaysVisible" => Some(5),
            "adjustUnspecified" => Some(0x00),
            "adjustResize" => Some(0x10),
            "adjustPan" => Some(0x20),
            "adjustNothing" => Some(0x30),
            _ => None,
        }
    } else {
        None
    }
}
