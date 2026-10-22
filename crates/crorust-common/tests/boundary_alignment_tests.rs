use crorust_common::archive_directory::locate_central_directory_anchor;
use crorust_common::boundary_archive::{AlignedArchiveSink, EntryCompressionStrategy};
use std::io::Cursor;

#[test]
fn test_16kb_page_alignment_for_shared_libraries() {
    let mut sink = AlignedArchiveSink::in_memory();

    let dummy_dex = b"dex\n035\0dummy_bytecode_payload";
    let dummy_so = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];

    sink.write_entry(
        "assets/data.txt",
        b"plain text",
        EntryCompressionStrategy::CompressedDeflate,
    )
    .expect("write data.txt");

    sink.write_entry(
        "classes.dex",
        dummy_dex,
        EntryCompressionStrategy::StoredBoundaryAligned(4),
    )
    .expect("write classes.dex");

    sink.write_entry(
        "lib/arm64-v8a/libnative.so",
        &dummy_so,
        EntryCompressionStrategy::StoredBoundaryAligned(16384),
    )
    .expect("write libnative.so");

    let archive_bytes = sink.finalize_archive_bytes().expect("finalize bytes");
    assert!(!archive_bytes.is_empty());

    let cursor = Cursor::new(&archive_bytes);
    let mut zip = zip::ZipArchive::new(cursor).expect("valid zip");

    let so_offset = {
        let so_file = zip
            .by_name("lib/arm64-v8a/libnative.so")
            .expect("find libnative.so");
        so_file.data_start()
    };
    assert_eq!(
        so_offset % 16384,
        0,
        "libnative.so data offset ({so_offset}) must be 16KB-page aligned for Android 15 zero-copy mmap"
    );

    let dex_offset = {
        let dex_file = zip.by_name("classes.dex").expect("find classes.dex");
        dex_file.data_start()
    };
    assert_eq!(
        dex_offset % 4,
        0,
        "classes.dex data offset ({dex_offset}) must be 4-byte aligned for Dalvik runtime mmap"
    );
}

#[test]
fn test_eocd_discovery_under_trailing_padding() {
    let mut sink = AlignedArchiveSink::in_memory();
    sink.write_entry(
        "test.txt",
        b"Hello, boundary test",
        EntryCompressionStrategy::CompressedDeflate,
    )
    .expect("write test.txt");
    let mut archive_bytes = sink.finalize_archive_bytes().expect("finalize");

    let mut cursor = Cursor::new(&archive_bytes);
    let original_eocd = locate_central_directory_anchor(&mut cursor).expect("locate anchor");
    assert!(original_eocd > 0);

    archive_bytes.extend_from_slice(&[0xAA; 512]);

    let mut padded_cursor = Cursor::new(&archive_bytes);
    let detected_eocd =
        locate_central_directory_anchor(&mut padded_cursor).expect("locate anchor with padding");
    assert_eq!(detected_eocd, original_eocd);
}
