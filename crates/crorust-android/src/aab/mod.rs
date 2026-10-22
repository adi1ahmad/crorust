pub mod proto;
pub mod sign;

use crate::manifest::AndroidXmlManifestDescriptor;
use crate::resources::ResourceSymbols;
use anyhow::{Context, Result};
use crorust_common::{
    ArchiveEntryOptions, BoundaryZipArchive, IdentityKeypair, RasterResizer, TransformConstraint,
};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub struct Aab {
    manifest: AndroidXmlManifestDescriptor,
    path: PathBuf,
    entries: BTreeMap<String, (Vec<u8>, ArchiveEntryOptions)>,
}

impl Aab {
    pub fn new(path: PathBuf, manifest: AndroidXmlManifestDescriptor) -> Result<Self> {
        Ok(Self {
            manifest,
            path,
            entries: BTreeMap::new(),
        })
    }

    pub fn add_bundle_config(&mut self) -> Result<()> {
        let config_bytes = proto::encode_bundle_config()?;
        self.entries.insert(
            "BundleConfig.pb".to_string(),
            (config_bytes, ArchiveEntryOptions::Compressed),
        );
        Ok(())
    }

    pub fn add_manifest(&mut self) -> Result<()> {
        let manifest_proto = proto::encode_manifest_xml(&self.manifest)?;
        self.entries.insert(
            "base/manifest/AndroidManifest.xml".to_string(),
            (manifest_proto, ArchiveEntryOptions::Compressed),
        );
        Ok(())
    }

    pub fn add_resources(
        &mut self,
        icon: Option<&Path>,
        symbols: Option<&ResourceSymbols>,
    ) -> Result<()> {
        let has_icon = icon.is_some();
        let res_table_bytes = proto::encode_resource_table(&self.manifest, has_icon, symbols)?;
        self.entries.insert(
            "base/resources.pb".to_string(),
            (res_table_bytes, ArchiveEntryOptions::Compressed),
        );

        if let Some(path) = icon {
            let mut scaler = RasterResizer::open(path)?;
            scaler.optimize();

            let pkg = self.manifest.package.as_deref().unwrap_or("app");
            let mipmap = crate::resources::compile_mipmap_resources(pkg, "icon")?;
            for (name, size) in mipmap.variants() {
                let mut buf = Vec::new();
                let mut cursor = Cursor::new(&mut buf);
                scaler.write(&mut cursor, TransformConstraint::new(size))?;

                let path_ref: &Path = name.as_ref();
                let norm_rel = crorust_common::normalize_zip_entry_path(path_ref)?;
                let dest = format!("base/{}", norm_rel);
                self.entries
                    .insert(dest, (buf, ArchiveEntryOptions::Compressed));
            }
        }
        Ok(())
    }

    pub fn add_dex(&mut self, dex_path: &Path, dex_index: usize) -> Result<()> {
        let file_name = if dex_index == 0 {
            "classes.dex".to_string()
        } else {
            format!("classes{}.dex", dex_index + 1)
        };
        let dest = format!("base/dex/{}", file_name);
        let bytes = std::fs::read(dex_path)
            .with_context(|| format!("Failed to read DEX file: {:?}", dex_path))?;
        self.entries
            .insert(dest, (bytes, ArchiveEntryOptions::Compressed));
        Ok(())
    }

    pub fn add_lib(&mut self, abi: &str, lib_path: &Path) -> Result<()> {
        let lib_name = lib_path
            .file_name()
            .context("Missing library filename")?
            .to_str()
            .context("Non-UTF8 library filename")?;
        let dest = format!("base/lib/{}/{}", abi, lib_name);
        let bytes = std::fs::read(lib_path)
            .with_context(|| format!("Failed to read shared library: {:?}", lib_path))?;
        self.entries
            .insert(dest, (bytes, ArchiveEntryOptions::Unaligned));
        Ok(())
    }

    pub fn add_asset(&mut self, source_path: &Path, rel_dest: &Path) -> Result<()> {
        let dest = if rel_dest.as_os_str().is_empty() || rel_dest == Path::new(".") {
            let file_name = source_path
                .file_name()
                .with_context(|| {
                    format!(
                        "Asset source path `{}` has no valid filename component",
                        source_path.display()
                    )
                })?
                .to_str()
                .with_context(|| {
                    format!(
                        "Asset filename `{}` contains invalid UTF-8",
                        source_path.display()
                    )
                })?;
            format!("base/assets/{}", file_name)
        } else {
            let norm_rel = crorust_common::normalize_zip_entry_path(rel_dest)?;
            format!("base/assets/{}", norm_rel)
        };
        let bytes = std::fs::read(source_path)
            .with_context(|| format!("Failed reading asset: {:?}", source_path))?;
        self.entries
            .insert(dest, (bytes, ArchiveEntryOptions::Compressed));
        Ok(())
    }

