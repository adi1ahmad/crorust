use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_atomic_extraction_fails_and_cleans_staging_on_corrupted_archive() {
    let temp_dir = TempDir::new().unwrap();
    let lock_dir = temp_dir.path().join("locks");
    let dest_dir = temp_dir.path().join("tools").join("corrupted-tool");
    let dummy_archive = temp_dir.path().join("corrupted.zip");
    std::fs::write(&dummy_archive, b"corrupted payload").unwrap();

    let result = crorust::download::extract_atomic(
        &dummy_archive,
        &dest_dir,
        &lock_dir,
        "corrupted-tool",
        |_archive, _staging| {
            anyhow::bail!("Simulated CRC32 checksum mismatch in corrupted archive");
        },
    );

    assert!(result.is_err());
    assert!(!dest_dir.exists());
    assert!(!dest_dir.join(".complete").exists());
}

#[test]
fn test_atomic_extraction_creates_complete_marker() {
    let temp_dir = TempDir::new().unwrap();
    let lock_dir = temp_dir.path().join("locks");
    let dest_dir = temp_dir.path().join("tools").join("sample-tool");
    let dummy_archive = temp_dir.path().join("sample.zip");
    std::fs::write(&dummy_archive, b"dummy content").unwrap();

    let result = crorust::download::extract_atomic(
        &dummy_archive,
        &dest_dir,
        &lock_dir,
        "sample-tool",
        |_archive, staging| {
            std::fs::write(staging.join("binary"), b"executable code")?;
            Ok(())
        },
    );

    assert!(result.is_ok());
    assert!(dest_dir.exists());
    assert!(dest_dir.join(".complete").exists());
    assert!(dest_dir.join("binary").exists());
}

#[test]
fn test_atomic_extraction_recovers_from_partial_staging() {
    let temp_dir = TempDir::new().unwrap();
    let lock_dir = temp_dir.path().join("locks");
    let dest_dir = temp_dir.path().join("tools").join("interrupted-tool");
    let dummy_archive = temp_dir.path().join("sample.zip");
    std::fs::write(&dummy_archive, b"dummy content").unwrap();

    let staging_dir = temp_dir
        .path()
        .join("staging")
        .join(".staging-interrupted-tool-9999");
    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::write(staging_dir.join("half-written.tmp"), b"junk").unwrap();

    let result = crorust::download::extract_atomic(
        &dummy_archive,
        &dest_dir,
        &lock_dir,
        "interrupted-tool",
        |_archive, staging| {
            std::fs::write(staging.join("valid-file"), b"clean data")?;
            Ok(())
        },
    );

    assert!(result.is_ok());
    assert!(dest_dir.exists());
    assert!(dest_dir.join(".complete").exists());
    assert!(dest_dir.join("valid-file").exists());
    assert!(!dest_dir.join("half-written.tmp").exists());
}

#[test]
fn test_concurrent_locks_serialize_cleanly() {
    let temp_dir = Arc::new(TempDir::new().unwrap());
    let lock_dir = temp_dir.path().join("locks");
    let dest_dir = temp_dir.path().join("tools").join("concurrent-tool");
    let dummy_archive = temp_dir.path().join("sample.zip");
    std::fs::write(&dummy_archive, b"dummy content").unwrap();

    let mut handles = vec![];
    for thread_idx in 0..4 {
        let _temp_dir = Arc::clone(&temp_dir);
        let lock_dir = lock_dir.clone();
        let dest_dir = dest_dir.clone();
        let dummy_archive = dummy_archive.clone();

        handles.push(std::thread::spawn(move || {
            crorust::download::extract_atomic(
                &dummy_archive,
                &dest_dir,
                &lock_dir,
                "concurrent-tool",
                |_archive, staging| {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    std::fs::write(
                        staging.join(format!("output-{thread_idx}.txt")),
                        b"extracted",
                    )?;
                    Ok(())
                },
            )
        }));
    }

    for handle in handles {
        let res = handle.join().unwrap();
        assert!(res.is_ok());
    }

    assert!(dest_dir.exists());
    assert!(dest_dir.join(".complete").exists());
}

#[test]
fn test_tool_manager_canonical_sysroot_discovery() {
    use crorust::download::catalog::{ToolCatalog, ToolCategory};
    use crorust::download::manager::ToolManager;

    let temp_dir = TempDir::new().unwrap();
    let mgr = ToolManager::with_cache_dir(
        temp_dir.path().to_path_buf(),
        "x86_64-unknown-linux-gnu",
        None,
        true,
    )
    .unwrap();

    let linux_artifact = ToolCatalog::get_sysroot("Linux.sysroot-x86_64").unwrap();
    assert_eq!(linux_artifact.strip_components, 0);

    assert!(mgr.find_installed(&linux_artifact).is_none());

    let canonical_dir = mgr.canonical_install_dir(&linux_artifact);
    std::fs::create_dir_all(&canonical_dir).unwrap();
    std::fs::write(canonical_dir.join(".complete"), b"ok").unwrap();
    assert_eq!(mgr.find_installed(&linux_artifact).unwrap(), canonical_dir);

    let ndk_artifact = ToolCatalog::get_ndk();
    assert_eq!(ndk_artifact.category, ToolCategory::Ndk);
    assert_eq!(
        ToolCatalog::expected_sha256_for_filename(linux_artifact.archive_filename()),
        Some(linux_artifact.sha256)
    );
}

