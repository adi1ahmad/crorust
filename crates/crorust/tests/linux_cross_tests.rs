use std::fs;
use tempfile::tempdir;

#[test]
fn test_appdir_assembly_with_safe_symlink_fallbacks() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let appimage = crorust_linux::AppImageContainerBuilder::new(
        root,
        "demo_app".to_string(),
        "x86_64".to_string(),
    )
    .unwrap();

    let exec_path = root.join("demo_app");
    fs::write(&exec_path, b"mock_elf_binary").unwrap();
    appimage
        .install_file(&exec_path, std::path::Path::new("demo_app"))
        .unwrap();

    appimage.configure_apprun_entrypoint().unwrap();

    let apprun_path = appimage.bundle_appdir().join("AppRun");
    assert!(apprun_path.exists(), "AppRun must exist inside AppDir");

    appimage.generate_desktop_file().unwrap();
    let desktop_path = appimage.bundle_appdir().join("demo_app.desktop");
    assert!(desktop_path.exists(), "Desktop file must exist");
    let desktop_content = fs::read_to_string(desktop_path).unwrap();
    assert!(desktop_content.contains("Exec=demo_app %u"));
}

#[test]
fn test_end_to_end_pure_rust_appimage_and_squashfs_payload() -> anyhow::Result<()> {
    use anyhow::Context;
    use crorust_linux::{
        AppImageContainerBuilder, AppImageOptions, SquashfsBackend, SquashfsCompression,
    };
    use std::io::Cursor;
    use std::path::Path;

    let temp_dir = tempdir()?;
    let app_dir = temp_dir.path().join("demo_app");
    fs::create_dir_all(&app_dir)?;

    let app = AppImageContainerBuilder::new(&app_dir, "DemoApp".to_string(), "x86_64".to_string())?;
    app.configure_apprun_entrypoint()?;
    app.generate_desktop_file()?;

    let mock_bin = temp_dir.path().join("DemoApp");
    fs::write(
        &mock_bin,
        b"#!/bin/sh\necho 'running inside pure-rust AppImage'",
    )?;
    app.install_file(&mock_bin, Path::new("DemoApp"))?;

    let out_appimage = temp_dir.path().join("DemoApp.AppImage");
    let stub_len = app.runtime_stub()?.len();

    let options = AppImageOptions {
        backend: SquashfsBackend::PureRust,
        compression: SquashfsCompression::Gzip,
    };

    app.build_with_options(&out_appimage, None, &options)?;

    assert!(out_appimage.exists(), "AppImage file must be created");

    let appimage_bytes = fs::read(&out_appimage)?;

    assert_eq!(&appimage_bytes[0..4], &[0x7f, b'E', b'L', b'F']);

    assert!(appimage_bytes.len() > stub_len + 96);
    let squashfs_slice = &appimage_bytes[stub_len..];
    assert_eq!(
        &squashfs_slice[0..4],
        b"hsqs",
        "SquashFS magic 'hsqs' must follow runtime stub"
    );

    let mut cursor = Cursor::new(squashfs_slice);
    let fs = backhand::FilesystemReader::from_reader(&mut cursor)
        .context("Failed to parse generated SquashFS image with backhand reader")?;

    let mut paths: Vec<_> = fs
        .files()
        .map(|f| f.fullpath.to_string_lossy().to_string())
        .collect();
    paths.sort();

    assert!(
        paths.iter().any(|p| p.ends_with("AppRun")),
        "Must contain AppRun"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("DemoApp.desktop")),
        "Must contain desktop file"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("DemoApp")),
        "Must contain DemoApp executable"
    );

    Ok(())
}
