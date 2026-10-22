use crorust_android::protocol::chunks::AndroidChunk;
use crorust_android::resources::{
    compile_mipmap_resources, compile_resources, ResourceCatalogTable, ResourceSymbols,
};
use std::io::Cursor;

#[test]
fn test_compile_resources_table() {
    let mut symbols = ResourceSymbols::new();
    symbols.add_symbol("string", "app_name", 0x7f010001);
    symbols.add_symbol("drawable", "ic_notification", 0x7f020001);

    let chunk = compile_resources(
        "com.crorust.app",
        Some("icon"),
        Some("Theme.AppCompat.NoActionBar"),
        Some(&symbols),
    )
    .expect("resource table compilation should succeed");

    let mut out = Vec::new();
    chunk.write(&mut out).expect("write arsc chunk");
    assert!(!out.is_empty());
    assert_eq!(&out[0..2], &[0x02, 0x00]);

    let mut cursor = Cursor::new(&out);
    let parsed = AndroidChunk::parse(&mut cursor).expect("parse arsc table");

    let mut table = ResourceCatalogTable::default();
    table.import_chunk(&parsed);

    let attr_id = table
        .lookup_attribute_id("windowNoTitle")
        .expect("windowNoTitle lookup");
    assert_eq!(attr_id, 0x01010058);
}

#[test]
fn test_mipmap_density_variants() {
    let mipmap = compile_mipmap_resources("com.crorust.app", "icon").expect("mipmap spec creation");
    let variants: Vec<_> = mipmap.variants().collect();
    assert_eq!(variants.len(), 5);

    assert_eq!(variants[0], ("res/icon/icon48.png".to_string(), 48));
    assert_eq!(variants[1], ("res/icon/icon72.png".to_string(), 72));
    assert_eq!(variants[2], ("res/icon/icon96.png".to_string(), 96));
    assert_eq!(variants[3], ("res/icon/icon144.png".to_string(), 144));
    assert_eq!(variants[4], ("res/icon/icon192.png".to_string(), 192));
}

#[test]
fn test_sparse_table_type_chunk_parsing() {
    use crorust_android::protocol::chunks::{
        ResourceTableEntry, ResourceTableValue, ResourceTypeHeader, TypedResourceValue,
    };
    use crorust_android::protocol::ResourceValueType;

    let config = vec![0u8; 48];
    let entries_start = 20 + config.len() as u32 + 8; // header(68) + 2 entries * 4 bytes

    // Sparse entry 0: idx = 3, offset = 0 (word 0) -> raw_val: (0 << 16) | 3
    let raw_0 = 3u32;
    // Sparse entry 1: idx = 10, offset = 16 bytes = 4 words -> raw_val: (4 << 16) | 10
    let raw_1 = (4u32 << 16) | 10u32;
    let offsets = vec![raw_0, raw_1];

    let entry_3 = ResourceTableEntry {
        entry_byte_size: 8,
        entry_flags: 0,
        key_string_index: 30,
        table_value: ResourceTableValue::Simple(TypedResourceValue::new(
            ResourceValueType::String,
            100,
        )),
    };
    let entry_10 = ResourceTableEntry {
        entry_byte_size: 8,
        entry_flags: 0,
        key_string_index: 100,
        table_value: ResourceTableValue::Simple(TypedResourceValue::new(
            ResourceValueType::String,
            200,
        )),
    };

    let mut entries = vec![None; 11];
    entries[3] = Some(entry_3.clone());
    entries[10] = Some(entry_10.clone());

    let chunk = AndroidChunk::TableType(
        ResourceTypeHeader {
            type_id_number: 1,
            configuration_flags: 0x01, // FLAG_SPARSE
            reserved_word: 0,
            total_entry_count: 2,
            entries_byte_offset: entries_start,
            config_payload: config,
        },
        offsets,
        entries,
    );

    let mut buf = Vec::new();
    chunk.write(&mut buf).expect("write sparse TableType");

    let parsed = AndroidChunk::parse(&mut Cursor::new(&buf)).expect("parse sparse TableType");
    if let AndroidChunk::TableType(header, parsed_offsets, parsed_entries) = parsed {
        assert_eq!(header.configuration_flags, 0x01);
        assert_eq!(header.total_entry_count, 2);
        assert_eq!(parsed_offsets.len(), 2);
        assert_eq!(parsed_entries.len(), 11);
        assert_eq!(parsed_entries[3], Some(entry_3));
        assert_eq!(parsed_entries[10], Some(entry_10));
        assert!(parsed_entries[0].is_none());
        assert!(parsed_entries[1].is_none());
        assert!(parsed_entries[2].is_none());
        assert!(parsed_entries[4].is_none());
    } else {
        panic!("expected AndroidChunk::TableType");
    }
}

