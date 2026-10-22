use crorust_common::fs::case_fold_lookup;
use crorust_common::path::sanitize_path;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_case_folding_resolution_on_disk() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let include_dir = temp.path().join("Include").join("Um");
    std::fs::create_dir_all(&include_dir)?;
    let header_file = include_dir.join("Windows.h");
    std::fs::write(&header_file, b"// Windows SDK Header")?;

    let resolved = case_fold_lookup(temp.path(), Path::new("include/um/windows.h"));
    assert!(
        resolved.is_some(),
        "case_fold_lookup failed to resolve mismatched casing"
    );
    assert_eq!(resolved.unwrap(), header_file);

    let resolved_upper = case_fold_lookup(temp.path(), Path::new("INCLUDE/UM/WINDOWS.H"));
    assert!(resolved_upper.is_some());
    assert_eq!(resolved_upper.unwrap(), header_file);

    let resolved_backslash = case_fold_lookup(temp.path(), Path::new(r"include\um\windows.h"));
    assert!(resolved_backslash.is_some());
    assert_eq!(resolved_backslash.unwrap(), header_file);

    let non_existent = case_fold_lookup(temp.path(), Path::new("include/um/missing.h"));
    assert!(non_existent.is_none());

    Ok(())
}

#[test]
fn test_sanitize_path_unc_and_verbatim() {
    assert_eq!(
        sanitize_path(Path::new(r"\\?\C:\Users\User\app")),
        "C:/Users/User/app"
    );
    assert_eq!(
        sanitize_path(Path::new(r"\\?\UNC\server\share\file.txt")),
        "//server/share/file.txt"
    );
    assert_eq!(
        sanitize_path(Path::new(r"assets\icons\logo.png")),
        "assets/icons/logo.png"
    );
    assert_eq!(
        sanitize_path(Path::new("/usr/local/include/header.h")),
        "/usr/local/include/header.h"
    );
}

#[test]
fn test_generate_clang_vfs_overlay_syntax_and_case_insensitivity() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let out_yaml = temp.path().join("vfs_overlay.yaml");

    let dir1 = temp.path().join("sysroot").join("include");
    let dir2 = temp.path().join("sysroot").join("crt");
    std::fs::create_dir_all(&dir1)?;
    std::fs::create_dir_all(&dir2)?;

    crorust_common::generate_clang_vfs_overlay(&out_yaml, &[dir1.clone(), dir2.clone()])?;

    let content = std::fs::read_to_string(&out_yaml)?;

    assert!(content.contains("'version': 0"));
    assert!(
        content.contains("'case-sensitive': 'false'"),
        "VFS overlay must explicitly specify case-sensitive: false"
    );
    assert!(content.contains("'type': 'directory'"));
    assert!(content.contains(&sanitize_path(&dir1)));
    assert!(content.contains(&sanitize_path(&dir2)));

    Ok(())
}