    pub fn finish(mut self, signer: Option<IdentityKeypair>) -> Result<()> {
        let jar_sig_files = sign::generate_jar_signature(&self.entries, signer)?;
        for (name, sig_data) in jar_sig_files {
            self.entries
                .insert(name, (sig_data, ArchiveEntryOptions::Compressed));
        }

        let mut zip = BoundaryZipArchive::new(&self.path, true)?;
        for (name, (data, opts)) in self.entries {
            zip.create_file(Path::new(&name), opts, &data)?;
        }
        zip.finish()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{
        ActivityComponentDescriptor, AndroidApplicationDescriptor, AndroidMetaDataDescriptor,
        AndroidPermissionDescriptor, AndroidProviderDescriptor, AndroidXmlManifestDescriptor,
    };
    use image::{ImageBuffer, Rgba};
    use std::fs::File;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn test_aab_generation_manifest_providers_symbols_and_rfc5652_jar_signing() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let aab_path = temp_dir.path().join("test_app.aab");

        let manifest = AndroidXmlManifestDescriptor {
            package: Some("com.example.testapp".to_string()),
            version_code: Some(42),
            version_name: Some("1.0.0".to_string()),
            uses_permission: vec![AndroidPermissionDescriptor {
                name: Some("android.permission.INTERNET".to_string()),
                max_sdk_version: None,
            }],
            application: AndroidApplicationDescriptor {
                label: Some("TestBundle".to_string()),
                providers: vec![AndroidProviderDescriptor {
                    name: Some("androidx.startup.InitializationProvider".to_string()),
                    authorities: Some("com.example.testapp.androidx-startup".to_string()),
                    exported: Some(false),
                    grant_uri_permissions: None,
                    meta_data: vec![AndroidMetaDataDescriptor {
                        name: Some("androidx.emoji2.text.EmojiCompatInitializer".to_string()),
                        value: Some("androidx.startup".to_string()),
                        resource: None,
                    }],
                }],
                activities: vec![
                    ActivityComponentDescriptor {
                        name: Some("com.example.testapp.MainActivity".to_string()),
                        exported: Some(true),
                        ..Default::default()
                    },
                    ActivityComponentDescriptor {
                        name: Some("com.example.testapp.SettingsActivity".to_string()),
                        exported: Some(false),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let icon_path = temp_dir.path().join("icon.png");
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(512, 512, Rgba([30, 144, 255, 255]));
        img.save(&icon_path)?;

        let mut symbols = ResourceSymbols::default();
        symbols.id_map.insert(
            ("string".to_string(), "custom_greeting".to_string()),
            0x7f050001,
        );
        symbols
            .string_values
            .insert("custom_greeting".to_string(), "Hello AAB".to_string());

        let mut aab = Aab::new(aab_path.clone(), manifest)?;
        aab.add_bundle_config()?;
        aab.add_manifest()?;
        aab.add_resources(Some(&icon_path), Some(&symbols))?;

        assert!(aab.add_asset(Path::new("/"), Path::new("")).is_err());

        let dex_file = temp_dir.path().join("classes.dex");
        std::fs::write(&dex_file, b"dex\n035\0dummy_dex_bytecode")?;
        aab.add_dex(&dex_file, 0)?;

        aab.finish(None)?;

        let file = File::open(&aab_path)?;
        let mut zip = ZipArchive::new(file)?;

        let mut entry_names = Vec::new();
        for i in 0..zip.len() {
            entry_names.push(zip.by_index(i)?.name().to_string());
        }

        assert!(entry_names.contains(&"BundleConfig.pb".to_string()));
        assert!(entry_names.contains(&"base/manifest/AndroidManifest.xml".to_string()));
        assert!(entry_names.contains(&"base/resources.pb".to_string()));
        assert!(entry_names.contains(&"base/res/icon/icon192.png".to_string()));
        assert!(entry_names.contains(&"base/dex/classes.dex".to_string()));

        let mut pb_manifest = Vec::new();
        zip.by_name("base/manifest/AndroidManifest.xml")?
            .read_to_end(&mut pb_manifest)?;
        let pb_manifest_lossy = String::from_utf8_lossy(&pb_manifest);
        assert!(pb_manifest_lossy.contains("com.example.testapp.MainActivity"));
        assert!(pb_manifest_lossy.contains("com.example.testapp.SettingsActivity"));
        assert!(pb_manifest_lossy.contains("androidx.startup.InitializationProvider"));
        assert!(pb_manifest_lossy.contains("androidx.emoji2.text.EmojiCompatInitializer"));

        let mut pb_resources = Vec::new();
        zip.by_name("base/resources.pb")?
            .read_to_end(&mut pb_resources)?;
        let pb_resources_lossy = String::from_utf8_lossy(&pb_resources);
        assert!(pb_resources_lossy.contains("Hello AAB"));
        assert!(pb_resources_lossy.contains("res/icon/icon192.png"));

        let mut cert_sf = Vec::new();
        zip.by_name("META-INF/CERT.SF")?.read_to_end(&mut cert_sf)?;
        let mut cert_rsa = Vec::new();
        zip.by_name("META-INF/CERT.RSA")?
            .read_to_end(&mut cert_rsa)?;
        sign::verify_jar_pkcs7(&cert_rsa, &cert_sf)?;

        Ok(())
    }
}
