use anyhow::Result;
use crorust_common::identity_keypair::{IdentityKeypair, DEFAULT_DEBUG_PEM};
use crorust_common::ArchiveEntryOptions;
use crorust_windows::mrm::{MrmPriPackage, MrmPriPackageBuilder};
use crorust_windows::package::{
    ManifestApplicationEntry, PackageIdentityDescriptor, WindowsPackageContainer,
    WindowsPackageManifestDocument,
};
use crorust_windows::security::parse_authenticode_signature;
use image::{ImageBuffer, Rgba};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use tempfile::tempdir;
use zip::ZipArchive;

#[test]
fn test_mrm_pri_archive_roundtrip_serialization() -> Result<()> {
    let mut builder = MrmPriPackageBuilder::new("TestCompany.UniqueApp");
    builder.add_asset_scale_variants(
        "Assets",
        "logo.png",
        &[
            (100, "Assets\\logo.scale-100.png".to_string()),
            (200, "Assets\\logo.scale-200.png".to_string()),
            (400, "Assets\\logo.scale-400.png".to_string()),
        ],
    );
    builder.add_asset_scale_variants(
        "Images",
        "splash.png",
        &[(100, "Images\\splash.scale-100.png".to_string())],
    );

    let pri = builder.finish()?;
    assert_eq!(pri.package_identity, "TestCompany.UniqueApp");
    assert_eq!(pri.resources.len(), 2);

    let mut binary_data = Vec::new();
    pri.write(&mut binary_data)?;
    assert!(binary_data.len() > 32);
    assert_eq!(&binary_data[..8], b"mrm_pri2");
    assert_eq!(&binary_data[binary_data.len() - 8..], b"mrm_pri2");

    let mut cursor = Cursor::new(&binary_data);
    let parsed = MrmPriPackage::read(&mut cursor)?;
    assert_eq!(parsed.package_identity, "TestCompany.UniqueApp");
    assert_eq!(parsed.resources.len(), 2);

    let logo_entry = parsed
        .resources
        .get("Files/Assets/logo.png")
        .expect("Assets/logo.png resource entry");
    assert_eq!(logo_entry.len(), 3);
    assert_eq!(logo_entry[0].qualifier_scale, 100);
    assert_eq!(
        logo_entry[0].target_relative_path,
        "Assets\\logo.scale-100.png"
    );
    assert_eq!(logo_entry[1].qualifier_scale, 200);
    assert_eq!(logo_entry[2].qualifier_scale, 400);

    Ok(())
}

#[test]
fn test_msix_container_with_mrm_and_authenticode() -> Result<()> {
    let dir = tempdir()?;
    let msix_path = dir.path().join("cleanroom_app.msix");
    let icon_path = dir.path().join("icon.png");
    let exe_path = dir.path().join("payload.exe");

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(512, 512, Rgba([30, 144, 255, 255]));
    img.save(&icon_path)?;
    std::fs::write(&exe_path, b"MZ\x90\x00_FRESH_CLEANROOM_PE_PAYLOAD_")?;

    let mut manifest = WindowsPackageManifestDocument {
        identity: PackageIdentityDescriptor {
            name: Some("CleanRoom.WindowsApp".into()),
            publisher: Some("CN=CroRust Authenticode, O=CroRust, C=US".into()),
            version: Some("2.0.0.0".into()),
            processor_architecture: Some("x64".into()),
        },
        ..Default::default()
    };
    manifest.properties.display_name = Some("CleanRoomApp".into());
    manifest.properties.publisher_display_name = Some("CroRust Team".into());
    manifest
        .applications
        .application
        .push(ManifestApplicationEntry {
            id: Some("AppMain".into()),
            executable: Some("payload.exe".into()),
            ..Default::default()
        });

    let mut msix = WindowsPackageContainer::new(msix_path.clone(), manifest, true)?;
    msix.embed_application_icon(&icon_path)?;
    msix.embed_file(
        &exe_path,
        Path::new("payload.exe"),
        ArchiveEntryOptions::Compressed,
    )?;

    let signer = IdentityKeypair::new(DEFAULT_DEBUG_PEM)?;
    msix.finalize_package(Some(signer))?;

    let file = File::open(&msix_path)?;
    let mut zip = ZipArchive::new(file)?;

    assert!(zip.by_name("[Content_Types].xml").is_ok());
    assert!(zip.by_name("AppxManifest.xml").is_ok());
    assert!(zip.by_name("AppxBlockMap.xml").is_ok());
    assert!(zip.by_name("resources.pri").is_ok());
    assert!(zip.by_name("payload.exe").is_ok());

    let mut sig_entry = zip.by_name("AppxSignature.p7x")?;
    let mut p7x_bytes = Vec::new();
    sig_entry.read_to_end(&mut p7x_bytes)?;

    let digests = parse_authenticode_signature(&p7x_bytes)?;
    assert_ne!(digests.axpc, [0u8; 32]);
    assert_ne!(digests.axbm, [0u8; 32]);
    assert_ne!(digests.axct, [0u8; 32]);
    assert_ne!(digests.axcd, [0u8; 32]);

    Ok(())
}
