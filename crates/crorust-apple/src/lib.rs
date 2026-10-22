//! # crorust-apple: Pure-Rust macOS & iOS Packaging and Codesigning Engine
//!
//! Assembles application bundles (`.app`), converts ICNS icons, signs Mach-O binaries,
//! submits packages for Apple Notarization, and builds DMGs/IPAs without Xcode.
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Assembly["Bundle Assembly"]
//!         Exe[Mach-O 64-bit Binary] --> Dir[AppBundleDirector]
//!         Icon[PNG Icon] --> ICNS[ICNS Converter]
//!         Plist[DarwinPropertyList] --> XML[Info.plist]
//!         ICNS & XML --> Dir
//!     end
//!
//!     subgraph Signing["Cryptographic Signing"]
//!         Dir --> CS[apple-codesign]
//!         Key[Private Key & Cert] --> CS
//!     end
//!
//!     subgraph Distribution["Distribution Artifacts"]
//!         CS --> DMG[apple-dmg: .dmg]
//!         CS --> IPA[create_ipa: .ipa]
//!         DMG --> Notary[Notarization REST API]
//!     end
//! ```
//!
//! ## Core Modules
//! - [`bundle`]: Application directory tree construction and icon generation.
//! - [`codesign`]: Pure-Rust Mach-O codesigning and Notarization REST client.
//! - [`dmg`]: Apple disk image creation via `apple-dmg`.
//! - [`ipa`]: iOS application package ZIP container generator.
//! - [`plist`]: XML property list (`Info.plist`) serializer.

pub mod bundle;
pub mod codesign;
pub mod dmg;
pub mod ipa;
pub mod plist;

pub use bundle::{
    assemble_darwin_package, AppBundleDirector, AppleBundleContainer, DarwinPackageSpec,
    DarwinPlatform, PackageOutputDescriptor, IOS_ICON_SIZES, MACOS_ICON_SIZES,
};
pub use codesign::{
    extract_bundle_identifier_from_package, prepare_signing_key_and_cert,
    submit_artifact_for_apple_notarization, NOTARIZE_TIMEOUT,
};
pub use dmg::sign_disk_image;
pub use ipa::create_ipa;
pub use plist::DarwinPropertyList;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use image::{ImageBuffer, Rgba};
    use std::path::Path;

    fn extract_file_name_str(path: &Path) -> Result<&str> {
        let name = path.file_name().ok_or_else(|| {
            anyhow::anyhow!("Path has no valid filename component: '{}'", path.display())
        })?;
        name.to_str()
            .ok_or_else(|| anyhow::anyhow!("Filename is not valid UTF-8: '{}'", path.display()))
    }

    #[test]
    fn test_extract_file_name_and_executable_validation() {
        assert_eq!(
            extract_file_name_str(Path::new("target/debug/my_app")).unwrap(),
            "my_app"
        );

        for invalid in ["", "/", ".", "path/to/.."] {
            let p = Path::new(invalid);
            let is_err = p.file_name().is_none() || p.file_name().unwrap().is_empty();
            assert!(is_err, "Expected invalid path for `{invalid}`");
        }

        let temp = tempfile::tempdir().unwrap();
        let info = DarwinPropertyList {
            bundle_name: Some("TestApp".to_string()),
            ..Default::default()
        };
        let mut bundle = AppleBundleContainer::new(temp.path(), info).unwrap();
        assert!(bundle.stage_executable_binary(Path::new("")).is_err());
    }

    #[test]
    fn test_macos_and_ios_appbundle_icons_plist_roundtrip_and_bundle_id() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let dummy_exe = temp.path().join("my_app");
        std::fs::write(&dummy_exe, b"Mach-O 64-bit executable")?;

        let dummy_dylib = temp.path().join("libhelper.dylib");
        std::fs::write(&dummy_dylib, b"Mach-O dylib")?;

        let icon_path = temp.path().join("icon.png");
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(512, 512, Rgba([255, 100, 50, 255]));
        img.save(&icon_path)?;

        let mac_info = DarwinPropertyList {
            bundle_name: Some("MyApp".to_string()),
            bundle_identifier: Some("com.example.myapp".to_string()),
            bundle_version: Some("1.2.3".to_string()),
            ..Default::default()
        };

        let mut mac_bundle = AppleBundleContainer::new(temp.path(), mac_info)?;
        mac_bundle.stage_executable_binary(&dummy_exe)?;
        mac_bundle.attach_application_icon(&icon_path)?;
        mac_bundle.stage_library(&dummy_dylib)?;
        mac_bundle.finalize_bundle(None)?;

        let mac_appdir = mac_bundle.bundle_appdir();
        assert!(mac_appdir.join("Contents/MacOS/my_app").is_file());
        assert!(mac_appdir.join("Contents/Resources/AppIcon.icns").is_file());
        assert!(mac_appdir
            .join("Contents/Frameworks/libhelper.dylib")
            .is_file());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(mac_appdir.join("Contents/MacOS/my_app"))?
                .permissions()
                .mode();
            assert_ne!(mode & 0o111, 0, "macOS executable must have +x permission");
        }

        let plist_path = mac_appdir.join("Contents/Info.plist");
        let roundtrip_info = DarwinPropertyList::read_from_file(&plist_path)?;
        assert_eq!(roundtrip_info.bundle_name.as_deref(), Some("MyApp"));
        assert_eq!(
            roundtrip_info.bundle_identifier.as_deref(),
            Some("com.example.myapp")
        );
        assert_eq!(roundtrip_info.executable_name.as_deref(), Some("my_app"));
        assert_eq!(roundtrip_info.icon_file.as_deref(), Some("AppIcon"));
        assert_eq!(
            extract_bundle_identifier_from_package(mac_appdir)?,
            "com.example.myapp"
        );

        let ios_info = DarwinPropertyList {
            bundle_name: Some("MyIosApp".to_string()),
            bundle_identifier: Some("com.example.myiosapp".to_string()),
            requires_ios: Some(true),
            ..Default::default()
        };

        let mut ios_bundle = AppleBundleContainer::open_or_new(temp.path(), ios_info)?;
        ios_bundle.stage_executable_binary(&dummy_exe)?;
        ios_bundle.register_existing_icon();
        ios_bundle.finalize_bundle(None)?;

        let ios_appdir = ios_bundle.bundle_appdir();
        assert!(ios_appdir.join("Info.plist").is_file());
        assert!(ios_appdir.join("my_app").is_file());
        assert_eq!(
            extract_bundle_identifier_from_package(ios_appdir)?,
            "com.example.myiosapp"
        );

        let ios_roundtrip = DarwinPropertyList::read_from_file(&ios_appdir.join("Info.plist"))?;
        assert_eq!(ios_roundtrip.icon_files.len(), IOS_ICON_SIZES.len());

        let ipa_path = temp.path().join("test.ipa");
        create_ipa(ios_appdir, &ipa_path, "MyIosApp")?;
        assert!(ipa_path.is_file());

        Ok(())
    }
}
