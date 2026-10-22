use super::manifest_dom::{
    AndroidCustomPermissionDescriptor, AndroidMetaDataDescriptor, AndroidPermissionDescriptor,
    AndroidProviderDescriptor, AndroidXmlManifestDescriptor,
};
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub fn merge_manifests(
    base: &mut AndroidXmlManifestDescriptor,
    manifest_paths: &[impl AsRef<Path>],
) -> Result<()> {
    let package_name = base.package.clone().unwrap_or_else(|| "app".to_string());

    // Resolve any placeholders in the base manifest
    for perm in &mut base.uses_permission {
        if let Some(ref mut name) = perm.name {
            if name.contains("${applicationId}") {
                *name = name.replace("${applicationId}", &package_name);
            }
        }
    }
    for perm in &mut base.permission {
        if let Some(ref mut name) = perm.name {
            if name.contains("${applicationId}") {
                *name = name.replace("${applicationId}", &package_name);
            }
        }
    }
    for prov in &mut base.application.providers {
        if let Some(ref mut auth) = prov.authorities {
            if auth.contains("${applicationId}") {
                *auth = auth.replace("${applicationId}", &package_name);
            }
        }
    }

    let mut existing_permissions: HashSet<String> = base
        .uses_permission
        .iter()
        .filter_map(|p| p.name.clone())
        .collect();

    let mut existing_custom_permissions: HashSet<String> = base
        .permission
        .iter()
        .filter_map(|p| p.name.clone())
        .collect();

    for path in manifest_paths {
        let path = path.as_ref();
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read AAR manifest at `{}`", path.display()))?;

        parse_and_merge_aar_manifest(
            base,
            &content,
            &package_name,
            &mut existing_permissions,
            &mut existing_custom_permissions,
        )?;
    }

    Ok(())
}

fn merge_permission_element(
    e: &quick_xml::events::BytesStart<'_>,
    base: &mut AndroidXmlManifestDescriptor,
    package_name: &str,
    existing_permissions: &mut HashSet<String>,
) -> Result<()> {
    for attr in e.attributes().filter_map(|a| a.ok()) {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = attr.unescape_value()?.to_string();
        let val = val.replace("${applicationId}", package_name);

        if key.ends_with("name") && !existing_permissions.contains(&val) {
            existing_permissions.insert(val.clone());
            base.uses_permission.push(AndroidPermissionDescriptor {
                name: Some(val),
                max_sdk_version: None,
            });
        }
    }
    Ok(())
}

fn merge_custom_permission_element(
    e: &quick_xml::events::BytesStart<'_>,
    base: &mut AndroidXmlManifestDescriptor,
    package_name: &str,
    existing_custom_permissions: &mut HashSet<String>,
) -> Result<()> {
    let mut name = None;
    let mut protection_level = None;
    for attr in e.attributes().filter_map(|a| a.ok()) {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = attr.unescape_value()?.to_string();
        let val = val.replace("${applicationId}", package_name);

        if key.ends_with("name") {
            name = Some(val);
        } else if key.ends_with("protectionLevel") {
            protection_level = Some(val);
        }
    }
    if let Some(n) = name {
        if !existing_custom_permissions.contains(&n) {
            existing_custom_permissions.insert(n.clone());
            base.permission.push(AndroidCustomPermissionDescriptor {
                name: Some(n),
                protection_level,
            });
        }
    }
    Ok(())
}

