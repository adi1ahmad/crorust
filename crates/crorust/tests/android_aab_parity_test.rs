use anyhow::{Context, Result};
use crorust_android::{Aab, AndroidXmlManifestDescriptor};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Default)]
pub struct AabAudit {
    pub entries: HashSet<String>,
    pub uncompressed_entries: HashSet<String>,
    pub has_bundle_config: bool,
    pub has_manifest_proto: bool,
    pub has_resources_proto: bool,
    pub has_dex: bool,
    pub has_jar_signature: bool,
}

impl AabAudit {
    pub fn inspect(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Failed opening AAB: {:?}", path))?;
        let mut zip = ZipArchive::new(file)?;
        let mut audit = Self::default();

        for i in 0..zip.len() {
            let entry = zip.by_index(i)?;
            let name = entry.name().to_string();
            let compression = entry.compression();

            if compression == zip::CompressionMethod::Stored {
                audit.uncompressed_entries.insert(name.clone());
            }

            audit.entries.insert(name);
        }

        audit.has_bundle_config = audit.entries.contains("BundleConfig.pb");
        audit.has_manifest_proto = audit.entries.contains("base/manifest/AndroidManifest.xml");
        audit.has_resources_proto = audit.entries.contains("base/resources.pb");
        audit.has_dex = audit.entries.contains("base/dex/classes.dex");
        audit.has_jar_signature = audit.entries.contains("META-INF/MANIFEST.MF")
            && audit.entries.contains("META-INF/CERT.SF")
            && audit.entries.contains("META-INF/CERT.RSA");

        Ok(audit)
    }
}

#[test]
fn test_pure_rust_aab_structural_integrity_and_signing() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let aab_out = temp_dir.path().join("app.aab");

    let manifest = AndroidXmlManifestDescriptor {
        package: Some("crorust.ruxt".to_string()),
        version_code: Some(1),
        version_name: Some("0.1.0".to_string()),
        ..Default::default()
    };

    let mut aab = Aab::new(aab_out.clone(), manifest)?;
    aab.add_bundle_config()?;
    aab.add_manifest()?;
    aab.add_resources(None, None)?;

    let dex_path = temp_dir.path().join("classes.dex");
    std::fs::write(&dex_path, b"dex\n035\0sample_bytecode_payload")?;
    aab.add_dex(&dex_path, 0)?;

    let lib_path = temp_dir.path().join("libruxt_app.so");
    std::fs::write(&lib_path, b"\x7fELF\x02\x01\x01\0sample_elf_binary")?;
    aab.add_lib("arm64-v8a", &lib_path)?;

    aab.finish(None)?;

    let audit = AabAudit::inspect(&aab_out)?;

    assert!(
        audit.has_bundle_config,
        "AAB missing root BundleConfig.pb specification"
    );
    assert!(
        audit.has_manifest_proto,
        "AAB missing base/manifest/AndroidManifest.xml in protobuf format"
    );
    assert!(
        audit.has_resources_proto,
        "AAB missing base/resources.pb in protobuf format"
    );
    assert!(
        audit.has_dex,
        "AAB missing base/dex/classes.dex Dalvik bytecode"
    );
    assert!(
        audit.entries.contains("base/lib/arm64-v8a/libruxt_app.so"),
        "AAB missing native library in base/lib/arm64-v8a/"
    );

    assert!(
        audit.uncompressed_entries.contains("base/lib/arm64-v8a/libruxt_app.so"),
        "Native shared libraries in AAB must be uncompressed (CompressionMethod::Stored) for Google Play dynamic splitting"
    );

    assert!(
        audit.has_jar_signature,
        "AAB missing standard JAR signature (MANIFEST.MF, CERT.SF, CERT.RSA)"
    );

    let file = File::open(&aab_out)?;
    let mut zip = ZipArchive::new(file)?;
    {
        let mut manifest_entry = zip.by_name("META-INF/MANIFEST.MF")?;
        let mut manifest_str = String::new();
        manifest_entry.read_to_string(&mut manifest_str)?;

        assert!(manifest_str.contains("Name: BundleConfig.pb\r\nSHA-256-Digest: "));
        assert!(
            manifest_str.contains("Name: base/manifest/AndroidManifest.xml\r\nSHA-256-Digest: ")
        );
        assert!(
            manifest_str.contains("Name: base/lib/arm64-v8a/libruxt_app.so\r\nSHA-256-Digest: ")
        );
    }

    let mut sf_bytes = Vec::new();
    zip.by_name("META-INF/CERT.SF")?
        .read_to_end(&mut sf_bytes)?;
    let mut rsa_bytes = Vec::new();
    zip.by_name("META-INF/CERT.RSA")?
        .read_to_end(&mut rsa_bytes)?;
    crorust_android::aab::sign::verify_jar_pkcs7(&rsa_bytes, &sf_bytes)?;

    println!("✅ Pure-Rust AAB packaging and signing passed structural parity and cryptographic verification.");
    Ok(())
}
