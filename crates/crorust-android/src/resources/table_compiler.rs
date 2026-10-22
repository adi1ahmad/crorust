use super::attribute_index::{
    lookup_framework_attribute, ATTR_WINDOW_ACTION_BAR, ATTR_WINDOW_ACTION_BAR_OVERLAY,
    ATTR_WINDOW_NO_TITLE,
};
use super::symbol_catalog::{ResourceIdentifierReference, ResourceSymbols};
use crate::protocol::chunk_stream::ResourceValueType;
use crate::protocol::chunks::{
    AndroidChunk, ResourceIdentifier, ResourcePackageHeader, ResourceTableEntry,
    ResourceTableMapEntry, ResourceTableValue, ResourceTypeHeader, ResourceTypeSpecHeader,
    TypedResourceValue,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

const ICON_DENSITIES: [(u16, u32); 5] = [(160, 48), (240, 72), (320, 96), (480, 144), (640, 192)];

#[derive(Clone, Debug)]
pub struct MipmapResourceRecord {
    pub package_name: String,
    pub icon_name: String,
}

impl MipmapResourceRecord {
    pub fn variants(&self) -> impl Iterator<Item = (String, u32)> + '_ {
        let icon = &self.icon_name;
        ICON_DENSITIES
            .iter()
            .map(move |&(_dpi, size)| (format!("res/{icon}/{icon}{size}.png"), size))
    }
}

pub fn compile_mipmap_resources(
    package_name: &str,
    icon_name: &str,
) -> Result<MipmapResourceRecord> {
    Ok(MipmapResourceRecord {
        package_name: package_name.to_string(),
        icon_name: icon_name.to_string(),
    })
}

