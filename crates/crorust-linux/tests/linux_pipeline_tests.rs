use crorust_linux::appimage::pipeline::{assemble_linux_package, LinuxPackageSpec};
use crorust_linux::appimage::{AppImageOptions, SquashfsBackend, SquashfsCompression};
use tempfile::tempdir;

#[test]
fn test_assemble_linux_package_end_to_end() {
    let tmp = tempdir().expect("tempdir");
    let dummy_bin = tmp.path().join("my_linux_bin");
    std::fs::write(
        &dummy_bin,
        b"#!/bin/sh\necho 'hello from decoupled linux pipeline'",
    )
    .expect("write bin");

    let dummy_icon = tmp.path().join("icon.png");
    std::fs::write(&dummy_icon, b"\x89PNG\r\n\x1a\ndummy_icon_payload").expect("write icon");

    let output_appimage = tmp.path().join("MyPureLinuxApp.AppImage");

    let spec = LinuxPackageSpec {
        application_name: "MyPureLinuxApp",
        target_arch: "x86_64",
        binary_path: &dummy_bin,
        icon_path: Some(&dummy_icon),
        shared_libraries: &[],
        desktop_file_content: None,
        output_path: &output_appimage,
        options: Some(AppImageOptions {
            backend: SquashfsBackend::PureRust,
            compression: SquashfsCompression::Gzip,
        }),
    };

    let descriptor = assemble_linux_package(&spec).expect("assemble linux package");
    assert_eq!(descriptor.path, output_appimage);
    assert!(descriptor.file_size_bytes > 0);
    assert!(!descriptor.sha256.is_empty());

    let bytes = std::fs::read(&output_appimage).expect("read appimage");
    assert_eq!(&bytes[0..4], &[0x7f, b'E', b'L', b'F']);
}