#[test]
fn test_offset16_table_type_chunk_parsing() {
    use crorust_android::protocol::chunks::{
        ResourceTableEntry, ResourceTableValue, ResourceTypeHeader, TypedResourceValue,
    };
    use crorust_android::protocol::ResourceValueType;

    let config = vec![0u8; 48];
    // header(68) + 3 offsets * 2 bytes (6 bytes) + 2 bytes padding = 76
    let entries_start = 20 + config.len() as u32 + 8;

    // Index 0: offset 0 (0 words)
    // Index 1: NO_ENTRY (0xffff)
    // Index 2: offset 16 (4 words)
    let offsets = vec![0u32, 0xffff, 4];

    let entry_0 = ResourceTableEntry {
        entry_byte_size: 8,
        entry_flags: 0,
        key_string_index: 1,
        table_value: ResourceTableValue::Simple(TypedResourceValue::new(
            ResourceValueType::String,
            1,
        )),
    };
    let entry_2 = ResourceTableEntry {
        entry_byte_size: 8,
        entry_flags: 0,
        key_string_index: 2,
        table_value: ResourceTableValue::Simple(TypedResourceValue::new(
            ResourceValueType::String,
            2,
        )),
    };

    let entries = vec![Some(entry_0.clone()), None, Some(entry_2.clone())];

    let chunk = AndroidChunk::TableType(
        ResourceTypeHeader {
            type_id_number: 2,
            configuration_flags: 0x02, // FLAG_OFFSET16
            reserved_word: 0,
            total_entry_count: 3,
            entries_byte_offset: entries_start,
            config_payload: config,
        },
        offsets,
        entries,
    );

    let mut buf = Vec::new();
    chunk.write(&mut buf).expect("write offset16 TableType");

    let parsed = AndroidChunk::parse(&mut Cursor::new(&buf)).expect("parse offset16 TableType");
    if let AndroidChunk::TableType(header, _parsed_offsets, parsed_entries) = parsed {
        assert_eq!(header.configuration_flags, 0x02);
        assert_eq!(header.total_entry_count, 3);
        assert_eq!(parsed_entries.len(), 3);
        assert_eq!(parsed_entries[0], Some(entry_0));
        assert!(parsed_entries[1].is_none());
        assert_eq!(parsed_entries[2], Some(entry_2));
    } else {
        panic!("expected AndroidChunk::TableType");
    }
}

#[test]
fn test_corrupt_count_allocation_guard() {
    use byteorder::{LittleEndian, WriteBytesExt};
    use crorust_android::protocol::chunks::ResourceTableEntry;

    let mut buf = Vec::new();
    buf.write_u16::<LittleEndian>(16).unwrap(); // entry_byte_size
    buf.write_u16::<LittleEndian>(0x0001).unwrap(); // FLAG_COMPLEX
    buf.write_u32::<LittleEndian>(0).unwrap(); // key_string_index
    buf.write_u32::<LittleEndian>(0).unwrap(); // parent
    buf.write_u32::<LittleEndian>(0x7fff_ffff).unwrap(); // corrupt count: 2 billion items

    let res = ResourceTableEntry::read(&mut Cursor::new(&buf));
    assert!(
        res.is_err(),
        "ResourceTableEntry::read must return error on count > 65536 instead of crashing with OOM"
    );
}

