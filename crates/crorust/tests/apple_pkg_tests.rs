use crorust::download::apple_pkg::CpioExtractor;
use std::fs;
use tempfile::tempdir;

fn make_cpio_entry(name: &str, mode: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let namesize = name.len() + 1;
    let filesize = data.len();
    let hdr = format!(
        "070701{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        0, mode, 0, 0, 1, 0, filesize, 0, 0, 0, 0, namesize, 0
    );
    out.extend_from_slice(hdr.as_bytes());
    out.extend_from_slice(name.as_bytes());
    out.push(0); // null terminator

    let header_plus_name = 110 + namesize;
    let name_pad = ((header_plus_name + 3) & !3) - header_plus_name;
    out.resize(out.len() + name_pad, 0);

    out.extend_from_slice(data);
    let data_pad = ((filesize + 3) & !3) - filesize;
    out.resize(out.len() + data_pad, 0);

    out
}

fn make_trailer() -> Vec<u8> {
    make_cpio_entry("TRAILER!!!", 0, &[])
}

#[test]
fn test_cpio_extraction_files_and_directories() {
    let mut archive = Vec::new();
    archive.extend(make_cpio_entry("usr", 0o040755, &[]));
    archive.extend(make_cpio_entry("usr/lib", 0o040755, &[]));
    archive.extend(make_cpio_entry(
        "usr/lib/libSystem.tbd",
        0o100644,
        b"--- !tapi-tbd\n",
    ));
    archive.extend(make_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("Extraction must succeed");

    let extracted_file = dir.path().join("usr/lib/libSystem.tbd");
    assert!(extracted_file.is_file());
    assert_eq!(
        fs::read_to_string(&extracted_file).unwrap(),
        "--- !tapi-tbd\n"
    );
}

#[test]
fn test_cpio_extraction_with_chained_symlinks() {
    let mut archive = Vec::new();
    // Simulate macOS framework structure
    // Framework.framework/Versions/A/Framework.tbd
    // Framework.framework/Versions/Current -> A
    // Framework.framework/Framework.tbd -> Versions/Current/Framework.tbd
    archive.extend(make_cpio_entry(
        "System/Library/Frameworks/Test.framework/Versions/A",
        0o040755,
        &[],
    ));
    archive.extend(make_cpio_entry(
        "System/Library/Frameworks/Test.framework/Versions/A/Test.tbd",
        0o100644,
        b"--- !tapi-tbd-v4\narchs: [arm64]\n",
    ));
    archive.extend(make_cpio_entry(
        "System/Library/Frameworks/Test.framework/Versions/Current",
        0o120777,
        b"A\0",
    ));
    archive.extend(make_cpio_entry(
        "System/Library/Frameworks/Test.framework/Test.tbd",
        0o120777,
        b"Versions/Current/Test.tbd\0",
    ));
    archive.extend(make_cpio_entry(
        "usr/lib/libc.tbd",
        0o120777,
        b"libSystem.tbd\0",
    ));
    archive.extend(make_cpio_entry(
        "usr/lib/libSystem.tbd",
        0o100644,
        b"--- !tapi-tbd\n",
    ));
    archive.extend(make_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("Extraction must succeed");

    // Verify framework symlink copied or linked correctly
    let top_tbd = dir
        .path()
        .join("System/Library/Frameworks/Test.framework/Test.tbd");
    assert!(top_tbd.exists(), "Test.tbd must exist on disk");
    let content = fs::read_to_string(&top_tbd).unwrap();
    assert_eq!(content, "--- !tapi-tbd-v4\narchs: [arm64]\n");

    // Verify libc.tbd resolved to libSystem.tbd
    let libc = dir.path().join("usr/lib/libc.tbd");
    assert!(libc.exists(), "libc.tbd must exist on disk");
    assert_eq!(fs::read_to_string(&libc).unwrap(), "--- !tapi-tbd\n");
}

#[test]
fn test_cpio_zip_slip_rejection() {
    let mut archive = Vec::new();
    archive.extend(make_cpio_entry(
        "../../../etc/evil.txt",
        0o100644,
        b"malicious",
    ));
    archive.extend(make_trailer());

    let dir = tempdir().unwrap();
    let res = CpioExtractor::extract(&archive[..], dir.path());
    assert!(res.is_err(), "Zip slip path must be rejected");
}

#[test]
fn test_cpio_skips_man_pages_and_docs() {
    let mut archive = Vec::new();
    // Valid SDK header
    archive.extend(make_cpio_entry(
        "usr/include/stdio.h",
        0o100644,
        b"#pragma once\n",
    ));
    // Man page that should be skipped
    archive.extend(make_cpio_entry(
        "usr/share/man/man1/clang.1",
        0o100644,
        b".TH CLANG 1\n",
    ));
    // Doc file that should be skipped
    archive.extend(make_cpio_entry(
        "usr/share/doc/clang/README.txt",
        0o100644,
        b"Documentation\n",
    ));
    archive.extend(make_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("Extraction must succeed");

    assert!(
        dir.path().join("usr/include/stdio.h").exists(),
        "SDK header must be extracted"
    );
    assert!(
        !dir.path().join("usr/share/man/man1/clang.1").exists(),
        "Man page must not be extracted"
    );
    assert!(
        !dir.path().join("usr/share/doc/clang/README.txt").exists(),
        "Doc file must not be extracted"
    );
}

#[test]
fn test_cpio_skips_windows_illegal_characters() {
    let mut archive = Vec::new();
    // Valid SDK file
    archive.extend(make_cpio_entry(
        "usr/lib/libtest.tbd",
        0o100644,
        b"--- !tapi-tbd\n",
    ));
    // Entry with colon ':' such as Perl manpage that causes os error 123 on Windows
    archive.extend(make_cpio_entry(
        "usr/share/man/man3/Mail::DKIM::TextWrap5.30.3pm",
        0o100644,
        b".TH MAIL 3pm\n",
    ));
    // Entry outside man/doc but with colon in filename
    archive.extend(make_cpio_entry(
        "usr/bin/tool:invalid",
        0o100644,
        b"invalid\n",
    ));
    archive.extend(make_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("Extraction must succeed");

    assert!(
        dir.path().join("usr/lib/libtest.tbd").exists(),
        "Valid file must be extracted"
    );
    #[cfg(windows)]
    assert!(
        !dir.path().join("usr/bin/tool:invalid").exists(),
        "Illegal colon entry must be skipped on Windows"
    );
}

fn make_odc_entry(name: &str, mode: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let namesize = name.len() + 1;
    let filesize = data.len();

    // 76 bytes POSIX odc header:
    // [0..6]   c_magic    "070707"
    // [6..12]  c_dev      6 octal digits
    // [12..18] c_ino      6 octal digits
    // [18..24] c_mode     6 octal digits
    // [24..30] c_uid      6 octal digits
    // [30..36] c_gid      6 octal digits
    // [36..42] c_nlink    6 octal digits
    // [42..48] c_rdev     6 octal digits
    // [48..59] c_mtime    11 octal digits
    // [59..65] c_namesize 6 octal digits
    // [65..76] c_filesize 11 octal digits
    let hdr = format!(
        "070707{:06o}{:06o}{:06o}{:06o}{:06o}{:06o}{:06o}{:011o}{:06o}{:011o}",
        0, 0, mode, 0, 0, 1, 0, 0, namesize, filesize
    );
    assert_eq!(hdr.len(), 76);
    out.extend_from_slice(hdr.as_bytes());
    out.extend_from_slice(name.as_bytes());
    out.push(0); // null terminator

    // POSIX odc format has no alignment padding
    out.extend_from_slice(data);
    out
}

fn make_odc_trailer() -> Vec<u8> {
    make_odc_entry("TRAILER!!!", 0, &[])
}

#[test]
fn test_cpio_odc_extraction_files_and_directories() {
    let mut archive = Vec::new();
    archive.extend(make_odc_entry("usr", 0o040755, &[]));
    archive.extend(make_odc_entry("usr/lib", 0o040755, &[]));
    archive.extend(make_odc_entry(
        "usr/lib/libSystem.tbd",
        0o100644,
        b"--- !tapi-tbd\n",
    ));
    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("ODC Extraction must succeed");

    let extracted_file = dir.path().join("usr/lib/libSystem.tbd");
    assert!(extracted_file.is_file());
    assert_eq!(
        fs::read_to_string(&extracted_file).unwrap(),
        "--- !tapi-tbd\n"
    );
}

#[test]
fn test_cpio_odc_extraction_with_chained_symlinks() {
    let mut archive = Vec::new();
    // Simulate macOS framework structure under ODC:
    // Framework.framework/Versions/A/Framework.tbd
    // Framework.framework/Versions/Current -> A
    // Framework.framework/Framework.tbd -> Versions/Current/Framework.tbd
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/ApplicationServices.framework/Versions/A",
        0o040755,
        &[],
    ));
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/ApplicationServices.framework/Versions/A/ApplicationServices.tbd",
        0o100644,
        b"--- !tapi-tbd-v4\narchs: [arm64]\n",
    ));
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/ApplicationServices.framework/Versions/Current",
        0o120777,
        b"A\0",
    ));
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/ApplicationServices.framework/ApplicationServices.tbd",
        0o120777,
        b"Versions/Current/ApplicationServices.tbd\0",
    ));
    archive.extend(make_odc_entry(
        "usr/lib/libc.tbd",
        0o120777,
        b"libSystem.tbd\0",
    ));
    archive.extend(make_odc_entry(
        "usr/lib/libSystem.tbd",
        0o100644,
        b"--- !tapi-tbd\n",
    ));
    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("ODC Extraction must succeed");

    // Verify framework symlink copied or linked correctly
    let top_tbd = dir
        .path()
        .join("System/Library/Frameworks/ApplicationServices.framework/ApplicationServices.tbd");
    assert!(
        top_tbd.exists(),
        "ApplicationServices.tbd must exist on disk"
    );
    let content = fs::read_to_string(&top_tbd).unwrap();
    assert_eq!(content, "--- !tapi-tbd-v4\narchs: [arm64]\n");

    // Verify libc.tbd resolved to libSystem.tbd
    let libc = dir.path().join("usr/lib/libc.tbd");
    assert!(libc.exists(), "libc.tbd must exist on disk");
    assert_eq!(fs::read_to_string(&libc).unwrap(), "--- !tapi-tbd\n");
}

