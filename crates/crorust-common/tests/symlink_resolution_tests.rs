use anyhow::Result;
use crorust_common::{
    copy_symlink_entry, unpack_archive_stream, ArchiveEntryOptions, BoundaryZipArchive,
};
use std::fs::{self, File};
use std::path::Path;
use zip::ZipArchive;

#[test]
#[cfg(unix)]
fn test_copy_dir_all_relative_symlinks_preservation() -> Result<()> {
    use crorust_common::copy_dir_all;
    use std::io::Write;

    let temp = tempfile::tempdir()?;
    let src = temp.path().join("src");
    let dst = temp.path().join("dst");

    let src_lib = src.join("lib");
    let src_sub = src_lib.join("sub");
    fs::create_dir_all(&src_sub)?;

    let real_file = src_lib.join("libfoo.so.1.0");
    let mut f = File::create(&real_file)?;
    f.write_all(b"ELF_BINARY_DATA_TEST_PAYLOAD")?;

    std::os::unix::fs::symlink(Path::new("libfoo.so.1.0"), src_lib.join("libfoo.so"))?;

    std::os::unix::fs::symlink(Path::new("../libfoo.so.1.0"), src_sub.join("libbar.so"))?;

    copy_dir_all(&src, &dst)?;

    let dst_real = dst.join("lib").join("libfoo.so.1.0");
    let dst_sibling_link = dst.join("lib").join("libfoo.so");
    let dst_nested_link = dst.join("lib").join("sub").join("libbar.so");

    assert!(dst_real.exists(), "Real file was not copied");
    assert!(
        dst_sibling_link.is_symlink(),
        "Sibling link is not a symlink"
    );
    assert!(dst_nested_link.is_symlink(), "Nested link is not a symlink");

    let target_sibling = fs::read_link(&dst_sibling_link)?;
    let target_nested = fs::read_link(&dst_nested_link)?;

    assert_eq!(target_sibling, Path::new("libfoo.so.1.0"));
    assert_eq!(target_nested, Path::new("../libfoo.so.1.0"));

    let sibling_content = fs::read(&dst_sibling_link)?;
    let nested_content = fs::read(&dst_nested_link)?;
    assert_eq!(sibling_content, b"ELF_BINARY_DATA_TEST_PAYLOAD");
    assert_eq!(nested_content, b"ELF_BINARY_DATA_TEST_PAYLOAD");

    Ok(())
}

#[test]
fn test_copy_symlink_entry_relative_link_creation() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let src = temp.path().join("src");
    let dst = temp.path().join("dst");
    fs::create_dir_all(&src)?;
    fs::create_dir_all(&dst)?;

    let target_file = src.join("original.txt");
    fs::write(&target_file, b"HELLO_SYMLINK")?;

    fs::copy(&target_file, dst.join("original.txt"))?;

    let raw_target = Path::new("original.txt");
    let resolved_src = src.join(raw_target);
    let dst_link = dst.join("copied_link.txt");

    copy_symlink_entry(raw_target, &resolved_src, &dst_link)?;

    #[cfg(unix)]
    {
        assert!(dst_link.is_symlink());
        assert_eq!(fs::read_link(&dst_link)?, raw_target);
    }
    let content = fs::read(&dst_link)?;
    assert_eq!(content, b"HELLO_SYMLINK");

    Ok(())
}

#[test]
fn test_zip_add_directory_follows_symlinks_deterministically() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let src = temp.path().join("src_assets");
    fs::create_dir_all(&src)?;

    fs::write(src.join("z.txt"), b"ZZZ")?;
    fs::write(src.join("m.txt"), b"MMM")?;
    fs::write(src.join("a.txt"), b"AAA")?;

    #[cfg(unix)]
    {
        let external_target = temp.path().join("external.txt");
        fs::write(&external_target, b"EXTERNAL_TARGET")?;
        std::os::unix::fs::symlink(&external_target, src.join("sym_ext.txt"))?;
    }

    let zip_file = temp.path().join("out.zip");
    {
        let mut zip = BoundaryZipArchive::new(&zip_file, false)?;
        zip.add_directory(&src, Path::new("assets"), ArchiveEntryOptions::Unaligned)?;
        zip.finish()?;
    }

    let mut archive = ZipArchive::new(File::open(&zip_file)?)?;
    let mut names = Vec::new();
    for i in 0..archive.len() {
        names.push(archive.by_index(i)?.name().to_string());
    }

    let mut sorted_names = names.clone();
    sorted_names.sort();
    assert_eq!(
        names, sorted_names,
        "ZIP entries must be deterministically sorted"
    );

    let extract_dir = temp.path().join("extracted");
    unpack_archive_stream(&zip_file, &extract_dir)?;
    assert_eq!(fs::read(extract_dir.join("assets").join("a.txt"))?, b"AAA");
    #[cfg(unix)]
    {
        assert_eq!(
            fs::read(extract_dir.join("assets").join("sym_ext.txt"))?,
            b"EXTERNAL_TARGET"
        );
    }

    Ok(())
}

#[test]
fn test_copy_dir_all_cycle_termination() -> Result<()> {
    // Create self-referential directory symlink on Unix
    #[cfg(unix)]
    {
        let tmp = tempfile::tempdir()?;
        let src = tmp.path().join("source");
        let dst = tmp.path().join("dest");
        std::fs::create_dir_all(&src)?;
        std::os::unix::fs::symlink(&src, src.join("self_loop"))?;
        let res = crorust_common::copy_dir_all(&src, &dst);
        assert!(
            res.is_ok(),
            "Directory walk with self-referential loop must not hang or crash"
        );
    }

    Ok(())
}