fn parse_and_merge_aar_manifest(
    base: &mut AndroidXmlManifestDescriptor,
    content: &str,
    package_name: &str,
    existing_permissions: &mut HashSet<String>,
    existing_custom_permissions: &mut HashSet<String>,
) -> Result<()> {
    let sanitized_content = content.replace("${applicationId}", package_name);
    let mut reader = Reader::from_str(&sanitized_content);
    let mut current_provider: Option<AndroidProviderDescriptor> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"uses-permission" => {
                    merge_permission_element(e, base, package_name, existing_permissions)?;
                }
                b"permission" => {
                    merge_custom_permission_element(
                        e,
                        base,
                        package_name,
                        existing_custom_permissions,
                    )?;
                }
                b"provider" => {
                    let mut name = String::new();
                    let mut authorities = String::new();
                    let mut exported = None;

                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = attr.unescape_value()?.to_string();
                        let val = val.replace("${applicationId}", package_name);

                        if key.ends_with("name") {
                            name = val;
                        } else if key.ends_with("authorities") {
                            authorities = val;
                        } else if key.ends_with("exported") {
                            exported = Some(val == "true");
                        }
                    }

                    if !name.is_empty() {
                        current_provider = Some(AndroidProviderDescriptor {
                            name: Some(name),
                            authorities: if authorities.is_empty() {
                                None
                            } else {
                                Some(authorities)
                            },
                            exported,
                            grant_uri_permissions: None,
                            meta_data: Vec::new(),
                        });
                    }
                }
                b"meta-data" => {
                    if let Some(ref mut provider) = current_provider {
                        let mut name = String::new();
                        let mut value = String::new();

                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = attr.unescape_value()?.to_string();

                            if key.ends_with("name") {
                                name = val;
                            } else if key.ends_with("value") {
                                value = val;
                            }
                        }

                        if !name.is_empty() {
                            provider.meta_data.push(AndroidMetaDataDescriptor {
                                name: Some(name),
                                value: if value.is_empty() { None } else { Some(value) },
                                resource: None,
                            });
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) => match e.name().as_ref() {
                b"uses-permission" => {
                    merge_permission_element(e, base, package_name, existing_permissions)?;
                }
                b"permission" => {
                    merge_custom_permission_element(
                        e,
                        base,
                        package_name,
                        existing_custom_permissions,
                    )?;
                }
                b"provider" => {
                    let mut name = String::new();
                    let mut authorities = String::new();
                    let mut exported = None;

                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = attr.unescape_value()?.to_string();
                        let val = val.replace("${applicationId}", package_name);

                        if key.ends_with("name") {
                            name = val;
                        } else if key.ends_with("authorities") {
                            authorities = val;
                        } else if key.ends_with("exported") {
                            exported = Some(val == "true");
                        }
                    }

                    if !name.is_empty() {
                        add_or_merge_provider(
                            &mut base.application.providers,
                            AndroidProviderDescriptor {
                                name: Some(name),
                                authorities: if authorities.is_empty() {
                                    None
                                } else {
                                    Some(authorities)
                                },
                                exported,
                                grant_uri_permissions: None,
                                meta_data: Vec::new(),
                            },
                        );
                    }
                }
                b"meta-data" => {
                    if let Some(ref mut provider) = current_provider {
                        let mut name = String::new();
                        let mut value = String::new();

                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = attr.unescape_value()?.to_string();

                            if key.ends_with("name") {
                                name = val;
                            } else if key.ends_with("value") {
                                value = val;
                            }
                        }

                        if !name.is_empty() {
                            provider.meta_data.push(AndroidMetaDataDescriptor {
                                name: Some(name),
                                value: if value.is_empty() { None } else { Some(value) },
                                resource: None,
                            });
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"provider" {
                    if let Some(provider) = current_provider.take() {
                        add_or_merge_provider(&mut base.application.providers, provider);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => anyhow::bail!("Error parsing AAR manifest: {}", e),
            _ => {}
        }
    }

    Ok(())
}

fn add_or_merge_provider(
    providers: &mut Vec<AndroidProviderDescriptor>,
    new_prov: AndroidProviderDescriptor,
) {
    if let Some(existing) = providers.iter_mut().find(|p| p.name == new_prov.name) {
        if existing.authorities.as_ref().is_none_or(|a| a.is_empty()) {
            existing.authorities = new_prov.authorities;
        }
        if existing.exported.is_none() {
            existing.exported = new_prov.exported;
        }
        for md in new_prov.meta_data {
            if !existing.meta_data.iter().any(|m| m.name == md.name) {
                existing.meta_data.push(md);
            }
        }
    } else {
        providers.push(new_prov);
    }
}

fn parse_single_aar_strings(res_dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut strings = BTreeMap::new();
    let values_dir = res_dir.join("values");
    if !values_dir.is_dir() {
        return Ok(strings);
    }

    let entries = match std::fs::read_dir(&values_dir) {
        Ok(e) => e,
        Err(_) => return Ok(strings),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "xml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut reader = Reader::from_str(&content);
                let mut current_name: Option<String> = None;
                let mut buf = Vec::new();

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(ref e)) if e.name().as_ref() == b"string" => {
                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                if attr.key.as_ref() == b"name" {
                                    if let Ok(val) = attr.unescape_value() {
                                        current_name = Some(val.to_string());
                                    }
                                }
                            }
                        }
                        Ok(Event::Text(ref e)) => {
                            if let Some(ref name) = current_name {
                                if let Ok(text) = e.unescape() {
                                    let text_clean = text.trim().to_string();
                                    strings.entry(name.clone()).or_insert(text_clean);
                                }
                            }
                        }
                        Ok(Event::End(ref e)) if e.name().as_ref() == b"string" => {
                            current_name = None;
                        }
                        Ok(Event::Eof) => break,
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(strings)
}

pub fn collect_aar_strings<P: AsRef<Path> + Sync>(
    res_dirs: &[P],
) -> Result<BTreeMap<String, String>> {
    let string_maps: Vec<Result<BTreeMap<String, String>>> = res_dirs
        .par_iter()
        .map(|res_dir| parse_single_aar_strings(res_dir.as_ref()))
        .collect();

    let mut strings = BTreeMap::new();
    for map_res in string_maps {
        for (k, v) in map_res? {
            strings.entry(k).or_insert(v);
        }
    }

    Ok(strings)
}

pub fn collect_aar_file_resources(
    res_dirs: &[impl AsRef<Path>],
) -> Result<BTreeMap<(String, String), (PathBuf, String)>> {
    let mut files_map = BTreeMap::new();

    for res_dir in res_dirs {
        let res_path = res_dir.as_ref();
        if !res_path.is_dir() {
            continue;
        }

        let subdirs = match std::fs::read_dir(res_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for subdir_entry in subdirs.filter_map(|e| e.ok()) {
            let subdir_path = subdir_entry.path();
            if !subdir_path.is_dir() {
                continue;
            }

            let folder_name = subdir_entry.file_name().to_string_lossy().to_string();

            if folder_name.starts_with("values") {
                continue;
            }

            let res_type = folder_name
                .split('-')
                .next()
                .unwrap_or(&folder_name)
                .to_string();

            let file_entries = match std::fs::read_dir(&subdir_path) {
                Ok(fe) => fe,
                Err(_) => continue,
            };

            for file_entry in file_entries.filter_map(|e| e.ok()) {
                let file_path = file_entry.path();
                if !file_path.is_file() {
                    continue;
                }

                let file_name = file_entry.file_name().to_string_lossy().to_string();
                let mut base_name = file_name.clone();
                for ext in &[".9.png", ".xml", ".png", ".webp"] {
                    if base_name.ends_with(ext) {
                        base_name = base_name[..base_name.len() - ext.len()].to_string();
                        break;
                    }
                }

                let apk_rel_path = format!("res/{}/{}", folder_name, file_name);
                files_map.insert((res_type.clone(), base_name), (file_path, apk_rel_path));
            }
        }
    }

    Ok(files_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_providers_and_permissions() -> Result<()> {
        let mut manifest = AndroidXmlManifestDescriptor {
            package: Some("com.example.game".into()),
            ..Default::default()
        };

        let aar_manifest = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="androidx.emoji2" >
    <uses-permission android:name="android.permission.INTERNET" />
    <application>
        <provider
            android:name="androidx.startup.InitializationProvider"
            android:authorities="${applicationId}.androidx-startup"
            android:exported="false">
            <meta-data
                android:name="androidx.emoji2.text.EmojiCompatInitializer"
                android:value="androidx.startup" />
        </provider>
    </application>
</manifest>"#;

        let tmp = tempfile::tempdir()?;
        let p = tmp.path().join("AndroidManifest.xml");
        std::fs::write(&p, aar_manifest)?;

        merge_manifests(&mut manifest, &[&p])?;

        assert_eq!(manifest.uses_permission.len(), 1);
        assert_eq!(
            manifest.uses_permission[0].name.as_deref(),
            Some("android.permission.INTERNET")
        );

        assert_eq!(manifest.application.providers.len(), 1);
        let prov = &manifest.application.providers[0];
        assert_eq!(
            prov.name.as_deref(),
            Some("androidx.startup.InitializationProvider")
        );
        assert_eq!(
            prov.authorities.as_deref(),
            Some("com.example.game.androidx-startup")
        );
        assert_eq!(prov.exported, Some(false));
        assert_eq!(prov.meta_data.len(), 1);
        assert_eq!(
            prov.meta_data[0].name.as_deref(),
            Some("androidx.emoji2.text.EmojiCompatInitializer")
        );

        Ok(())
    }

    #[test]
    fn test_merge_uses_permission_start_tag_and_nine_patch_stripping() -> Result<()> {
        let mut manifest = AndroidXmlManifestDescriptor {
            package: Some("com.example.app".into()),
            ..Default::default()
        };

        let aar_manifest = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.lib">
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE"></uses-permission>
    <uses-permission android:name="android.permission.CAMERA" />
</manifest>"#;

        let tmp = tempfile::tempdir()?;
        let p = tmp.path().join("AndroidManifest.xml");
        std::fs::write(&p, aar_manifest)?;

        merge_manifests(&mut manifest, &[&p])?;
        assert_eq!(manifest.uses_permission.len(), 2);
        assert!(manifest
            .uses_permission
            .iter()
            .any(|p| p.name.as_deref() == Some("android.permission.ACCESS_NETWORK_STATE")));
        assert!(manifest
            .uses_permission
            .iter()
            .any(|p| p.name.as_deref() == Some("android.permission.CAMERA")));

        let res_dir = tmp.path().join("res");
        let drawable_dir = res_dir.join("drawable-hdpi");
        std::fs::create_dir_all(&drawable_dir)?;
        std::fs::write(drawable_dir.join("btn_bg.9.png"), b"fake_png")?;
        std::fs::write(drawable_dir.join("icon.png"), b"fake_png")?;

        let file_resources = collect_aar_file_resources(&[res_dir])?;
        assert!(file_resources.contains_key(&("drawable".to_string(), "btn_bg".to_string())));
        assert!(!file_resources.contains_key(&("drawable".to_string(), "btn_bg.9".to_string())));
        assert!(file_resources.contains_key(&("drawable".to_string(), "icon".to_string())));

        Ok(())
    }

    #[test]
    fn test_collect_aar_strings() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let res_dir = tmp.path().join("res");
        let values_dir = res_dir.join("values");
        std::fs::create_dir_all(&values_dir)?;

        let xml_content = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="androidx_startup" translatable="false">androidx.startup</string>
    <string name="app_name">My App</string>
</resources>"#;
        std::fs::write(values_dir.join("strings.xml"), xml_content)?;

        let strings = collect_aar_strings(&[res_dir])?;
        assert_eq!(
            strings.get("androidx_startup"),
            Some(&"androidx.startup".to_string())
        );
        assert_eq!(strings.get("app_name"), Some(&"My App".to_string()));

        Ok(())
    }

    #[test]
    fn test_merge_application_id_placeholder_in_permissions_and_custom_permissions() -> Result<()> {
        let mut manifest = AndroidXmlManifestDescriptor {
            package: Some("com.example.app".into()),
            ..Default::default()
        };

        let aar_manifest = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="androidx.core">
    <permission
        android:name="${applicationId}.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION"
        android:protectionLevel="signature" />
    <uses-permission android:name="${applicationId}.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION" />
</manifest>"#;

        let tmp = tempfile::tempdir()?;
        let p = tmp.path().join("AndroidManifest.xml");
        std::fs::write(&p, aar_manifest)?;

        merge_manifests(&mut manifest, &[&p])?;

        assert_eq!(manifest.permission.len(), 1);
        assert_eq!(
            manifest.permission[0].name.as_deref(),
            Some("com.example.app.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION")
        );
        assert_eq!(
            manifest.permission[0].protection_level.as_deref(),
            Some("signature")
        );

        assert_eq!(manifest.uses_permission.len(), 1);
        assert_eq!(
            manifest.uses_permission[0].name.as_deref(),
            Some("com.example.app.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION")
        );
        assert!(!manifest.uses_permission[0]
            .name
            .as_ref()
            .unwrap()
            .contains('$'));

        Ok(())
    }
}
