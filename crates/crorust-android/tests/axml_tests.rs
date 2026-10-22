use crorust_android::manifest::{
    compile_manifest, compile_xml, ActivityComponentDescriptor, AndroidCustomPermissionDescriptor,
    AndroidIntentFilterDescriptor, AndroidXmlManifestDescriptor,
};
use crorust_android::package::{build_axml_tree, find_entry_point};
use crorust_android::protocol::chunk_stream::ResourceValueType;
use crorust_android::protocol::chunks::AndroidChunk;
use crorust_android::resources::ResourceCatalogTable;
use std::io::Cursor;

#[test]
fn test_compile_empty_manifest() {
    let manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.cleanroom".to_string()),
        version_code: Some(42),
        version_name: Some("1.2.3".to_string()),
        ..Default::default()
    };

    let bytes = compile_manifest(&manifest).expect("compile_manifest should succeed");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[0..2], &[0x03, 0x00]);

    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parsed AXML root chunk");
    if let AndroidChunk::Xml(children) = chunk {
        assert!(children.len() >= 2);
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_manifest_entry_point_discovery() {
    let mut manifest = AndroidXmlManifestDescriptor {
        package: Some("org.crorust.sample".to_string()),
        ..Default::default()
    };

    let mut act = ActivityComponentDescriptor {
        name: Some(".MainActivity".to_string()),
        ..Default::default()
    };

    let mut filter = AndroidIntentFilterDescriptor::default();
    filter
        .actions
        .push("android.intent.action.MAIN".to_string());
    filter
        .categories
        .push("android.intent.category.LAUNCHER".to_string());
    act.intent_filters.push(filter);

    manifest.application.activities.push(act);

    let bytes = compile_manifest(&manifest).expect("compilation failed");
    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parse failed");

    if let AndroidChunk::Xml(children) = chunk {
        let entry = find_entry_point(&children).expect("should discover entry point");
        assert_eq!(entry.package, "org.crorust.sample");
        assert_eq!(entry.activity, "org.crorust.sample.MainActivity");
    } else {
        panic!("expected XML chunk");
    }
}

#[test]
fn test_compile_xml_with_namespaces() {
    let raw_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"
    android:layout_width="match_parent"
    android:layout_height="match_parent"
    android:orientation="vertical">
    <TextView
        android:id="@+id/text_title"
        android:text="CroRust Native"
        android:layout_width="wrap_content"
        android:layout_height="wrap_content" />
</LinearLayout>"#;

    let table = ResourceCatalogTable::default();
    let chunk = compile_xml(raw_xml, &table).expect("compile_xml succeeded");

    let mut out = Vec::new();
    chunk.write(&mut out).expect("write chunk");
    assert!(!out.is_empty());

    let mut cursor = Cursor::new(&out);
    let parsed = AndroidChunk::parse(&mut cursor).expect("re-parse binary chunk");
    if let AndroidChunk::Xml(children) = parsed {
        let strings = if let Some(AndroidChunk::StringPool(s, _)) = children.first() {
            s
        } else {
            panic!("missing string pool");
        };
        let tree = build_axml_tree(&children, strings).expect("build tree");
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "LinearLayout");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].name, "TextView");
    } else {
        panic!("expected XML chunk");
    }
}

fn ruxt_template_path(rel: &str) -> std::path::PathBuf {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/crorust.ruxt");
    base.join(rel)
}

#[test]
fn test_merge_real_androidx_core_manifest_and_compile() {
    let mut manifest = AndroidXmlManifestDescriptor {
        package: Some("crorust.ruxt".to_string()),
        ..Default::default()
    };

    let path = ruxt_template_path(
        ".cache/target/crorust/debug/android/unpacked_aars/androidx.core-core-1.19.1/AndroidManifest.xml",
    );
    if path.exists() {
        crorust_android::manifest::merge_manifests(&mut manifest, &[path])
            .expect("merge manifests");
        for perm in &manifest.uses_permission {
            let name = perm.name.as_ref().unwrap();
            assert!(
                !name.contains("${applicationId}"),
                "Uses permission still contains raw macro: {}",
                name
            );
            assert!(
                !name.contains('$'),
                "Uses permission contains invalid dollar char: {}",
                name
            );
        }
        let bytes = compile_manifest(&manifest).expect("compile_manifest succeeded");
        assert!(!bytes.is_empty());
        let mut cursor = Cursor::new(&bytes);
        let chunk = AndroidChunk::parse(&mut cursor).expect("parsed AXML");
        if let AndroidChunk::Xml(children) = chunk {
            if let Some(AndroidChunk::StringPool(strings, _)) = children.first() {
                for s in strings {
                    assert!(
                        !s.contains("${applicationId}"),
                        "String pool contains unreplaced macro: {}",
                        s
                    );
                }
            }
        }
    }
}

