use crorust_apple::bundle::pipeline::{assemble_darwin_package, DarwinPackageSpec};
use crorust_apple::DarwinPropertyList;
use image::{ImageBuffer, Rgba};
use tempfile::tempdir;

#[test]
fn test_assemble_darwin_package_macos_end_to_end() {
    let tmp = tempdir().expect("tempdir");
    let dummy_exe = tmp.path().join("my_app_bin");
    std::fs::write(&dummy_exe, b"Mach-O 64-bit mock executable").expect("write exe");

    let icon_path = tmp.path().join("app_icon.png");
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(512, 512, Rgba([100, 150, 200, 255]));
    img.save(&icon_path).expect("save icon");

    let info = DarwinPropertyList {
        bundle_name: Some("PureDarwinApp".to_string()),
        bundle_identifier: Some("com.example.puredarwin".to_string()),
        bundle_version: Some("1.0.0".to_string()),
        ..Default::default()
    };

    let spec = DarwinPackageSpec {
        bundle_name: "PureDarwinApp",
        executable_binary: &dummy_exe,
        metadata: info,
        icon_path: Some(&icon_path),
        frameworks: &[],
        provisioning_profile: None,
        output_dir: tmp.path(),
        signing_identity: None,
    };

    let descriptor = assemble_darwin_package(&spec).expect("assemble darwin package");
    assert_eq!(descriptor.path, tmp.path().join("PureDarwinApp.app"));
    assert!(descriptor.file_size_bytes > 0);
    assert!(!descriptor.sha256.is_empty());

    assert!(descriptor.path.join("Contents/MacOS/my_app_bin").is_file());
    assert!(descriptor.path.join("Contents/Info.plist").is_file());
    assert!(descriptor
        .path
        .join("Contents/Resources/AppIcon.icns")
        .is_file());
}
