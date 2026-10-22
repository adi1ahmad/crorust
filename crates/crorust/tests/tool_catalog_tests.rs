use crorust::download::catalog::{ArchiveKind, ToolCatalog, ToolCategory};
use crorust::{TargetArch, TargetPlatform};

#[test]
fn test_all_jdk_catalog_entries_have_valid_metadata() {
    let host_matrix = [
        (TargetPlatform::Linux, TargetArch::X64),
        (TargetPlatform::Linux, TargetArch::Arm64),
        (TargetPlatform::Macos, TargetArch::Arm64),
        (TargetPlatform::Macos, TargetArch::X64),
        (TargetPlatform::Windows, TargetArch::X64),
    ];

    for (platform, arch) in host_matrix {
        let jdk = ToolCatalog::get_jdk(platform, arch)
            .unwrap_or_else(|| panic!("JDK missing for {platform:?}-{arch:?}"));

        assert_eq!(jdk.category, ToolCategory::Jdk);
        assert!(jdk.url.starts_with("https://"));
        assert_eq!(jdk.sha256.len(), 64, "SHA-256 must be 64-char hex string");
        assert!(jdk.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!jdk.binary_rel_paths.is_empty());
        assert!(jdk.binary_rel_paths[0].starts_with("bin/"));
    }
}

#[test]
fn test_gradle_catalog_versions() {
    for version in &["8.7", "8.8"] {
        let gradle =
            ToolCatalog::get_gradle(version).unwrap_or_else(|| panic!("Gradle {version} missing"));

        assert_eq!(gradle.category, ToolCategory::Gradle);
        assert_eq!(gradle.version, *version);
        assert_eq!(gradle.archive_kind, ArchiveKind::Zip);
        assert_eq!(gradle.strip_components, 1);
        assert_eq!(gradle.sha256.len(), 64);
    }
}

#[test]
fn test_platform_tools_catalog() {
    for platform in &[
        TargetPlatform::Linux,
        TargetPlatform::Macos,
        TargetPlatform::Windows,
    ] {
        let pt = ToolCatalog::get_platform_tools(*platform)
            .unwrap_or_else(|| panic!("platform-tools missing for {platform:?}"));

        assert_eq!(pt.category, ToolCategory::PlatformTools);
        assert_eq!(pt.archive_kind, ArchiveKind::Zip);
        assert_eq!(pt.strip_components, 1);
        assert_eq!(pt.sha256.len(), 64);
    }
}

#[test]
fn test_sysroot_catalog_entries() {
    let sysroots = [
        "iPhoneSimulator.sdk",
        "iPhoneOS.sdk",
        "Linux.sysroot-x86_64",
        "Linux.sysroot-aarch64",
    ];

    for name in sysroots {
        let sysroot = ToolCatalog::get_sysroot(name)
            .unwrap_or_else(|| panic!("Sysroot {name} missing from catalog"));

        assert_eq!(sysroot.category, ToolCategory::Sysroot);
        assert_eq!(sysroot.archive_kind, ArchiveKind::TarXz);
        if name == "iPhoneOS.sdk" {
            assert_eq!(sysroot.strip_components, 1);
            assert_eq!(sysroot.sha256.len(), 64);
            assert!(sysroot.url.contains("theos/sdks"));
        } else if name == "iPhoneSimulator.sdk" {
            assert_eq!(sysroot.strip_components, 1);
        } else {
            assert_eq!(sysroot.strip_components, 0);
            assert_eq!(sysroot.sha256.len(), 64);
            assert!(sysroot.url.contains("commondatastorage.googleapis.com"));
        }
    }

    assert!(ToolCatalog::get_sysroot("Windows.sdk").is_none());
    assert!(ToolCatalog::get_sysroot("MacOSX.sdk").is_none());

    let ndk = ToolCatalog::get_ndk();
    assert_eq!(ndk.category, ToolCategory::Ndk);
    assert_eq!(ndk.archive_kind, ArchiveKind::Zip);
    assert_eq!(ndk.strip_components, 1);
    assert!(ndk
        .url
        .starts_with("https://dl.google.com/android/repository/android-ndk-r26d-"));
}

/// Catches entries that can never download (blank url / sha256). The iOS
/// Simulator SDK is the one deliberate exception: the host provides it, and
/// `crorust i --fix` skips entries without a URL instead of failing.
#[test]
fn test_sysroot_and_ndk_entries_are_downloadable_or_explicitly_host_provided() {
    let mut entries = vec![ToolCatalog::get_ndk()];
    for name in [
        "iPhoneSimulator.sdk",
        "iPhoneOS.sdk",
        "Linux.sysroot-x86_64",
        "Linux.sysroot-aarch64",
    ] {
        if let Some(a) = ToolCatalog::get_sysroot(name) {
            entries.push(a);
        }
    }

    for a in entries {
        if a.url.is_empty() {
            assert_eq!(a.name, "iPhoneSimulator.sdk", "{} has no URL", a.name);
            continue;
        }
        assert!(a.url.starts_with("https://"), "{}: {}", a.name, a.url);
        assert!(
            a.sha256.len() == 40 || a.sha256.len() == 64,
            "{}: sha checksum must be 40 (SHA-1) or 64 (SHA-256) hex chars",
            a.name
        );
        assert!(
            a.sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "{}",
            a.name
        );
    }
}