pub fn compile_resources(
    package_name: &str,
    icon_name: Option<&str>,
    theme_name: Option<&str>,
    symbols: Option<&ResourceSymbols>,
) -> Result<AndroidChunk> {
    let mut global_strings: Vec<String> = if let Some(icon) = icon_name {
        ICON_DENSITIES
            .iter()
            .map(|&(_dpi, size)| format!("res/{icon}/{icon}{size}.png"))
            .collect()
    } else {
        Vec::new()
    };
    let mut global_string_map: BTreeMap<String, u32> = global_strings
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), i as u32))
        .collect();

    let mut get_or_add_global_string = |val: &str| -> u32 {
        if let Some(&idx) = global_string_map.get(val) {
            idx
        } else {
            let idx = global_strings.len() as u32;
            global_strings.push(val.to_string());
            global_string_map.insert(val.to_string(), idx);
            idx
        }
    };

    let types = if let Some(sym) = symbols {
        let mut t = sym.types.clone();
        if icon_name.is_some() && !t.contains(&"mipmap".to_string()) {
            t.push("mipmap".to_string());
        }
        if !t.contains(&"attr".to_string()) {
            t.push("attr".to_string());
        }
        if !t.contains(&"style".to_string()) {
            t.push("style".to_string());
        }
        if !t.contains(&"string".to_string()) && !sym.string_values.is_empty() {
            t.push("string".to_string());
        }
        t
    } else {
        vec![
            "mipmap".to_string(),
            "attr".to_string(),
            "style".to_string(),
            "string".to_string(),
        ]
    };

    let mut key_strings = Vec::new();
    let mut key_map: BTreeMap<String, u32> = BTreeMap::new();

    let mut get_or_add_key = |name: &str| -> u32 {
        if let Some(&idx) = key_map.get(name) {
            idx
        } else {
            let idx = key_strings.len() as u32;
            key_strings.push(name.to_string());
            key_map.insert(name.to_string(), idx);
            idx
        }
    };

    if icon_name.is_some() {
        get_or_add_key("icon");
    }

    let _win_action_bar_key = get_or_add_key("windowActionBar");
    let _win_no_title_key = get_or_add_key("windowNoTitle");
    let _win_action_bar_overlay_key = get_or_add_key("windowActionBarOverlay");

    let default_theme = "Theme_AppCompat_NoActionBar";
    let active_theme = theme_name.unwrap_or(default_theme);
    let theme_norm = active_theme.replace('.', "_");
    let theme_key = get_or_add_key(&theme_norm);
    if active_theme.contains('.') {
        get_or_add_key(active_theme);
    }

    if let Some(sym) = symbols {
        for (_, name) in sym.id_map.keys() {
            get_or_add_key(name);
        }
        for name in sym.string_values.keys() {
            get_or_add_key(name);
        }
        for (_, name) in sym.file_resources.keys() {
            get_or_add_key(name);
        }
    }

    let win_action_bar_id = symbols
        .and_then(|s| s.get_attribute_id("windowActionBar"))
        .unwrap_or_else(|| {
            let attr_idx = types
                .iter()
                .position(|t| t == "attr")
                .map(|i| (i + 1) as u32)
                .unwrap_or(2);
            (0x7f << 24) | (attr_idx << 16) | 1
        });

    let win_no_title_id = symbols
        .and_then(|s| s.get_attribute_id("windowNoTitle"))
        .unwrap_or_else(|| {
            let attr_idx = types
                .iter()
                .position(|t| t == "attr")
                .map(|i| (i + 1) as u32)
                .unwrap_or(2);
            (0x7f << 24) | (attr_idx << 16) | 2
        });

    let win_action_bar_overlay_id = symbols
        .and_then(|s| s.get_attribute_id("windowActionBarOverlay"))
        .unwrap_or_else(|| {
            let attr_idx = types
                .iter()
                .position(|t| t == "attr")
                .map(|i| (i + 1) as u32)
                .unwrap_or(2);
            (0x7f << 24) | (attr_idx << 16) | 3
        });

    let theme_bag = vec![
        ResourceTableMapEntry {
            map_key_id: ATTR_WINDOW_NO_TITLE,
            entry_value: TypedResourceValue::new(ResourceValueType::IntBoolean, 0xffff_ffff),
        },
        ResourceTableMapEntry {
            map_key_id: ATTR_WINDOW_ACTION_BAR,
            entry_value: TypedResourceValue::new(ResourceValueType::IntBoolean, 0),
        },
        ResourceTableMapEntry {
            map_key_id: ATTR_WINDOW_ACTION_BAR_OVERLAY,
            entry_value: TypedResourceValue::new(ResourceValueType::IntBoolean, 0),
        },
        ResourceTableMapEntry {
            map_key_id: win_action_bar_id,
            entry_value: TypedResourceValue::new(ResourceValueType::IntBoolean, 0),
        },
        ResourceTableMapEntry {
            map_key_id: win_no_title_id,
            entry_value: TypedResourceValue::new(ResourceValueType::IntBoolean, 0xffff_ffff),
        },
        ResourceTableMapEntry {
            map_key_id: win_action_bar_overlay_id,
            entry_value: TypedResourceValue::new(ResourceValueType::IntBoolean, 0),
        },
    ];

    let mut type_chunks = Vec::new();

    for (type_idx, type_name) in types.iter().enumerate() {
        let type_id = (type_idx + 1) as u8;

        if type_name == "mipmap" && icon_name.is_some() {
            let icon_entry_idx = symbols
                .and_then(|s| {
                    s.id_map
                        .get(&("mipmap".to_string(), "icon".to_string()))
                        .map(|id| (*id & 0xffff) as usize)
                })
                .unwrap_or(0);
            let entry_count = (icon_entry_idx + 1) as u32;

            let mut specs = vec![0u32; entry_count as usize];
            specs[icon_entry_idx] = 256;
            type_chunks.push(AndroidChunk::TableTypeSpec(
                ResourceTypeSpecHeader {
                    type_id_number: type_id,
                    reserved_zero_byte: 0,
                    reserved_zero_short: 0,
                    total_entry_count: entry_count,
                },
                specs,
            ));

            let icon_key = key_map.get("icon").copied().unwrap_or(0);
            for (string_id, &(density, _size)) in ICON_DENSITIES.iter().enumerate() {
                let mut index = vec![0xffff_ffff; entry_count as usize];
                let mut entries = vec![None; entry_count as usize];
                index[icon_entry_idx] = 0;
                entries[icon_entry_idx] = Some(ResourceTableEntry {
                    entry_byte_size: 8,
                    entry_flags: 0,
                    key_string_index: icon_key,
                    table_value: ResourceTableValue::Simple(TypedResourceValue::new(
                        ResourceValueType::String,
                        string_id as u32,
                    )),
                });

                let mut config = vec![0u8; 48];
                config[0] = 48;
                config[32] = (density & 0xff) as u8;
                config[33] = (density >> 8) as u8;

                type_chunks.push(AndroidChunk::TableType(
                    ResourceTypeHeader {
                        type_id_number: type_id,
                        configuration_flags: 0,
                        reserved_word: 0,
                        total_entry_count: entry_count,
                        entries_byte_offset: 20 + config.len() as u32 + (index.len() * 4) as u32,
                        config_payload: config,
                    },
                    index,
                    entries,
                ));
            }
        } else if type_name == "style" {
            let theme_entry_idx = symbols
                .and_then(|s| {
                    s.id_map
                        .get(&("style".to_string(), theme_norm.clone()))
                        .or_else(|| {
                            s.id_map
                                .get(&("style".to_string(), active_theme.to_string()))
                        })
                        .map(|id| (*id & 0xffff) as usize)
                })
                .unwrap_or(0);
            let entry_count = (theme_entry_idx + 1) as u32;

            let mut specs = vec![0u32; entry_count as usize];
            specs[theme_entry_idx] = 0;
            type_chunks.push(AndroidChunk::TableTypeSpec(
                ResourceTypeSpecHeader {
                    type_id_number: type_id,
                    reserved_zero_byte: 0,
                    reserved_zero_short: 0,
                    total_entry_count: entry_count,
                },
                specs,
            ));

            let mut index = vec![0xffff_ffff; entry_count as usize];
            let mut entries = vec![None; entry_count as usize];
            index[theme_entry_idx] = 0;
            entries[theme_entry_idx] = Some(ResourceTableEntry {
                entry_byte_size: 16,
                entry_flags: 0x0001,
                key_string_index: theme_key,
                table_value: ResourceTableValue::Complex(0, theme_bag.clone()),
            });

            let mut config = vec![0u8; 48];
            config[0] = 48;

            type_chunks.push(AndroidChunk::TableType(
                ResourceTypeHeader {
                    type_id_number: type_id,
                    configuration_flags: 0,
                    reserved_word: 0,
                    total_entry_count: entry_count,
                    entries_byte_offset: 20 + config.len() as u32 + (index.len() * 4) as u32,
                    config_payload: config,
                },
                index,
                entries,
            ));
        } else if type_name == "string" {
            if let Some(sym) = symbols {
                let mut string_entries: BTreeMap<usize, (String, String)> = BTreeMap::new();
                let mut max_entry_idx = 0usize;

                for ((res_type, name), &id) in &sym.id_map {
                    if res_type == "string" {
                        let entry_idx = (id & 0xffff) as usize;
                        if entry_idx > max_entry_idx {
                            max_entry_idx = entry_idx;
                        }
                        let val = sym
                            .string_values
                            .get(name)
                            .cloned()
                            .unwrap_or_else(|| name.clone());
                        string_entries.insert(entry_idx, (name.clone(), val));
                    }
                }

                for (name, val) in &sym.string_values {
                    let already_mapped = string_entries.values().any(|(n, _)| n == name);
                    if !already_mapped {
                        max_entry_idx += 1;
                        string_entries.insert(max_entry_idx, (name.clone(), val.clone()));
                    }
                }

                if !string_entries.is_empty() {
                    let entry_count = (max_entry_idx + 1) as u32;

                    let specs = vec![0u32; entry_count as usize];
                    type_chunks.push(AndroidChunk::TableTypeSpec(
                        ResourceTypeSpecHeader {
                            type_id_number: type_id,
                            reserved_zero_byte: 0,
                            reserved_zero_short: 0,
                            total_entry_count: entry_count,
                        },
                        specs,
                    ));

                    let mut index = vec![0xffff_ffff; entry_count as usize];
                    let mut entries = vec![None; entry_count as usize];
                    let mut current_offset = 0u32;

                    for (&entry_idx, (name, val)) in &string_entries {
                        let key_idx = key_map.get(name).copied().unwrap_or(0);
                        let global_str_idx = get_or_add_global_string(val);

                        let entry = ResourceTableEntry {
                            entry_byte_size: 8,
                            entry_flags: 0,
                            key_string_index: key_idx,
                            table_value: ResourceTableValue::Simple(TypedResourceValue::new(
                                ResourceValueType::String,
                                global_str_idx,
                            )),
                        };

                        let mut entry_bytes = Vec::new();
                        entry.write(&mut entry_bytes)?;

                        index[entry_idx] = current_offset;
                        current_offset += entry_bytes.len() as u32;
                        entries[entry_idx] = Some(entry);
                    }

                    let mut config = vec![0u8; 48];
                    config[0] = 48;

                    type_chunks.push(AndroidChunk::TableType(
                        ResourceTypeHeader {
                            type_id_number: type_id,
                            configuration_flags: 0,
                            reserved_word: 0,
                            total_entry_count: entry_count,
                            entries_byte_offset: 20
                                + config.len() as u32
                                + (index.len() * 4) as u32,
                            config_payload: config,
                        },
                        index,
                        entries,
                    ));
                }
            }
        } else if let Some(sym) = symbols {
            let matching_files: Vec<_> = sym
                .file_resources
                .iter()
                .filter(|((t, _), _)| t == type_name)
                .collect();

            if !matching_files.is_empty() {
                let mut file_entries: BTreeMap<usize, (String, String)> = BTreeMap::new();
                let mut max_entry_idx = 0usize;

                for ((t, name), rel_path) in &matching_files {
                    let entry_idx = sym
                        .id_map
                        .get(&(t.clone(), name.clone()))
                        .map(|id| (*id & 0xffff) as usize);

                    let idx = if let Some(e_idx) = entry_idx {
                        e_idx
                    } else {
                        max_entry_idx += 1;
                        max_entry_idx
                    };

                    if idx > max_entry_idx {
                        max_entry_idx = idx;
                    }
                    file_entries.insert(idx, (name.clone(), (*rel_path).clone()));
                }

                if !file_entries.is_empty() {
                    let entry_count = (max_entry_idx + 1) as u32;

                    let specs = vec![0u32; entry_count as usize];
                    type_chunks.push(AndroidChunk::TableTypeSpec(
                        ResourceTypeSpecHeader {
                            type_id_number: type_id,
                            reserved_zero_byte: 0,
                            reserved_zero_short: 0,
                            total_entry_count: entry_count,
                        },
                        specs,
                    ));

                    let mut index = vec![0xffff_ffff; entry_count as usize];
                    let mut entries = vec![None; entry_count as usize];
                    let mut current_offset = 0u32;

                    for (&entry_idx, (name, rel_path)) in &file_entries {
                        let key_idx = key_map.get(name).copied().unwrap_or(0);
                        let global_str_idx = get_or_add_global_string(rel_path);

                        let entry = ResourceTableEntry {
                            entry_byte_size: 8,
                            entry_flags: 0,
                            key_string_index: key_idx,
                            table_value: ResourceTableValue::Simple(TypedResourceValue::new(
                                ResourceValueType::String,
                                global_str_idx,
                            )),
                        };

                        let mut entry_bytes = Vec::new();
                        entry.write(&mut entry_bytes)?;

                        index[entry_idx] = current_offset;
                        current_offset += entry_bytes.len() as u32;
                        entries[entry_idx] = Some(entry);
                    }

                    let mut config = vec![0u8; 48];
                    config[0] = 48;

                    type_chunks.push(AndroidChunk::TableType(
                        ResourceTypeHeader {
                            type_id_number: type_id,
                            configuration_flags: 0,
                            reserved_word: 0,
                            total_entry_count: entry_count,
                            entries_byte_offset: 20
                                + config.len() as u32
                                + (index.len() * 4) as u32,
                            config_payload: config,
                        },
                        index,
                        entries,
                    ));
                }
            }
        }
    }

    let type_pool_chunk = AndroidChunk::StringPool(types.clone(), Vec::new());
    let mut type_pool_bytes = Vec::new();
    type_pool_chunk.write(&mut type_pool_bytes)?;

    let type_strings_offset = 288u32;
    let key_strings_offset = 288u32 + type_pool_bytes.len() as u32;
    let last_public_type_id = types.len() as u32;
    let last_public_key_id = key_strings.len() as u32;

    let mut pkg_children = vec![
        AndroidChunk::StringPool(types, Vec::new()),
        AndroidChunk::StringPool(key_strings, Vec::new()),
    ];
    pkg_children.extend(type_chunks);

    let pkg_header = ResourcePackageHeader {
        package_id: 0x7f,
        package_name: package_name.to_string(),
        type_strings_offset,
        last_public_type_id,
        key_strings_offset,
        last_public_key_id,
        type_id_offset_val: 0,
    };

    let table_package = AndroidChunk::TablePackage(pkg_header, pkg_children);
    let table = AndroidChunk::Table(
        1,
        vec![
            AndroidChunk::StringPool(global_strings, Vec::new()),
            table_package,
        ],
    );

    Ok(table)
}