#[test]
fn test_cpio_odc_zip_slip_rejection() {
    let mut archive = Vec::new();
    archive.extend(make_odc_entry(
        "../../../etc/evil.txt",
        0o100644,
        b"malicious",
    ));
    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    let res = CpioExtractor::extract(&archive[..], dir.path());
    assert!(res.is_err(), "Zip slip path in ODC must be rejected");
}

fn make_odc_entry_with_ino(name: &str, ino: u64, mode: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let namesize = name.len() + 1;
    let filesize = data.len();

    let hdr = format!(
        "070707{:06o}{:06o}{:06o}{:06o}{:06o}{:06o}{:06o}{:011o}{:06o}{:011o}",
        0, ino, mode, 0, 0, 1, 0, 0, namesize, filesize
    );
    assert_eq!(hdr.len(), 76);
    out.extend_from_slice(hdr.as_bytes());
    out.extend_from_slice(name.as_bytes());
    out.push(0); // null terminator

    out.extend_from_slice(data);
    out
}

#[test]
fn test_cpio_windows_backslash_paths() {
    let mut archive = Vec::new();
    archive.extend(make_odc_entry(
        r".\usr\bin\clang",
        0o100755,
        b"#!/bin/sh\necho clang",
    ));
    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("ODC Extraction must succeed");

    let extracted_file = dir.path().join("usr").join("bin").join("clang");
    assert!(extracted_file.is_file());
    assert_eq!(
        fs::read_to_string(&extracted_file).unwrap(),
        "#!/bin/sh\necho clang"
    );
}