#[test]
fn test_generate_unique_part_path_collision_free_across_threads() {
    use crorust::download::generate_unique_part_path;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;

    let dest = PathBuf::from("/tmp/cache/artifact.jar");
    let seen_paths = Arc::new(Mutex::new(HashSet::new()));
    let thread_count = 50;
    let iterations_per_thread = 100;
    let mut handles = Vec::new();

    for _ in 0..thread_count {
        let seen = Arc::clone(&seen_paths);
        let dest = dest.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..iterations_per_thread {
                let part = generate_unique_part_path(&dest);
                let mut guard = seen.lock().unwrap();
                assert!(
                    guard.insert(part.clone()),
                    "Detected duplicate temporary part path: {:?}",
                    part
                );
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked");
    }

    let total = seen_paths.lock().unwrap().len();
    assert_eq!(total, thread_count * iterations_per_thread);
}

#[test]
fn test_verifiable_download_sha256_mismatch_cleans_temporary_part_file() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let dest_file = tmp.path().join("test_asset.bin");
    let client = crorust::download::create_hardened_http_client()?;

    // Create a local test fixture file to serve as a mock download source
    let fixture_src = tmp.path().join("fixture_src.bin");
    std::fs::write(&fixture_src, b"Valid mock payload contents")?;
    let local_url = format!("file://{}", fixture_src.display());

    // Expecting an incorrect SHA-256 hash must trigger error and clean up .part file
    let wrong_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let verifier = crorust::download::VerifiableDownload::new_quiet(&local_url, Some(wrong_sha));

    let result = verifier.download_with_auth(&client, &dest_file, None);
    assert!(result.is_err(), "Download must fail on SHA-256 mismatch");

    // Ensure final target file was NOT created
    assert!(
        !dest_file.exists(),
        "Target file must not exist on checksum failure"
    );

    // Ensure no orphaned .part files remain in the target directory
    let has_dangling_parts = std::fs::read_dir(tmp.path())?
        .flatten()
        .any(|e| e.file_name().to_string_lossy().contains(".part-"));
    assert!(
        !has_dangling_parts,
        "All temporary .part files must be cleaned up on failure"
    );

    Ok(())
}

#[test]
fn test_parallel_tool_extraction_and_marker_generation() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let cache_dir = tmp.path().to_path_buf();
    let lock_dir = cache_dir.join("locks");
    let tools_dir = cache_dir.join("tools");
    std::fs::create_dir_all(&lock_dir)?;
    std::fs::create_dir_all(&tools_dir)?;

    // Create 4 valid mock zip tool archives
    let mut archives = Vec::new();
    for i in 1..=4 {
        let tool_name = format!("mock_tool_{i}");
        let target_dir = tools_dir.join(&tool_name);
        let archive_path = cache_dir.join(format!("{tool_name}.zip"));

        let file = std::fs::File::create(&archive_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        zip.start_file(format!("{tool_name}.bin"), options)?;
        use std::io::Write;
        zip.write_all(format!("Binary payload {i}").as_bytes())?;
        zip.finish()?;

        archives.push((tool_name, archive_path, target_dir, i));
    }

    // Extract in parallel across threads using Rayon
    use rayon::prelude::*;
    archives.par_iter().try_for_each(
        |(tool_name, archive_path, target_dir, _i)| -> anyhow::Result<()> {
            crorust::download::extract_atomic(
                archive_path,
                target_dir,
                &lock_dir,
                tool_name,
                crorust_common::unpack_archive_stream,
            )
        },
    )?;

    // Assert all .complete markers exist and binaries are intact
    for (tool_name, _, target_dir, i) in &archives {
        assert!(
            target_dir.join(".complete").exists(),
            "Complete marker must be created for {tool_name}"
        );
        let binary_path = target_dir.join(format!("{tool_name}.bin"));
        assert!(binary_path.exists(), "Extracted binary must exist");
        let content = std::fs::read_to_string(&binary_path)?;
        assert_eq!(content, format!("Binary payload {i}"));
    }

    Ok(())
}

#[test]
fn test_atomic_rename_with_backoff_success() {
    let temp_dir = TempDir::new().unwrap();
    let staging_dir = temp_dir.path().join("staging_dir");
    let dest_dir = temp_dir.path().join("dest_dir");

    std::fs::create_dir_all(staging_dir.join("sub")).unwrap();
    std::fs::write(staging_dir.join("sub").join("file.txt"), b"atomic data").unwrap();

    let res = crorust::download::atomic_rename_with_backoff(&staging_dir, &dest_dir);
    assert!(res.is_ok());
    assert!(!staging_dir.exists());
    assert!(dest_dir.join("sub").join("file.txt").exists());
    assert_eq!(
        std::fs::read(dest_dir.join("sub").join("file.txt")).unwrap(),
        b"atomic data"
    );
}

