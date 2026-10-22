use anyhow::{ensure, Context, Result};
use crorust_android::protocol::AndroidChunk;
use crorust_android::AndroidPackageContainer;
use std::collections::HashSet;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Default)]
pub struct FullApkAudit {
    pub package_name: String,
    pub main_activity: String,
    pub permissions: HashSet<String>,
    pub providers: HashSet<String>,
    pub native_libraries: HashSet<String>,
    pub so_alignments: Vec<(String, u64, zip::CompressionMethod)>,
    pub dex_classes: HashSet<String>,
    pub dex_alignments: Vec<(String, u64)>,
    pub asset_files: HashSet<String>,
    pub arsc_strings: HashSet<String>,
    pub has_theme_appcompat: bool,
    pub is_signature_valid: bool,
}

impl FullApkAudit {
    pub fn inspect(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("Failed to open APK: {:?}", path))?;
        let mut zip = ZipArchive::new(file)?;
        let mut audit = Self::default();

        if let Ok(entry_point) = AndroidPackageContainer::query_entry_point(path) {
            audit.package_name = entry_point.package;
            audit.main_activity = entry_point.activity;
        }

        if let Ok(manifest_bytes) =
            crorust_common::unpack_single_entry_secure(path, "AndroidManifest.xml")
        {
            audit.audit_manifest(&manifest_bytes)?;
        }

        for i in 0..zip.len() {
            let entry = zip.by_index(i)?;
            let name = entry.name().to_string();
            let compression = entry.compression();
            let offset = entry.data_start();

            if name.starts_with("lib/") && name.ends_with(".so") {
                audit.native_libraries.insert(name.clone());
                audit.so_alignments.push((name, offset, compression));
            } else if name.starts_with("classes") && name.ends_with(".dex") {
                audit.dex_alignments.push((name.clone(), offset));
            } else if name.starts_with("assets/") && !entry.is_dir() {
                audit.asset_files.insert(name);
            }
        }

        for (dex_name, _) in &audit.dex_alignments {
            if let Ok(dex_bytes) = crorust_common::unpack_single_entry_secure(path, dex_name) {
                let classes = DexParser::extract_classes(&dex_bytes)
                    .with_context(|| format!("Failed parsing DEX in {}", dex_name))?;
                audit.dex_classes.extend(classes);
            }
        }

        if let Ok(arsc_bytes) = crorust_common::unpack_single_entry_secure(path, "resources.arsc") {
            let _ = audit.audit_arsc(&arsc_bytes);
        }

        audit.is_signature_valid = AndroidPackageContainer::verify_apk_target(path).is_ok();

