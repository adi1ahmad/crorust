use std::fs;
use tempfile::tempdir;

#[test]
fn test_safe_symlink_or_copy_basic_file() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("source.txt");
    let dst = dir.path().join("sub").join("dest.txt");

    fs::write(&src, b"hello world").unwrap();

    crorust_common::safe_symlink_or_copy(&src, &dst).unwrap();
    assert!(dst.exists());
    assert_eq!(fs::read(&dst).unwrap(), b"hello world");
}

#[test]
fn test_safe_symlink_or_copy_overwrites_file_dir_and_dangling_symlink() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("source.txt");
    let dst = dir.path().join("dest.txt");

    fs::write(&src, b"new content").unwrap();

    fs::write(&dst, b"old content").unwrap();
    crorust_common::safe_symlink_or_copy(&src, &dst).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"new content");

    fs::remove_file(&dst).unwrap();
    fs::create_dir_all(dst.join("nested")).unwrap();
    crorust_common::safe_symlink_or_copy(&src, &dst).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"new content");

    #[cfg(unix)]
    {
        fs::remove_file(&dst).unwrap();
        std::os::unix::fs::symlink(dir.path().join("nonexistent_target"), &dst).unwrap();
        assert!(dst.is_symlink() && !dst.exists());
        crorust_common::safe_symlink_or_copy(&src, &dst).unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"new content");
    }
}

#[test]
fn test_safe_symlink_or_copy_recursive_directory() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src_dir");
    let nested = src_dir.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("file.txt"), b"nested data").unwrap();

    let dst_dir = dir.path().join("dst_dir");
    crorust_common::safe_symlink_or_copy(&src_dir, &dst_dir).unwrap();

    let copied_file = dst_dir.join("nested").join("file.txt");
    assert!(copied_file.exists());
    assert_eq!(fs::read(&copied_file).unwrap(), b"nested data");
}

#[cfg(unix)]
#[test]
fn test_copy_dir_all_and_symlink_preserve_executable_permission() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src_bin");
    fs::create_dir_all(&src_dir).unwrap();
    let src_script = src_dir.join("run.sh");
    fs::write(&src_script, b"#!/bin/sh\necho hi\n").unwrap();
    fs::set_permissions(&src_script, fs::Permissions::from_mode(0o755)).unwrap();

    let dst_dir = dir.path().join("dst_bin");
    fs::create_dir_all(&dst_dir).unwrap();
    crorust_common::copy_dir_all(&src_dir, &dst_dir).unwrap();

    let copied_script = dst_dir.join("run.sh");
    assert!(!copied_script.is_symlink());
    let mode = fs::metadata(&copied_script).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "Copied script must retain executable bit");
}