#[test]
fn test_compile_resources_with_string_values() {
    let mut symbols = ResourceSymbols::new();
    // Simulate real AAR resource layout:
    // "string" type at type_id = 12 (0x0c)
    // androidx_startup at entry_idx = 28 (0x001c) -> id = 0x7f0c001c
    symbols.types = vec![
        "anim".into(),
        "attr".into(),
        "bool".into(),
        "color".into(),
        "dimen".into(),
        "drawable".into(),
        "id".into(),
        "integer".into(),
        "interpolator".into(),
        "layout".into(),
        "mipmap".into(),
        "string".into(),
        "style".into(),
    ];
    symbols.add_symbol("string", "androidx_startup", 0x7f0c001c);
    symbols.string_values.insert(
        "androidx_startup".to_string(),
        "androidx.startup".to_string(),
    );
    symbols
        .string_values
        .insert("app_name".to_string(), "My Cool App".to_string());

    let chunk = compile_resources(
        "com.crorust.app",
        Some("icon"),
        Some("Theme.AppCompat.NoActionBar"),
        Some(&symbols),
    )
    .expect("compile resources with strings should succeed");

    let mut out = Vec::new();
    chunk.write(&mut out).expect("write arsc chunk");
    assert!(!out.is_empty());

    let mut cursor = Cursor::new(&out);
    let parsed = AndroidChunk::parse(&mut cursor).expect("parse arsc table");

    // Verify global string pool contains "androidx.startup"
    if let AndroidChunk::Table(_, ref packages) = parsed {
        if let Some(AndroidChunk::StringPool(ref global_strings, _)) = packages.first() {
            assert!(
                global_strings.contains(&"androidx.startup".to_string()),
                "global string pool must contain 'androidx.startup'"
            );
            assert!(
                global_strings.contains(&"My Cool App".to_string()),
                "global string pool must contain 'My Cool App'"
            );
        } else {
            panic!("expected global string pool as first table child");
        }

        // Verify package chunks contain TableTypeSpec and TableType for type_id = 12 (0x0c)
        if let Some(AndroidChunk::TablePackage(_, ref children)) = packages.get(1) {
            let mut found_spec = false;
            let mut found_type = false;

            for child in children {
                if let AndroidChunk::TableTypeSpec(header, specs) = child {
                    if header.type_id_number == 12 {
                        found_spec = true;
                        assert!(
                            header.total_entry_count >= 29,
                            "type spec must have at least 29 entries for 0x1c"
                        );
                        assert_eq!(specs.len(), header.total_entry_count as usize);
                    }
                } else if let AndroidChunk::TableType(header, offsets, entries) = child {
                    if header.type_id_number == 12 {
                        found_type = true;
                        assert!(
                            header.total_entry_count >= 29,
                            "type chunk must have at least 29 entries for 0x1c"
                        );
                        assert_ne!(
                            offsets[28], 0xffff_ffff,
                            "offset for entry 28 (androidx_startup) must not be NO_ENTRY"
                        );
                        let entry = entries[28]
                            .as_ref()
                            .expect("entry 28 must be Some(ResourceTableEntry)");
                        if let crorust_android::protocol::chunks::ResourceTableValue::Simple(tv) =
                            &entry.table_value
                        {
                            assert_eq!(
                                tv.data_type_tag,
                                crorust_android::protocol::ResourceValueType::String as u8
                            );
                        } else {
                            panic!("expected Simple string value for androidx_startup");
                        }
                    }
                }
            }

            assert!(found_spec, "string TableTypeSpec (id=12) must be present");
            assert!(found_type, "string TableType (id=12) must be present");
        } else {
            panic!("expected TablePackage as second table child");
        }
    } else {
        panic!("expected AndroidChunk::Table");
    }

    let mut table = ResourceCatalogTable::default();
    table.import_chunk(&parsed);

    let res = table
        .entry_by_ref(crorust_android::resources::ResourceIdentifierReference {
            package: Some("com.crorust.app"),
            ty: "string",
            name: "androidx_startup",
        })
        .expect("androidx_startup lookup");
    assert_eq!(u32::from(res.id()), 0x7f0c001c);
}
