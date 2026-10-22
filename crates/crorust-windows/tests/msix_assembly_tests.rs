use crorust_common::identity_keypair::IdentityKeypair;
use crorust_windows::package::{
    assemble_windows_package, hash_payload_blocks_parallel, PackageIdentityDescriptor,
    WindowsPackageManifestDocument, WindowsPackageSpec, WindowsPayloadEntry,
};
use std::fs::File;
use tempfile::tempdir;
use zip::ZipArchive;

use crorust_common::identity_keypair::DEFAULT_DEBUG_PEM as DEBUG_PEM;

#[test]
fn test_assemble_windows_package_end_to_end() {
    let tmp = tempdir().expect("tempdir");
    let output_msix = tmp.path().join("test_app.msix");

    let manifest = WindowsPackageManifestDocument {
        identity: PackageIdentityDescriptor {
            name: Some("TestCompany.TestApp".into()),
            publisher: Some("CN=Crorust Debug, O=Crorust, C=US".into()),
            version: Some("1.0.0.0".into()),
            processor_architecture: Some("x64".into()),
        },
        ..Default::default()
    };

    let dummy_exe = vec![0x4du8, 0x5a, 0x90, 0x00];
    let dummy_dll = vec![0x4du8, 0x5a, 0x00, 0x00];

    let payload_files = vec![
        WindowsPayloadEntry {
            relative_path: "app.exe".to_string(),
            data: dummy_exe,
        },
        WindowsPayloadEntry {
            relative_path: "support.dll".to_string(),
            data: dummy_dll,
        },
    ];

    let signer = IdentityKeypair::new(DEBUG_PEM).expect("load debug pem");

    let spec = WindowsPackageSpec {
        manifest: &manifest,
        payload_files: &payload_files,
        resource_index: None,
        signing_identity: &signer,
        output_path: &output_msix,
    };

    let descriptor = assemble_windows_package(&spec).expect("assemble windows package");
    assert_eq!(descriptor.path, output_msix);
    assert!(descriptor.file_size_bytes > 0);
    assert!(!descriptor.sha256.is_empty());

    let file = File::open(&output_msix).expect("open msix");
    let mut zip = ZipArchive::new(file).expect("valid zip");

    assert!(
        zip.by_name("[Content_Types].xml").is_ok(),
        "[Content_Types].xml must exist"
    );
    assert!(
        zip.by_name("AppxManifest.xml").is_ok(),
        "AppxManifest.xml must exist"
    );
    assert!(
        zip.by_name("AppxBlockMap.xml").is_ok(),
        "AppxBlockMap.xml must exist"
    );
    assert!(
        zip.by_name("AppxSignature.p7x").is_ok(),
        "AppxSignature.p7x must exist"
    );
    assert!(zip.by_name("app.exe").is_ok(), "app.exe must exist");
    assert!(zip.by_name("support.dll").is_ok(), "support.dll must exist");
}

#[test]
fn test_hash_payload_blocks_parallel_accuracy() {
    let buffer = vec![0x42u8; 200 * 1024];
    let hashes = hash_payload_blocks_parallel(&buffer);
    assert_eq!(hashes.len(), 4);
    assert!(!hashes[0].hash.is_empty());
}

#[test]
fn test_hash_file_blocks_streaming_parity() {
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new().expect("tempfile");
    let data = vec![0xaau8; 500 * 1024];
    temp.write_all(&data).expect("write temp");

    let file = std::fs::File::open(temp.path()).expect("open temp");
    let streamed_blocks = crorust_windows::package::block_map::hash_file_blocks_streaming(file)
        .expect("streaming blocks");
    let direct_blocks = hash_payload_blocks_parallel(&data);

    assert_eq!(streamed_blocks.len(), direct_blocks.len());
    for (s, d) in streamed_blocks.iter().zip(direct_blocks.iter()) {
        assert_eq!(s.hash, d.hash);
        assert_eq!(s.size, d.size);
    }
}