#[test]
fn test_tar_archive_with_hardlinks_extracts_cleanly() -> anyhow::Result<()> {
    use crorust::download::catalog::ExtractionPolicy;
    use crorust::download::manager::unpack_tar_archive_with_strip;
    use std::io::Cursor;
    use tar::{Builder, Header};

    let mut tar_builder = Builder::new(Vec::new());

    // 1. Regular file
    let file_data = b"shared hardlink data content";
    let mut header = Header::new_gnu();
    header.set_size(file_data.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    tar_builder.append_data(&mut header, "pkg/lib/liboriginal.so", &file_data[..])?;

    // 2. Hardlink pointing to the first file
    let mut hl_header = Header::new_gnu();
    hl_header.set_size(0);
    hl_header.set_entry_type(tar::EntryType::Link);
    hl_header.set_link_name("pkg/lib/liboriginal.so")?;
    hl_header.set_cksum();
    tar_builder.append_data(&mut hl_header, "pkg/lib/libalias.so", Cursor::new(&[]))?;

    let tar_bytes = tar_builder.into_inner()?;

    let temp_dir = TempDir::new()?;
    let staging_dir = temp_dir.path().join("staging");
    std::fs::create_dir_all(&staging_dir)?;

    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    unpack_tar_archive_with_strip(
        &mut archive,
        &staging_dir,
        1, // strip "pkg"
        ExtractionPolicy::default(),
    )?;

    let orig_path = staging_dir.join("lib").join("liboriginal.so");
    let link_path = staging_dir.join("lib").join("libalias.so");

    assert!(orig_path.exists(), "Original file must exist");
    assert!(link_path.exists(), "Hardlink file must exist");
    assert_eq!(std::fs::read(&orig_path)?, file_data);
    assert_eq!(std::fs::read(&link_path)?, file_data);

    Ok(())
}

#[test]
fn test_tar_archive_with_symlinks_extracts_cleanly() -> anyhow::Result<()> {
    use crorust::download::catalog::ExtractionPolicy;
    use crorust::download::manager::unpack_tar_archive_with_strip;
    use std::io::Cursor;
    use tar::{Builder, Header};

    let mut tar_builder = Builder::new(Vec::new());

    // 1. Regular file
    let file_data = b"target library data";
    let mut header = Header::new_gnu();
    header.set_size(file_data.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    tar_builder.append_data(&mut header, "pkg/lib/libtarget.so.1", &file_data[..])?;

    // 2. Relative symlink to file with forward slashes
    let mut sl_header = Header::new_gnu();
    sl_header.set_size(0);
    sl_header.set_entry_type(tar::EntryType::Symlink);
    sl_header.set_link_name("libtarget.so.1")?;
    sl_header.set_cksum();
    tar_builder.append_data(&mut sl_header, "pkg/lib/libtarget.so", Cursor::new(&[]))?;

    // 3. Directory and nested header file
    let header_data = b"#define TEST 1\n";
    let mut h_header = Header::new_gnu();
    h_header.set_size(header_data.len() as u64);
    h_header.set_mode(0o644);
    h_header.set_entry_type(tar::EntryType::Regular);
    h_header.set_cksum();
    tar_builder.append_data(
        &mut h_header,
        "pkg/Frameworks/My.framework/Versions/A/Headers/My.h",
        &header_data[..],
    )?;

    // 4. Relative symlink with forward slashes pointing across folders: Versions/A/Headers
    let mut dir_sl_header = Header::new_gnu();
    dir_sl_header.set_size(0);
    dir_sl_header.set_entry_type(tar::EntryType::Symlink);
    dir_sl_header.set_link_name("Versions/A/Headers")?;
    dir_sl_header.set_cksum();
    tar_builder.append_data(
        &mut dir_sl_header,
        "pkg/Frameworks/My.framework/Headers",
        Cursor::new(&[]),
    )?;

    let tar_bytes = tar_builder.into_inner()?;

    let temp_dir = TempDir::new()?;
    let staging_dir = temp_dir.path().join("staging");
    std::fs::create_dir_all(&staging_dir)?;

    let mut archive = tar::Archive::new(Cursor::new(tar_bytes));
    unpack_tar_archive_with_strip(
        &mut archive,
        &staging_dir,
        1, // strip "pkg"
        ExtractionPolicy::default(),
    )?;

    let lib_symlink = staging_dir.join("lib").join("libtarget.so");
    let header_symlink = staging_dir
        .join("Frameworks")
        .join("My.framework")
        .join("Headers")
        .join("My.h");

    assert!(lib_symlink.exists(), "File symlink target must exist");
    assert_eq!(std::fs::read(&lib_symlink)?, file_data);

    assert!(
        header_symlink.exists(),
        "Directory symlink sub-path must exist"
    );
    assert_eq!(std::fs::read(&header_symlink)?, header_data);

    Ok(())
}