#[derive(Clone, Copy, Debug)]
pub struct ResourceCatalogEntry<'a> {
    entry: &'a ResourceTableEntry,
    id: ResourceIdentifier,
}

impl<'a> ResourceCatalogEntry<'a> {
    pub fn id(self) -> ResourceIdentifier {
        self.id
    }

    pub fn attribute_type_raw(self) -> u32 {
        if let ResourceTableValue::Complex(_, entries) = &self.entry.table_value {
            if !entries.is_empty() {
                return entries[0].entry_value.data_payload;
            }
        }
        0
    }

    pub fn lookup_value(&self, id: ResourceIdentifier) -> Option<TypedResourceValue> {
        if let ResourceTableValue::Complex(_, entries) = &self.entry.table_value {
            for entry in entries {
                if entry.map_key_id == u32::from(id) {
                    return Some(entry.entry_value);
                }
            }
        }
        None
    }
}

#[derive(Debug, Default)]
pub struct ResourceCatalogTable {
    package_chunks: Vec<AndroidChunk>,
}

impl ResourceCatalogTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn import_apk(&mut self, apk: &Path) -> Result<()> {
        let resources = crorust_common::unpack_single_entry_secure(apk, "resources.arsc")
            .with_context(|| format!("failed to extract resources.arsc from {}", apk.display()))?;
        let chunk = AndroidChunk::parse(&mut Cursor::new(resources))
            .with_context(|| "failed to parse resources.arsc chunk")?;
        self.import_chunk(&chunk);
        Ok(())
    }

    pub fn import_chunk(&mut self, chunk: &AndroidChunk) {
        if let AndroidChunk::Table(_, packages) = chunk {
            self.package_chunks.extend_from_slice(packages);
        }
    }

    pub fn lookup_attribute_id(&self, name: &str) -> Result<u32> {
        if let Some(id) = lookup_framework_attribute(name) {
            return Ok(id);
        }
        let entry = self.entry_by_ref(ResourceIdentifierReference::attr(name))?;
        Ok(u32::from(entry.id()))
    }

    pub fn entry_by_ref(&self, r: ResourceIdentifierReference) -> Result<ResourceCatalogEntry<'_>> {
        let pkg_id = if let Some(pkg) = r.package {
            if pkg == "android" {
                1
            } else {
                0x7f
            }
        } else {
            0x7f
        };

        for chunk in &self.package_chunks {
            if let AndroidChunk::TablePackage(header, children) = chunk {
                if header.package_id == pkg_id as u32
                    || (pkg_id == 1 && header.package_name == "android")
                {
                    return self.find_entry_in_package(children, pkg_id, r.ty, r.name);
                }
            }
        }

        bail!("Resource '{}' not found in table", r.name)
    }

    fn find_entry_in_package<'a>(
        &'a self,
        children: &'a [AndroidChunk],
        pkg_id: u8,
        ty_name: &str,
        entry_name: &str,
    ) -> Result<ResourceCatalogEntry<'a>> {
        let types = if let Some(AndroidChunk::StringPool(strings, _)) = children.first() {
            strings
        } else {
            bail!("missing type strings pool");
        };

        let keys = if let Some(AndroidChunk::StringPool(strings, _)) = children.get(1) {
            strings
        } else {
            bail!("missing key strings pool");
        };

        let ty_idx = types
            .iter()
            .position(|t| t == ty_name)
            .map(|i| (i + 1) as u8)
            .with_context(|| format!("type '{ty_name}' not found in package"))?;

        let norm_name = entry_name.replace('.', "_");
        let key_idx = keys
            .iter()
            .position(|k| k == entry_name || k == &norm_name)
            .with_context(|| format!("key '{entry_name}' not found in package"))?
            as u32;

        for chunk in &children[2..] {
            if let AndroidChunk::TableType(header, _offsets, entries) = chunk {
                if header.type_id_number == ty_idx {
                    for (entry_idx, entry_opt) in entries.iter().enumerate() {
                        if let Some(entry) = entry_opt {
                            if entry.key_string_index == key_idx {
                                let res_ref =
                                    ResourceIdentifier::new(pkg_id, ty_idx, entry_idx as u16);
                                return Ok(ResourceCatalogEntry { entry, id: res_ref });
                            }
                        }
                    }
                }
            }
        }

        bail!("Entry '{entry_name}' not found in type '{ty_name}'")
    }
}
