use anyhow::Result;
use crorust_common::{
    normalize_zip_entry_path, unpack_archive_stream, ArchiveEntryOptions, BoundaryZipArchive,
};
use tempfile::TempDir;

#[test]
fn test_normalize_zip_entry_rejects_zip_slip_parent_dir() {
    let bad_path = std::path::Path::new("subdir/../../etc/passwd");
    let res = normalize_zip_entry_path(bad_path);
    assert!(
        res.is_err(),
        "Must reject parent directory traversal in ZIP entry paths"
    );
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("parent directory traversal"),
        "Error message must clearly identify traversal violation: {err}"
    );
}

#[test]
fn test_normalize_zip_entry_strips_leading_slashes_and_normalizes() {
    let p = std::path::Path::new("/assets/./images//icon.png");
    let normalized = normalize_zip_entry_path(p).expect("Should normalize cleanly");
    assert_eq!(normalized, "assets/images/icon.png");
}

#[test]
fn test_extract_zip_prevents_directory_escape() -> Result<()> {
    let temp = TempDir::new()?;
    let zip_path = temp.path().join("test.zip");

    {
        let mut zip = BoundaryZipArchive::new(&zip_path, false)?;
        zip.create_file(
            std::path::Path::new("safe/inner.txt"),
            ArchiveEntryOptions::Unaligned,
            b"hello inside",
        )?;
        zip.finish()?;
    }

    let extract_dir = temp.path().join("output");
    unpack_archive_stream(&zip_path, &extract_dir)?;

    assert!(extract_dir.join("safe/inner.txt").exists());
    assert_eq!(
        std::fs::read(extract_dir.join("safe/inner.txt"))?,
        b"hello inside"
    );

    Ok(())
}

#[test]
fn test_unpack_rejects_symlink_escape_parent() -> Result<()> {
    let temp = TempDir::new()?;
    let dest = temp.path().join("dest");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&dest)?;
    std::fs::create_dir_all(&outside)?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, dest.join("link_out"))?;

    let zip_path = temp.path().join("malicious.zip");
    {
        let file = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("link_out/pwn.txt", zip::write::FileOptions::default())?;
        std::io::Write::write_all(&mut zip, b"payload")?;
        zip.finish()?;
    }

    #[cfg(unix)]
    {
        let res = crorust_common::unpack_archive_stream(&zip_path, &dest);
        assert!(
            res.is_err(),
            "Extraction through external symlink parent must be rejected"
        );
    }

    Ok(())
}
