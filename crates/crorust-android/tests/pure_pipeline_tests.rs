use crorust_android::manifest::manifest_dom::{
    ActivityComponentDescriptor, AndroidApplicationDescriptor, AndroidXmlManifestDescriptor,
};
use crorust_android::package::authenticity::{sign_apk_file, verify_apk_signature};
use crorust_android::package::boundary_align::verify_apk_boundary_alignments;
use crorust_android::package::container::{
    assemble_android_package, AndroidPackageSpec, NativeLibraryArtifact,
};
use crorust_common::identity_keypair::IdentityKeypair;
use tempfile::tempdir;

use crorust_common::identity_keypair::DEFAULT_DEBUG_PEM as DEBUG_PEM;

#[test]
fn test_pure_function_assemble_android_package_end_to_end() {
    let tmp = tempdir().expect("tempdir");
    let output_apk = tmp.path().join("assembled_app.apk");

    let manifest = AndroidXmlManifestDescriptor {
        package: Some("com.example.puretest".into()),
        application: AndroidApplicationDescriptor {
            label: Some("PureTestApp".into()),
            activities: vec![ActivityComponentDescriptor {
                name: Some(".MainActivity".into()),
                ..Default::default()
            }],
            ..Default::default()
        },
        ..Default::default()
    };

    let bytecode_dex = b"dex\n035\0dummy_bytecode_payload_for_testing";
    let native_so = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0];

    let native_libs = vec![NativeLibraryArtifact {
        abi: "arm64-v8a".to_string(),
        filename: "libtest.so".to_string(),
        data: native_so,
    }];

    let signer = IdentityKeypair::new(DEBUG_PEM).expect("load debug pem");

    let spec = AndroidPackageSpec {
        manifest: &manifest,
        bytecode_dex,
        native_libraries: &native_libs,
        assets_directory: None,
        resource_catalog: None,
        signing_identity: &signer,
        output_path: &output_apk,
    };

    let descriptor = assemble_android_package(&spec).expect("assemble android package");
    assert_eq!(descriptor.path, output_apk);
    assert!(descriptor.file_size_bytes > 0);
    assert!(!descriptor.sha256.is_empty());

    let aligned = verify_apk_boundary_alignments(&output_apk).expect("verify boundary alignments");
    assert!(
        aligned,
        "APK must satisfy 16KB and 4-byte alignment boundaries"
    );

    let certs = verify_apk_signature(&output_apk).expect("verify apk signature");
    assert!(
        !certs.is_empty(),
        "APK must have at least one valid signing certificate"
    );
}

#[test]
fn test_rayon_parallel_merkle_hashing_consistency() {
    let tmp = tempdir().expect("tempdir");
    let test_apk = tmp.path().join("merkle_test.apk");

    let mut zip = crorust_common::BoundaryZipArchive::new(&test_apk, true).expect("create zip");
    zip.create_file(
        std::path::Path::new("dummy.txt"),
        crorust_common::ArchiveEntryOptions::Compressed,
        b"Merkle tree parallel hashing consistency payload",
    )
    .expect("create file");
    zip.finish().expect("finish zip");

    let signer = IdentityKeypair::new(DEBUG_PEM).expect("load debug pem");
    sign_apk_file(&test_apk, Some(&signer)).expect("sign with parallel hasher");

    let certs = verify_apk_signature(&test_apk).expect("verify signature");
    assert_eq!(certs.len(), 1);
}
