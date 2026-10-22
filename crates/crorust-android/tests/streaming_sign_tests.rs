use anyhow::Result;
use crorust_android::AndroidPackageContainer;
use crorust_common::{ArchiveEntryOptions, BoundaryZipArchive, IdentityKeypair};
use tempfile::TempDir;

use crorust_common::identity_keypair::DEFAULT_DEBUG_PEM as PEM;

#[test]
fn test_streaming_apk_v2_signing_and_verification() -> Result<()> {
    let temp = TempDir::new()?;
    let apk_path = temp.path().join("streaming_test.apk");

    {
        let mut zip = BoundaryZipArchive::new(&apk_path, true)?;
        zip.create_file(
            std::path::Path::new("AndroidManifest.xml"),
            ArchiveEntryOptions::Compressed,
            b"<manifest></manifest>",
        )?;

        let large_payload = vec![0x42u8; 256 * 1024];
        zip.create_file(
            std::path::Path::new("lib/arm64-v8a/libmain.so"),
            ArchiveEntryOptions::Aligned(4096),
            &large_payload,
        )?;
        zip.finish()?;
    }

    let signer = IdentityKeypair::new(PEM)?;
    AndroidPackageContainer::sign_apk_target(&apk_path, Some(signer))?;

    let certs = AndroidPackageContainer::verify_apk_target(&apk_path)
        .expect("Streaming APK v2 signature verification must succeed");
    assert!(!certs.is_empty(), "Verified APK must contain certificate");

    Ok(())
}

#[test]
fn test_streaming_apk_digest_multi_chunk_boundaries() -> Result<()> {
    use std::io::Cursor;

    // Synthetic APK payload with 18 MB of data spanning across multiple 1MB chunk boundaries
    let mut buffer = Cursor::new(vec![0x7au8; 18 * 1024 * 1024]);
    let sb_start = 14 * 1024 * 1024;
    let cd_start = 16 * 1024 * 1024;
    let eocd_start = 17 * 1024 * 1024;

    let root_digest = crorust_android::package::authenticity::compute_apk_digest_parallel(
        &mut buffer,
        sb_start,
        cd_start,
        eocd_start,
    )?;

    assert_ne!(root_digest, [0u8; 32], "Root digest must be non-zero");
    Ok(())
}

#[test]
fn test_sign_apk_overwrites_existing_file() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let apk_path = tmp.path().join("test_app.apk");

    {
        let mut zip = BoundaryZipArchive::new(&apk_path, false)?;
        zip.create_file(
            std::path::Path::new("AndroidManifest.xml"),
            ArchiveEntryOptions::Compressed,
            b"<manifest></manifest>",
        )?;
        zip.finish()?;
    }

    let signer1 = IdentityKeypair::new(PEM)?;
    AndroidPackageContainer::sign_apk_target(&apk_path, Some(signer1))?;

    let signer2 = IdentityKeypair::new(PEM)?;
    AndroidPackageContainer::sign_apk_target(&apk_path, Some(signer2))?;

    assert!(apk_path.exists());
    let certs = AndroidPackageContainer::verify_apk_target(&apk_path)?;
    assert!(!certs.is_empty());
    Ok(())
}