#[test]
fn test_cpio_hardlink_extraction() {
    let mut archive = Vec::new();
    // First entry has content and non-zero ino
    archive.extend(make_odc_entry_with_ino(
        "usr/lib/libSystem.B.dylib",
        12345,
        0o100755,
        b"libSystem-payload-binary-bytes",
    ));
    // Hard link entry has filesize == 0 and matching ino
    archive.extend(make_odc_entry_with_ino(
        "usr/lib/libSystem.dylib",
        12345,
        0o100755,
        &[],
    ));
    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("Hard link extraction must succeed");

    let hardlink = dir.path().join("usr/lib/libSystem.dylib");
    assert!(hardlink.is_file(), "Hard link file must exist on disk");
    assert_eq!(
        fs::read(&hardlink).unwrap(),
        b"libSystem-payload-binary-bytes"
    );
}

#[test]
fn test_cpio_framework_tbd_materialization() {
    let mut archive = Vec::new();

    // Directory: System/Library/Frameworks/CoreGraphics.framework/Versions/A
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/CoreGraphics.framework/Versions/A",
        0o040755,
        &[],
    ));

    // File: System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics.tbd
    let tbd_content = b"--- !tapi-tbd\ntbd-version: 4\ntargets: [arm64-macos]\ninstall-name: '/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics'\n";
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics.tbd",
        0o100644,
        tbd_content,
    ));

    // Symlink: System/Library/Frameworks/CoreGraphics.framework/Versions/Current -> A
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/CoreGraphics.framework/Versions/Current",
        0o120777,
        b"A\0",
    ));

    // Symlink: System/Library/Frameworks/CoreGraphics.framework/CoreGraphics.tbd -> Versions/Current/CoreGraphics.tbd
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/CoreGraphics.framework/CoreGraphics.tbd",
        0o120777,
        b"Versions/Current/CoreGraphics.tbd\0",
    ));

    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path()).expect("Extraction must succeed");

    let top_tbd = dir
        .path()
        .join("System/Library/Frameworks/CoreGraphics.framework/CoreGraphics.tbd");
    assert!(top_tbd.exists(), "CoreGraphics.tbd must exist on disk");

    let content = fs::read(&top_tbd).unwrap();
    assert_eq!(
        content, tbd_content,
        "CoreGraphics.tbd must contain actual TBD content, not a symlink path string"
    );
}

#[test]
fn test_cpio_self_referential_directory_symlink_does_not_overflow_stack() {
    let mut archive = Vec::new();
    // Reproduces Ruby.framework/Versions/2.6/Headers/ruby/ruby -> . in MacOSX14.5.sdk
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/Ruby.framework/Versions/2.6/Headers/ruby",
        0o040755,
        &[],
    ));
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/Ruby.framework/Versions/2.6/Headers/ruby/ruby.h",
        0o100644,
        b"#define RUBY_H 1\n",
    ));
    archive.extend(make_odc_entry(
        "System/Library/Frameworks/Ruby.framework/Versions/2.6/Headers/ruby/ruby",
        0o120777,
        b".\0",
    ));
    archive.extend(make_odc_trailer());

    let dir = tempdir().unwrap();
    CpioExtractor::extract(&archive[..], dir.path())
        .expect("Self-referential symlink extraction must not overflow stack");

    let header = dir
        .path()
        .join("System/Library/Frameworks/Ruby.framework/Versions/2.6/Headers/ruby/ruby.h");
    assert!(header.is_file());
}
