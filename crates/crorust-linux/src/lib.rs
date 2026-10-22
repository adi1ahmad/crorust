//! # crorust-linux: Pure-Rust Linux AppDir & Type 2 AppImage Bundler
//!
//! Assembles Linux AppDir structures, creates SquashFS v4 filesystems using `backhand`,
//! and builds self-executing Type 2 AppImages without requiring Linux host tools or root.
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Assembly["AppDir Filesystem"]
//!         Bin[Linux ELF Executable] --> Dir[AppDirFilesystemLayout]
//!         Icon[PNG Icon] --> Dir
//!         Desk[Desktop Entry] --> Dir
//!         Run[AppRun Entrypoint] --> Dir
//!     end
//!
//!     subgraph Compression["SquashFS v4 Generation"]
//!         Dir --> Back[Pure-Rust backhand<br/><i>gzip / xz compression</i>]
//!     end
//!
//!     subgraph Composite["Composite Binary Construction"]
//!         Stub[Embedded 128KB ELF Runtime Stub] --> Comp[Binary Concatenation]
//!         Back --> Comp
//!         Comp --> AppImg[Type 2 .AppImage]
//!     end
//! ```
//!
//! ## Core Modules
//! - [`appdir`]: Freedesktop-compliant application directory layout.
//! - [`appimage`]: Pure-Rust SquashFS packaging and runtime stub integration.
//! - [`portable`]: Portable Linux binary packaging.

pub mod appdir;
pub mod appimage;
pub mod portable;

pub use appdir::AppDirFilesystemLayout;
pub use appimage::{
    assemble_linux_package, AppImageContainerBuilder, AppImageOptions, LinuxPackageSpec,
    PackageOutputDescriptor, SquashfsBackend, SquashfsCompression, RUNTIME_AARCH64, RUNTIME_X86_64,
};

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use backhand::{FilesystemReader, InnerNode};
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    use std::path::Path;
    use std::str::FromStr;

    #[test]
    fn test_pure_rust_appimage_squashfs_contents_permissions_and_variants() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let app = AppImageContainerBuilder::new(
            temp_dir.path(),
            "TestApp".to_string(),
            "x86_64".to_string(),
        )?;

        app.configure_apprun_entrypoint()?;
        app.generate_desktop_file()?;

        let dummy_icon = temp_dir.path().join("icon.png");
        std::fs::write(&dummy_icon, b"\x89PNG\r\n\x1a\ndummy")?;
        app.install_icon(&dummy_icon)?;

        let dummy_bin = temp_dir.path().join("dummy_bin");
        std::fs::write(&dummy_bin, b"#!/bin/sh\necho hello")?;
        app.install_file(&dummy_bin, Path::new("TestApp"))?;

        let out = temp_dir.path().join("TestApp.AppImage");
        let options = AppImageOptions {
            backend: SquashfsBackend::from_str("pure-rust")?,
            compression: SquashfsCompression::from_str("gzip")?,
        };

        app.build_with_options(&out, None, &options)?;

        let file_bytes = std::fs::read(&out)?;
        assert_eq!(&file_bytes[0..4], &[0x7f, b'E', b'L', b'F']);

        let runtime_len = RUNTIME_X86_64.len();
        assert!(file_bytes.len() > runtime_len);
        assert_eq!(&file_bytes[runtime_len..runtime_len + 4], b"hsqs");

        let reader = BufReader::new(File::open(&out)?);
        let sqfs = FilesystemReader::from_reader_with_offset(reader, runtime_len as u64)?;

        let mut found_test_app_exec = false;
        let mut found_desktop = false;
        let mut found_apprun = false;
        let mut found_diricon = false;

        for node in sqfs.files() {
            let path_str = node.fullpath.to_string_lossy();
            if path_str.ends_with("TestApp") {
                assert_eq!(
                    node.header.permissions & 0o755,
                    0o755,
                    "Main binary inside SquashFS must have 0o755 permissions"
                );
                if let InnerNode::File(ref file_reader) = node.inner {
                    let mut buf = Vec::new();
                    sqfs.file(file_reader).reader().read_to_end(&mut buf)?;
                    assert_eq!(buf, b"#!/bin/sh\necho hello");
                }
                found_test_app_exec = true;
            } else if path_str.ends_with("TestApp.desktop") {
                found_desktop = true;
            } else if path_str.ends_with("AppRun") {
                found_apprun = true;
            } else if path_str.ends_with(".DirIcon") {
                found_diricon = true;
            }
        }

        assert!(found_test_app_exec, "SquashFS missing executable TestApp");
        assert!(found_desktop, "SquashFS missing TestApp.desktop");
        assert!(found_apprun, "SquashFS missing AppRun");
        assert!(found_diricon, "SquashFS missing .DirIcon");

        let arm_app = AppImageContainerBuilder::new(
            temp_dir.path(),
            "ArmApp".to_string(),
            "aarch64".to_string(),
        )?;
        arm_app.install_file(&dummy_bin, Path::new("ArmApp"))?;
        arm_app.configure_apprun_entrypoint()?;
        let arm_out = temp_dir.path().join("ArmApp.AppImage");
        arm_app.build_with_options(
            &arm_out,
            None,
            &AppImageOptions {
                backend: SquashfsBackend::PureRust,
                compression: SquashfsCompression::Gzip,
            },
        )?;
        let arm_bytes = std::fs::read(&arm_out)?;
        assert_eq!(
            &arm_bytes[RUNTIME_AARCH64.len()..RUNTIME_AARCH64.len() + 4],
            b"hsqs"
        );

        let arm_app_xz = AppImageContainerBuilder::new(
            temp_dir.path(),
            "ArmAppXz".to_string(),
            "aarch64".to_string(),
        )?;
        arm_app_xz.install_file(&dummy_bin, Path::new("ArmAppXz"))?;
        let arm_xz_out = temp_dir.path().join("ArmAppXz.AppImage");
        let pure_rust_xz_err = arm_app_xz.build_with_options(
            &arm_xz_out,
            None,
            &AppImageOptions {
                backend: SquashfsBackend::PureRust,
                compression: SquashfsCompression::Xz,
            },
        );
        assert!(
            pure_rust_xz_err.is_err(),
            "Pure-Rust backend must reject Xz with helpful guidance"
        );

        let bad_arch = AppImageContainerBuilder::new(
            temp_dir.path(),
            "BadApp".to_string(),
            "mips64".to_string(),
        )?;
        assert!(bad_arch.runtime_stub().is_err());

        Ok(())
    }
}