#[test]
fn test_resource_map_string_pool_alignment() {
    use crorust_android::manifest::AndroidApplicationDescriptor;
    use crorust_android::resources::attribute_index::lookup_framework_attribute;

    let manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.alignment_test".to_string()),
        version_code: Some(101),
        version_name: Some("1.0.1".to_string()),
        compile_sdk_version: Some(34),
        application: AndroidApplicationDescriptor {
            theme: Some("@style/Theme.AppCompat.NoActionBar".to_string()),
            icon: Some("@mipmap/icon".to_string()),
            label: Some("TestApp".to_string()),
            debuggable: Some(true),
            ..Default::default()
        },
        ..Default::default()
    };

    let bytes = compile_manifest(&manifest).expect("compile_manifest succeeded");
    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parse AXML root");

    if let AndroidChunk::Xml(children) = chunk {
        let (strings, res_map) = match (&children[0], &children[1]) {
            (AndroidChunk::StringPool(s, _), AndroidChunk::XmlResourceMap(m)) => (s, m),
            _ => panic!("Expected StringPool followed by XmlResourceMap"),
        };

        // Invariant: The first res_map.len() entries in strings must match the resource IDs in res_map 1:1
        assert!(!res_map.is_empty(), "Resource map must not be empty");
        for (i, &res_id) in res_map.iter().enumerate() {
            let attr_name = &strings[i];
            let expected_id = lookup_framework_attribute(attr_name);
            assert!(
                expected_id.is_some(),
                "StringPool[{i}] = '{attr_name}' must be a known framework attribute"
            );
            assert_eq!(
                res_id,
                expected_id.unwrap(),
                "res_map[{i}] must match expected resource ID for '{attr_name}'"
            );
        }

        // Non-resource attribute names (like "package") and element tags ("manifest", "application")
        // must be at string pool indices >= res_map.len()
        let package_idx = strings
            .iter()
            .position(|s| s == "package")
            .expect("package string in pool");
        assert!(
            package_idx >= res_map.len(),
            "package string index ({package_idx}) must be >= res_map.len() ({}) to prevent accidental resource ID lookup",
            res_map.len()
        );

        let manifest_tag_idx = strings
            .iter()
            .position(|s| s == "manifest")
            .expect("manifest tag in pool");
        assert!(
            manifest_tag_idx >= res_map.len(),
            "manifest tag index must be >= res_map.len()"
        );
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_theme_and_icon_typed_value_references() {
    use crorust_android::manifest::AndroidApplicationDescriptor;
    use crorust_android::protocol::chunk_stream::ResourceValueType;

    let manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.refs".to_string()),
        application: AndroidApplicationDescriptor {
            theme: Some("@style/Theme.AppCompat.NoActionBar".to_string()),
            icon: Some("@mipmap/icon".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let bytes = compile_manifest(&manifest).expect("compile_manifest succeeded");
    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parse AXML root");

    if let AndroidChunk::Xml(children) = chunk {
        let strings = if let Some(AndroidChunk::StringPool(s, _)) = children.first() {
            s
        } else {
            panic!("missing string pool");
        };

        let mut found_theme = false;
        let mut found_icon = false;

        for c in &children {
            if let AndroidChunk::XmlStartElement(_node, start_elem, attrs) = c {
                let elem_name = &strings[start_elem.element_name_index as usize];
                if elem_name == "application" {
                    for attr in attrs {
                        let attr_name = &strings[attr.attribute_name_index as usize];
                        if attr_name == "theme" {
                            found_theme = true;
                            assert_eq!(
                                attr.typed_value_entry.data_type_tag,
                                ResourceValueType::Reference as u8,
                                "android:theme must be compiled as TYPE_REFERENCE (0x1), got: 0x{:x}",
                                attr.typed_value_entry.data_type_tag
                            );
                            assert_ne!(
                                attr.typed_value_entry.data_payload, 0,
                                "android:theme reference ID must be non-zero"
                            );
                            assert_eq!(
                                attr.raw_string_index, -1,
                                "raw_string_index should be -1 for typed references"
                            );
                        } else if attr_name == "icon" {
                            found_icon = true;
                            assert_eq!(
                                attr.typed_value_entry.data_type_tag,
                                ResourceValueType::Reference as u8,
                                "android:icon must be compiled as TYPE_REFERENCE (0x1), got: 0x{:x}",
                                attr.typed_value_entry.data_type_tag
                            );
                            assert_ne!(
                                attr.typed_value_entry.data_payload, 0,
                                "android:icon reference ID must be non-zero"
                            );
                        }
                    }
                }
            }
        }

        assert!(found_theme, "Must find android:theme in <application>");
        assert!(found_icon, "Must find android:icon in <application>");
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_custom_permission_protection_level_typed_value() {
    let mut manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.perms".to_string()),
        ..Default::default()
    };

    manifest.permission.push(AndroidCustomPermissionDescriptor {
        name: Some("com.example.perms.SIG".to_string()),
        protection_level: Some("signature".to_string()),
    });

    manifest.permission.push(AndroidCustomPermissionDescriptor {
        name: Some("com.example.perms.MULTI".to_string()),
        protection_level: Some("signature|privileged".to_string()),
    });

    let bytes = compile_manifest(&manifest).expect("compile_manifest should succeed");
    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parsed AXML root chunk");

    if let AndroidChunk::Xml(children) = chunk {
        let strings = if let Some(AndroidChunk::StringPool(s, _)) = children.first() {
            s
        } else {
            panic!("missing string pool");
        };

        let mut checked_sig = false;
        let mut checked_multi = false;

        for c in &children {
            if let AndroidChunk::XmlStartElement(_node, start_elem, attrs) = c {
                let elem_name = &strings[start_elem.element_name_index as usize];
                if elem_name == "permission" {
                    let mut perm_name = None;
                    let mut prot_attr = None;
                    for attr in attrs {
                        let attr_name = &strings[attr.attribute_name_index as usize];
                        if attr_name == "name" {
                            let val_idx = if attr.raw_string_index >= 0 {
                                attr.raw_string_index as usize
                            } else {
                                attr.typed_value_entry.data_payload as usize
                            };
                            perm_name = Some(&strings[val_idx]);
                        } else if attr_name == "protectionLevel" {
                            prot_attr = Some(attr);
                        }
                    }

                    if perm_name == Some(&"com.example.perms.SIG".to_string()) {
                        let prot = prot_attr.expect("protectionLevel attribute missing for SIG");
                        assert_eq!(
                            prot.typed_value_entry.data_type_tag,
                            ResourceValueType::IntDec as u8,
                            "Single value 'signature' should be encoded as IntDec (0x10), got 0x{:02x}",
                            prot.typed_value_entry.data_type_tag
                        );
                        assert_eq!(
                            prot.typed_value_entry.data_payload, 2,
                            "protectionLevel 'signature' data_payload should be 2, got {}",
                            prot.typed_value_entry.data_payload
                        );
                        assert_eq!(
                            prot.raw_string_index, -1,
                            "raw_string_index should be -1 for typed integers"
                        );
                        checked_sig = true;
                    } else if perm_name == Some(&"com.example.perms.MULTI".to_string()) {
                        let prot = prot_attr.expect("protectionLevel attribute missing for MULTI");
                        assert_eq!(
                            prot.typed_value_entry.data_type_tag,
                            ResourceValueType::IntHex as u8,
                            "Flag combination 'signature|privileged' should be encoded as IntHex (0x11), got 0x{:02x}",
                            prot.typed_value_entry.data_type_tag
                        );
                        // signature (0x2) | privileged (0x10) = 0x12
                        assert_eq!(
                            prot.typed_value_entry.data_payload, 0x12,
                            "protectionLevel 'signature|privileged' data_payload should be 0x12, got 0x{:x}",
                            prot.typed_value_entry.data_payload
                        );
                        assert_eq!(
                            prot.raw_string_index, -1,
                            "raw_string_index should be -1 for typed integers"
                        );
                        checked_multi = true;
                    }
                }
            }
        }

        assert!(checked_sig, "Must have checked com.example.perms.SIG");
        assert!(checked_multi, "Must have checked com.example.perms.MULTI");
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_ruxt_apk_manifest_protection_level() {
    let apk_path = ruxt_template_path(".cache/target/crorust/debug/android/ruxt.apk");
    if !apk_path.exists() {
        return;
    }
    let file = std::fs::File::open(apk_path).expect("open apk");
    let mut zip = zip::ZipArchive::new(file).expect("zip archive");
    let mut manifest_entry = zip.by_name("AndroidManifest.xml").expect("manifest in zip");
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut manifest_entry, &mut bytes).expect("read manifest");

    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parse AXML root");

    if let AndroidChunk::Xml(children) = chunk {
        let strings = if let Some(AndroidChunk::StringPool(s, _)) = children.first() {
            s
        } else {
            panic!("missing string pool");
        };

        let mut found_protection_level = false;

        for c in &children {
            if let AndroidChunk::XmlStartElement(_node, start_elem, attrs) = c {
                let elem_name = &strings[start_elem.element_name_index as usize];
                if elem_name == "permission" {
                    for attr in attrs {
                        let attr_name = &strings[attr.attribute_name_index as usize];
                        if attr_name == "protectionLevel" {
                            found_protection_level = true;
                            assert_eq!(
                                attr.typed_value_entry.data_type_tag,
                                ResourceValueType::IntDec as u8,
                                "protectionLevel must be IntDec (0x10), got 0x{:02x}",
                                attr.typed_value_entry.data_type_tag
                            );
                            assert_eq!(
                                attr.typed_value_entry.data_payload, 2,
                                "protectionLevel data_payload must be 2, got {}",
                                attr.typed_value_entry.data_payload
                            );
                            assert_eq!(
                                attr.raw_string_index, -1,
                                "raw_string_index must be -1 for typed values"
                            );
                        }
                    }
                }
            }
        }

        assert!(
            found_protection_level,
            "Must find android:protectionLevel in ruxt.apk"
        );
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_compile_manifest_with_android_jar() {
    let jar_path =
        ruxt_template_path(".cache/crorust/Android.sdk/platforms/android-35/android.jar");
    if !jar_path.exists() {
        return;
    }
    let mut table = ResourceCatalogTable::default();
    table.import_apk(&jar_path).expect("import android.jar");

    let mut manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.perms".to_string()),
        ..Default::default()
    };
    manifest.permission.push(AndroidCustomPermissionDescriptor {
        name: Some("com.example.perms.SIG".to_string()),
        protection_level: Some("signature".to_string()),
    });

    let xml = quick_xml::se::to_string(&manifest).expect("serialize manifest");
    let chunk = crorust_android::manifest::compile_xml_with_symbols(&xml, &table, None)
        .expect("compile xml");

    let mut out = Vec::new();
    chunk.write(&mut out).expect("write chunk");
    let mut cursor = Cursor::new(&out);
    let parsed = AndroidChunk::parse(&mut cursor).expect("parse chunk");

    if let AndroidChunk::Xml(children) = parsed {
        let strings = if let Some(AndroidChunk::StringPool(s, _)) = children.first() {
            s
        } else {
            panic!("missing string pool");
        };

        let mut found_protection_level = false;

        for c in &children {
            if let AndroidChunk::XmlStartElement(_node, start_elem, attrs) = c {
                let elem_name = &strings[start_elem.element_name_index as usize];
                if elem_name == "permission" {
                    for attr in attrs {
                        let attr_name = &strings[attr.attribute_name_index as usize];
                        if attr_name == "protectionLevel" {
                            found_protection_level = true;
                            assert_eq!(
                                attr.typed_value_entry.data_type_tag,
                                ResourceValueType::IntDec as u8,
                                "protectionLevel must be IntDec (0x10), got 0x{:02x}",
                                attr.typed_value_entry.data_type_tag
                            );
                            assert_eq!(
                                attr.typed_value_entry.data_payload, 2,
                                "protectionLevel data_payload must be 2, got {}",
                                attr.typed_value_entry.data_payload
                            );
                            assert_eq!(
                                attr.raw_string_index, -1,
                                "raw_string_index must be -1 for typed values"
                            );
                        }
                    }
                }
            }
        }

        assert!(
            found_protection_level,
            "Must find android:protectionLevel in compiled XML"
        );
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_inspect_abc_vector_test() {
    let apk_path = ruxt_template_path(".cache/target/crorust/debug/android/ruxt.apk");
    if !apk_path.exists() {
        return;
    }
    let file = std::fs::File::open(apk_path).expect("open apk");
    let mut zip = zip::ZipArchive::new(file).expect("zip archive");
    let mut entry = zip
        .by_name("res/drawable/abc_vector_test.xml")
        .expect("entry in zip");
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut bytes).expect("read");

    let mut cursor = Cursor::new(&bytes);
    let chunk = AndroidChunk::parse(&mut cursor).expect("parse AXML root");
    if let AndroidChunk::Xml(children) = chunk {
        let (strings, _res_map) = match (&children[0], &children[1]) {
            (AndroidChunk::StringPool(s, _), AndroidChunk::XmlResourceMap(m)) => (s, m),
            _ => panic!("Expected StringPool followed by XmlResourceMap"),
        };
        let mut verified_fill_color = false;
        for c in &children {
            if let AndroidChunk::XmlStartElement(_node, start_elem, attrs) = c {
                let elem_name = &strings[start_elem.element_name_index as usize];
                if elem_name == "path" {
                    for a in attrs {
                        let a_name = &strings[a.attribute_name_index as usize];
                        if a_name == "fillColor" {
                            assert_eq!(
                                a.typed_value_entry.data_type_tag,
                                crorust_android::protocol::ResourceValueType::Reference as u8,
                                "fillColor in ruxt.apk must be TYPE_REFERENCE"
                            );
                            assert_eq!(
                                a.typed_value_entry.data_payload,
                                0x0106_000b,
                                "fillColor in ruxt.apk must resolve to @android:color/white (0x0106000b)"
                            );
                            verified_fill_color = true;
                        }
                    }
                }
            }
        }
        assert!(
            verified_fill_color,
            "Must find and verify fillColor in abc_vector_test.xml"
        );
    }
}

#[test]
fn test_lookup_android_color_white() {
    let jar_path =
        ruxt_template_path(".cache/crorust/Android.sdk/platforms/android-35/android.jar");
    if !jar_path.exists() {
        return;
    }
    use crorust_android::resources::ResourceIdentifierReference;
    let mut table = ResourceCatalogTable::default();
    table.import_apk(&jar_path).expect("import android.jar");

    let entry = table
        .entry_by_ref(ResourceIdentifierReference::parse("@android:color/white").unwrap())
        .expect("find @android:color/white");
    assert_eq!(
        u32::from(entry.id()),
        0x0106_000b,
        "@android:color/white must resolve to 0x0106000b"
    );
}

#[test]
fn test_compile_vector_drawable_color_white_reference() {
    let jar_path =
        ruxt_template_path(".cache/crorust/Android.sdk/platforms/android-35/android.jar");
    if !jar_path.exists() {
        return;
    }
    let mut table = ResourceCatalogTable::default();
    table.import_apk(&jar_path).expect("import android.jar");

    let xml = r#"<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24.0"
    android:viewportHeight="24.0">
    <path
        android:fillColor="@android:color/white"
        android:pathData="M20,11L7.8,11l5.6,-5.6L12,4l-8,8l8,8l1.4,-1.4L7.8,13L20,13L20,11z"/>
</vector>"#;

    let chunk = crorust_android::manifest::compile_xml_with_symbols(xml, &table, None)
        .expect("compile vector xml");
    let mut bytes = Vec::new();
    chunk.write(&mut bytes).expect("write chunk");

    let mut cursor = Cursor::new(&bytes);
    let parsed = AndroidChunk::parse(&mut cursor).expect("parse compiled AXML");

    if let AndroidChunk::Xml(children) = parsed {
        let strings = if let Some(AndroidChunk::StringPool(s, _)) = children.first() {
            s
        } else {
            panic!("missing string pool");
        };

        let mut found_fill_color = false;
        for c in &children {
            if let AndroidChunk::XmlStartElement(_node, start_elem, attrs) = c {
                let elem_name = &strings[start_elem.element_name_index as usize];
                if elem_name == "path" {
                    for attr in attrs {
                        let attr_name = &strings[attr.attribute_name_index as usize];
                        if attr_name == "fillColor" {
                            found_fill_color = true;
                            assert_eq!(
                                attr.typed_value_entry.data_type_tag,
                                ResourceValueType::Reference as u8,
                                "fillColor must be TYPE_REFERENCE (0x1), got 0x{:02x}",
                                attr.typed_value_entry.data_type_tag
                            );
                            assert_eq!(
                                attr.typed_value_entry.data_payload,
                                0x0106_000b,
                                "fillColor payload must be @android:color/white (0x0106000b), got 0x{:08x}",
                                attr.typed_value_entry.data_payload
                            );
                        }
                    }
                }
            }
        }
        assert!(
            found_fill_color,
            "Must find android:fillColor in compiled vector"
        );
    } else {
        panic!("expected AndroidChunk::Xml");
    }
}

#[test]
fn test_compact_entry_roundtrip() {
    use crorust_android::protocol::chunk_stream::ResourceValueType;
    use crorust_android::protocol::chunks::{
        ResourceTableEntry, ResourceTableValue, TypedResourceValue,
    };

    // Simulate an ARSC compact color entry: key=100, type=IntColorArgb8 (0x1c), data=0xffffffff (white)
    let compact_entry = ResourceTableEntry {
        entry_byte_size: 8,
        entry_flags: ResourceTableEntry::FLAG_COMPACT
            | ResourceTableEntry::FLAG_PUBLIC
            | ((ResourceValueType::IntColorArgb8 as u16) << 8),
        key_string_index: 100,
        table_value: ResourceTableValue::Simple(TypedResourceValue::new(
            ResourceValueType::IntColorArgb8,
            0xffff_ffff,
        )),
    };

    let mut buf = Vec::new();
    compact_entry.write(&mut buf).expect("write compact entry");
    assert_eq!(
        buf.len(),
        8,
        "Compact entry must be exactly 8 bytes on wire"
    );

    let mut cursor = Cursor::new(&buf);
    let decoded = ResourceTableEntry::read(&mut cursor).expect("read compact entry");

    assert_eq!(decoded.key_string_index, 100);
    assert_eq!(decoded.entry_byte_size, 8);
    assert_eq!(decoded.entry_flags, compact_entry.entry_flags);
    if let ResourceTableValue::Simple(tv) = decoded.table_value {
        assert_eq!(tv.data_type_tag, ResourceValueType::IntColorArgb8 as u8);
        assert_eq!(tv.data_payload, 0xffff_ffff);
    } else {
        panic!("Expected Simple table value for compact entry");
    }
}

#[test]
fn test_lookup_framework_resource_static() {
    use crorust_android::resources::{
        lookup_framework_attribute, lookup_framework_color, lookup_framework_id,
        lookup_framework_resource,
    };

    assert_eq!(lookup_framework_color("white"), Some(0x0106_000b));
    assert_eq!(lookup_framework_color("black"), Some(0x0106_000c));
    assert_eq!(lookup_framework_color("transparent"), Some(0x0106_000d));

    assert_eq!(lookup_framework_attribute("name"), Some(0x0101_0003));
    assert_eq!(
        lookup_framework_attribute("protectionLevel"),
        Some(0x0101_0009)
    );

    assert_eq!(lookup_framework_id("content"), Some(0x0102_0002));

    assert_eq!(
        lookup_framework_resource("color", "white"),
        Some(0x0106_000b)
    );
    assert_eq!(
        lookup_framework_resource("attr", "theme"),
        Some(0x0101_0000)
    );
    assert_eq!(
        lookup_framework_resource("id", "content"),
        Some(0x0102_0002)
    );
    assert_eq!(lookup_framework_resource("unknown", "foo"), None);
}
