use crorust_common::{
    normalize_zip_entry_path, unpack_archive_stream_with_strip, ArchiveEntryOptions,
    BoundaryZipArchive,
};
use std::fs::File;
use std::path::Path;
use tempfile::tempdir;
use zip::ZipArchive;

#[test]
fn test_normalize_standard_relative_path() {
    let p = Path::new("assets/icons/logo.png");
    let normalized = normalize_zip_entry_path(p).expect("should succeed on valid relative path");
    assert_eq!(normalized, "assets/icons/logo.png");
}

#[test]
fn test_normalize_strips_leading_root_slash() {
    let p = Path::new("/assets/icons/logo.png");
    let normalized =
        normalize_zip_entry_path(p).expect("should succeed and strip leading root slash");
    assert_eq!(normalized, "assets/icons/logo.png");
}

#[test]
fn test_normalize_skips_current_dir_segments() {
    let p = Path::new("./assets/./sub/logo.png");
    let normalized = normalize_zip_entry_path(p).expect("should succeed and skip '.' components");
    assert_eq!(normalized, "assets/sub/logo.png");
}

#[test]
fn test_normalize_preserves_unicode_characters() {
    let p = Path::new("locales/日本語/カタカナ/音声_🎙️.json");
    let normalized =
        normalize_zip_entry_path(p).expect("should preserve valid UTF-8 international characters");
    assert_eq!(normalized, "locales/日本語/カタカナ/音声_🎙️.json");
}

#[test]
fn test_normalize_rejects_parent_dir_zip_slip() {
    let p = Path::new("../evil.sh");
    let err = normalize_zip_entry_path(p).expect_err("should reject parent directory traversal");
    assert!(err.to_string().contains("parent directory traversal"));

    let nested = Path::new("assets/../../secret.key");
    let nested_err =
        normalize_zip_entry_path(nested).expect_err("should reject nested directory traversal");
    assert!(nested_err
        .to_string()
        .contains("parent directory traversal"));
}

#[test]
fn test_normalize_rejects_empty_paths() {
    let p = Path::new("");
    let err = normalize_zip_entry_path(p).expect_err("should reject empty path");
    assert!(err.to_string().contains("cannot be empty"));

    let root_only = Path::new("/");
    let root_err = normalize_zip_entry_path(root_only)
        .expect_err("should reject path consisting solely of root slash");
    assert!(root_err.to_string().contains("cannot be empty"));
}

#[test]
#[cfg(unix)]
fn test_normalize_rejects_non_utf8_path_with_hex_diagnostic() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let invalid_bytes: &[u8] = b"assets/bad_\x80_file.png";
    let os_str = OsStr::from_bytes(invalid_bytes);
    let p = Path::new(os_str);

    let err = normalize_zip_entry_path(p).expect_err("should reject non-UTF-8 path on Unix");
    let msg = err.to_string();
    assert!(msg.contains("Invalid non-UTF-8 path segment"));
    assert!(msg.contains("\\x80"));
}

#[test]
fn test_zip_create_file_end_to_end_roundtrip() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("archive.zip");

    {
        let mut zip = BoundaryZipArchive::new(&zip_path, false).expect("failed to create zip");
        zip.create_file(
            Path::new("/nested/deeply/config.json"),
            ArchiveEntryOptions::Unaligned,
            b"{\"status\": \"ok\"}",
        )
        .expect("failed to create file in zip");

        zip.create_file(
            Path::new("classes.dex"),
            ArchiveEntryOptions::Aligned(4),
            b"DEX_DATA_BYTES",
        )
        .expect("failed to add aligned dex");

        zip.create_file(
            Path::new("lib/arm64-v8a/libtest.so"),
            ArchiveEntryOptions::Aligned(16384),
            b"ELF_16KB_PAGE_ALIGNED_DATA",
        )
        .expect("failed to add aligned so");

        zip.finish().expect("failed to finish zip");
    }

    let file = File::open(&zip_path).expect("failed to open archive");
    let mut archive = ZipArchive::new(file).expect("failed to read zip archive");
    assert_eq!(archive.len(), 3);

    {
        let entry0 = archive.by_index(0).expect("failed to read first entry");
        assert_eq!(entry0.name(), "nested/deeply/config.json");
    }

    {
        let entry1 = archive.by_name("classes.dex").expect("classes.dex entry");
        assert_eq!(
            entry1.data_start() % 4,
            0,
            "DEX entry data must start on 4-byte boundary"
        );
    }

    {
        let entry2 = archive
            .by_name("lib/arm64-v8a/libtest.so")
            .expect("libtest.so entry");
        assert_eq!(
            entry2.data_start() % 16384,
            0,
            "Shared library entry data must start on 16KB page boundary"
        );
    }
}

#[test]
fn test_unpack_archive_stream_with_strip() {
    let dir = tempdir().expect("failed creating tempdir");
    let zip_path = dir.path().join("archive.zip");
    let extract_dir = dir.path().join("extracted");

    {
        let file = File::create(&zip_path).expect("failed to create zip file");
        let mut zip = BoundaryZipArchive::from_file(file, false);
        zip.create_file(
            Path::new("root_folder/bin/tool"),
            ArchiveEntryOptions::Compressed,
            b"BINARY_PAYLOAD",
        )
        .expect("create file");
        zip.create_file(
            Path::new("root_folder/lib/data.txt"),
            ArchiveEntryOptions::Compressed,
            b"TEXT_DATA",
        )
        .expect("create file");
        zip.finish().expect("finish");
    }

    unpack_archive_stream_with_strip(&zip_path, &extract_dir, 1).expect("unpack with strip 1");

    assert!(extract_dir.join("bin/tool").is_file());
    assert!(extract_dir.join("lib/data.txt").is_file());
    assert_eq!(
        std::fs::read(extract_dir.join("bin/tool")).unwrap(),
        b"BINARY_PAYLOAD"
    );
    assert_eq!(
        std::fs::read(extract_dir.join("lib/data.txt")).unwrap(),
        b"TEXT_DATA"
    );
}