        Ok(audit)
    }

    fn audit_manifest(&mut self, bytes: &[u8]) -> Result<()> {
        let chunk = AndroidChunk::parse(&mut Cursor::new(bytes))
            .context("Failed parsing AndroidManifest.xml as AXML chunk")?;

        if let AndroidChunk::Xml(chunks) = chunk {
            if let Some(AndroidChunk::StringPool(strings, _)) = chunks.first() {
                for s in strings {
                    if s.starts_with("android.permission.") {
                        self.permissions.insert(s.clone());
                    }
                    if self.main_activity.is_empty()
                        && (s.ends_with("Activity") || s.contains("MainActivity"))
                    {
                        self.main_activity = s.clone();
                    }
                    if s.contains("Provider") || s.contains("InitializationProvider") {
                        self.providers.insert(s.clone());
                    }
                }
            }
        }
        Ok(())
    }

    fn audit_arsc(&mut self, bytes: &[u8]) -> Result<()> {
        let chunk = AndroidChunk::parse(&mut Cursor::new(bytes))
            .context("Failed parsing resources.arsc chunk")?;
        if let AndroidChunk::Table(_, chunks) = chunk {
            if let Some(AndroidChunk::StringPool(strings, _)) = chunks.first() {
                for s in strings {
                    self.arsc_strings.insert(s.clone());
                    if s.contains("Theme.AppCompat") {
                        self.has_theme_appcompat = true;
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct DexParser;

impl DexParser {
    pub fn extract_classes(bytes: &[u8]) -> Result<HashSet<String>> {
        ensure!(
            bytes.len() >= 112,
            "DEX file smaller than standard header (112 bytes)"
        );

        let mut r = Cursor::new(bytes);
        let mut magic = [0u8; 8];
        r.read_exact(&mut magic)?;

        ensure!(
            &magic[0..4] == b"dex\n",
            "Invalid DEX magic: not a Dalvik Executable"
        );

        r.seek(SeekFrom::Start(56))?;
        let string_ids_size = read_u32_le(&mut r)? as usize;
        let string_ids_off = read_u32_le(&mut r)? as u64;

        let type_ids_size = read_u32_le(&mut r)? as usize;
        let type_ids_off = read_u32_le(&mut r)? as u64;

        let mut string_offsets = Vec::with_capacity(string_ids_size);
        r.seek(SeekFrom::Start(string_ids_off))?;
        for _ in 0..string_ids_size {
            string_offsets.push(read_u32_le(&mut r)? as u64);
        }

        let mut type_string_indices = Vec::with_capacity(type_ids_size);
        r.seek(SeekFrom::Start(type_ids_off))?;
        for _ in 0..type_ids_size {
            type_string_indices.push(read_u32_le(&mut r)? as usize);
        }

        let mut classes = HashSet::new();
        for &str_idx in &type_string_indices {
            if str_idx < string_offsets.len() {
                let off = string_offsets[str_idx];
                if (off as usize) < bytes.len() {
                    r.seek(SeekFrom::Start(off))?;

                    let _ = read_uleb128(&mut r)?;

                    let mut string_bytes = Vec::new();
                    let mut b = read_u8(&mut r)?;
                    while b != 0 {
                        string_bytes.push(b);
                        b = read_u8(&mut r)?;
                    }
                    if let Ok(s) = String::from_utf8(string_bytes) {
                        if s.starts_with('L') && s.ends_with(';') {
                            classes.insert(s);
                        }
                    }
                }
            }
        }
        Ok(classes)
    }
}

fn read_u8<R: Read>(r: &mut R) -> Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

fn read_u32_le<R: Read>(r: &mut R) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_uleb128<R: Read>(r: &mut R) -> Result<u32> {
    let mut result = 0u32;
    let mut shift = 0;
    loop {
        let byte = read_u8(r)?;
        result |= ((byte & 0x7f) as u32) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
        ensure!(shift < 35, "ULEB128 overflow");
    }
    Ok(result)
}

fn synthesize_minimal_dex(classes: &[&str]) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; 112];
    buf[0..8].copy_from_slice(b"dex\n035\0");

    let string_ids_size = classes.len() as u32;
    let string_ids_off = 112u32;
    let type_ids_size = classes.len() as u32;
    let type_ids_off = string_ids_off + (string_ids_size * 4);

    let mut string_data_pool = Vec::new();
    let mut string_offsets = Vec::new();

    let string_data_start = type_ids_off + (type_ids_size * 4);

    for class in classes {
        let current_offset = string_data_start + string_data_pool.len() as u32;
        string_offsets.push(current_offset);

        string_data_pool.push(class.len() as u8);
        string_data_pool.extend_from_slice(class.as_bytes());
        string_data_pool.push(0);
    }

    let total_size = string_data_start + string_data_pool.len() as u32;
    buf[32..36].copy_from_slice(&total_size.to_le_bytes());
    buf[36..40].copy_from_slice(&112u32.to_le_bytes());
    buf[40..44].copy_from_slice(&0x12345678u32.to_le_bytes());

    buf[56..60].copy_from_slice(&string_ids_size.to_le_bytes());
    buf[60..64].copy_from_slice(&string_ids_off.to_le_bytes());
    buf[64..68].copy_from_slice(&type_ids_size.to_le_bytes());
    buf[68..72].copy_from_slice(&type_ids_off.to_le_bytes());

    for off in string_offsets {
        buf.extend_from_slice(&off.to_le_bytes());
    }

    for i in 0..type_ids_size {
        buf.extend_from_slice(&i.to_le_bytes());
    }

    buf.extend_from_slice(&string_data_pool);

    Ok(buf)
}

#[test]
fn test_dex_parser_structural_inspection() -> Result<()> {
    let test_classes = [
        "Lcrorust/ruxt/MainActivity;",
        "Lcrorust/ruxt/RustBridge;",
        "Landroidx/games/activity/GameActivity;",
        "Landroidx/core/app/ActivityCompat;",
    ];

    let dex_bytes = synthesize_minimal_dex(&test_classes)?;
    let parsed_classes = DexParser::extract_classes(&dex_bytes)?;

    for expected in &test_classes {
        assert!(
            parsed_classes.contains(*expected),
            "DexParser failed to extract class descriptor: {}",
            expected
        );
    }
    assert_eq!(parsed_classes.len(), test_classes.len());
    Ok(())
}

#[test]
fn test_apk_zip_entry_alignment_invariants() -> Result<()> {
    use crorust_android::{AndroidAbi, AndroidXmlManifestDescriptor};

    let dir = tempfile::tempdir()?;
    let apk_path = dir.path().join("aligned.apk");
    let dex_path = dir.path().join("classes.dex");
    let so_arm64 = dir.path().join("libmain.so");
    let so_x86_64 = dir.path().join("libextra.so");

    std::fs::write(&dex_path, b"dex\n035\0alignment_test_dex_bytes")?;
    std::fs::write(
        &so_arm64,
        b"\x7fELF\x02\x01\x01\0alignment_test_arm64_bytes",
    )?;
    std::fs::write(
        &so_x86_64,
        b"\x7fELF\x02\x01\x01\0alignment_test_x86_64_bytes",
    )?;

    let manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.align".to_string()),
        ..Default::default()
    };
    let mut apk = AndroidPackageContainer::new(apk_path.clone(), manifest, true)?;
    apk.bundle_dex(&dex_path)?;
    apk.bundle_shared_library(AndroidAbi::Arm64V8a, &so_arm64)?;
    apk.bundle_shared_library(AndroidAbi::X86_64, &so_x86_64)?;
    apk.finalize_apk(None)?;

    let mut archive = ZipArchive::new(File::open(&apk_path)?)?;
    let mut found_so = 0;
    let mut found_dex = 0;

    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        let offset = entry.data_start();
        let compression = entry.compression();

        if name.starts_with("lib/") && name.ends_with(".so") {
            found_so += 1;
            assert_eq!(
                compression,
                zip::CompressionMethod::Stored,
                "Native shared library `{name}` must be stored UNCOMPRESSED for zero-copy mmap"
            );
            assert_eq!(
                offset % 16384,
                0,
                "Native library `{name}` at offset {offset} violates 16KB page alignment"
            );
        } else if name.starts_with("classes") && name.ends_with(".dex") {
            found_dex += 1;
            assert_eq!(
                offset % 4,
                0,
                "DEX file `{name}` at offset {offset} violates 4-byte Dalvik alignment"
            );
        }
    }

    assert_eq!(found_so, 2, "Expected 2 shared libraries");
    assert_eq!(found_dex, 1, "Expected 1 DEX file");
    Ok(())
}

#[test]
#[ignore = "Requires Android SDK and built template artifacts"]
fn test_template_apk_parity_and_drift() -> Result<()> {
    let template_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("templates/crorust.ruxt");

    let standalone_candidates = [
        template_dir.join(".cache/target/crorust/android/standalone_app.apk"),
        template_dir.join("target/crorust/android/standalone_app.apk"),
    ];
    let standalone_apk = standalone_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| template_dir.join(".cache/target/crorust/android/standalone_app.apk"));

    let gradle_candidates = [
        template_dir.join(".cache/target/crorust/android/gradle_app.apk"),
        template_dir.join("target/crorust/android/gradle_app.apk"),
    ];
    let gradle_apk = gradle_candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| template_dir.join(".cache/target/crorust/android/gradle_app.apk"));

    if !standalone_apk.exists() || !gradle_apk.exists() {
        if std::env::var("CRORUST_REQUIRE_PARITY").is_ok() {
            anyhow::bail!(
                "CRORUST_REQUIRE_PARITY set, but artifacts are missing: standalone_apk ({}) exists: {}, gradle_apk ({}) exists: {}",
                standalone_apk.display(),
                standalone_apk.exists(),
                gradle_apk.display(),
                gradle_apk.exists()
            );
        }
        println!("Skipping parity test: standalone_app.apk or gradle_app.apk not found");
        return Ok(());
    }

    let audit_standalone = FullApkAudit::inspect(&standalone_apk)?;
    let audit_gradle = FullApkAudit::inspect(&gradle_apk)?;

    assert_eq!(
        audit_standalone.package_name, audit_gradle.package_name,
        "Package name mismatch between standalone and gradle builds"
    );
    assert_eq!(
        audit_standalone.main_activity, audit_gradle.main_activity,
        "Launcher activity mismatch between standalone and gradle builds"
    );
    assert_eq!(
        audit_standalone.permissions, audit_gradle.permissions,
        "Permission set divergence between standalone and gradle builds"
    );

    assert!(
        audit_standalone
            .native_libraries
            .contains("lib/arm64-v8a/libruxt_app.so"),
        "Standalone APK is missing compiled Rust shared library libruxt_app.so"
    );
    assert!(
        audit_standalone
            .native_libraries
            .contains("lib/arm64-v8a/libgame_activity.so"),
        "Standalone APK is missing AAR companion library libgame_activity.so"
    );

    for (lib_name, offset, compression) in &audit_standalone.so_alignments {
        assert_eq!(
            *compression,
            zip::CompressionMethod::Stored,
            "Native library `{}` must be uncompressed (CompressionMethod::Stored) for zero-copy mmap",
            lib_name
        );
        assert_eq!(
            offset % 16384,
            0,
            "Native library `{}` in standalone APK has invalid alignment offset: {} (must be multiple of 16384)",
            lib_name,
            offset
        );
    }

    for (dex_name, offset) in &audit_standalone.dex_alignments {
        assert_eq!(
            offset % 4,
            0,
            "DEX entry `{}` has invalid alignment offset: {} (must be multiple of 4)",
            dex_name,
            offset
        );
    }

    for required_class in [
        "Lcrorust/ruxt/MainActivity;",
        "Lcrorust/ruxt/RustBridge;",
        "Landroidx/games/activity/GameActivity;",
    ] {
        assert!(
            audit_standalone.dex_classes.contains(required_class),
            "Standalone APK DEX bytecode missing essential class: {}",
            required_class
        );
    }

    assert!(
        audit_standalone.is_signature_valid,
        "Standalone APK failed APK Signature Scheme v2 cryptographic verification"
    );

    println!("✅ Parity verification passed with 0 functional drift.");
    Ok(())
}
