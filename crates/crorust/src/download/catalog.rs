use crate::{TargetArch, TargetPlatform};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveKind {
    Zip,
    TarGz,
    TarZst,
    TarXz,
    RawBinary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCategory {
    Jdk,
    Gradle,
    Kotlinc,
    R8,
    PlatformTools,
    Ndk,
    Sysroot,
    AppImageRuntime,
    DesugarJdkLibs,
    DesugarJdkLibsConfig,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExtractionPolicy {
    pub skip_symlinks_on_non_linux: bool,

    pub skip_colons_on_windows: bool,
}

#[derive(Clone, Debug)]
pub struct ToolArtifact {
    pub category: ToolCategory,
    pub name: &'static str,
    pub version: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub archive_kind: ArchiveKind,
    pub strip_components: usize,
    pub binary_rel_paths: &'static [&'static str],
    pub extraction_policy: ExtractionPolicy,
}

impl ToolArtifact {
    pub fn resolved_url(&self, mirror_base: Option<&str>) -> String {
        if let Some(base) = mirror_base {
            let base_trimmed = base.trim_end_matches('/');
            if let Some((_, filename)) = self.url.rsplit_once('/') {
                format!("{base_trimmed}/{filename}")
            } else {
                self.url.to_string()
            }
        } else {
            self.url.to_string()
        }
    }

    pub fn archive_filename(&self) -> &'static str {
        if self.name.starts_with("Linux.sysroot-") {
            if self.name.contains("x86_64") {
                return "debian_bullseye_amd64_sysroot.tar.xz";
            } else if self.name.contains("aarch64") {
                return "debian_bullseye_arm64_sysroot.tar.xz";
            }
        }
        self.url
            .rsplit_once('/')
            .map(|(_, fname)| fname)
            .unwrap_or(self.name)
    }
}

pub struct ToolCatalog;

impl ToolCatalog {
    pub fn get_jdk(host_platform: TargetPlatform, host_arch: TargetArch) -> Option<ToolArtifact> {
        match (host_platform, host_arch) {
            (TargetPlatform::Linux, TargetArch::X64) => Some(ToolArtifact {
                category: ToolCategory::Jdk,
                name: "jdk",
                version: "17.0.10_7",
                url: "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.10+7/OpenJDK17U-jdk_x64_linux_hotspot_17.0.10_7.tar.gz",
                sha256: "a8fd07e1e97352e97e330beb20f1c6b351ba064ca7878e974c7d68b8a5c1b378",
                archive_kind: ArchiveKind::TarGz,
                strip_components: 1,
                binary_rel_paths: &["bin/java", "bin/javac"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            (TargetPlatform::Linux, TargetArch::Arm64) => Some(ToolArtifact {
                category: ToolCategory::Jdk,
                name: "jdk",
                version: "17.0.10_7",
                url: "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.10+7/OpenJDK17U-jdk_aarch64_linux_hotspot_17.0.10_7.tar.gz",
                sha256: "6e4201abfb3b020c1fb899b7ac063083c271250bf081f3aa7e63d91291a90b74",
                archive_kind: ArchiveKind::TarGz,
                strip_components: 1,
                binary_rel_paths: &["bin/java", "bin/javac"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            (TargetPlatform::Macos, TargetArch::Arm64) => Some(ToolArtifact {
                category: ToolCategory::Jdk,
                name: "jdk",
                version: "17.0.10_7",
                url: "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.10+7/OpenJDK17U-jdk_aarch64_mac_hotspot_17.0.10_7.tar.gz",
                sha256: "a6ec3b94f61695e8f445ee508411c56a2ce0cabc16ea4c4296ff062d13559d92",
                archive_kind: ArchiveKind::TarGz,
                strip_components: 3,
                binary_rel_paths: &["bin/java", "bin/javac"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            (TargetPlatform::Macos, TargetArch::X64) => Some(ToolArtifact {
                category: ToolCategory::Jdk,
                name: "jdk",
                version: "17.0.10_7",
                url: "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.10+7/OpenJDK17U-jdk_x64_mac_hotspot_17.0.10_7.tar.gz",
                sha256: "e16ee89d3304bb2ba706f9a7b0ba279725c2aea55d5468336f8de4bb859f300d",
                archive_kind: ArchiveKind::TarGz,
                strip_components: 3,
                binary_rel_paths: &["bin/java", "bin/javac"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            (TargetPlatform::Windows, TargetArch::X64) => Some(ToolArtifact {
                category: ToolCategory::Jdk,
                name: "jdk",
                version: "17.0.10_7",
                url: "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.10+7/OpenJDK17U-jdk_x64_windows_hotspot_17.0.10_7.zip",
                sha256: "fa963bc6fa54c3537c663d444c7bb4aa8ae9939cba8fadaf672311f352835060",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["bin/java.exe", "bin/javac.exe"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_gradle(version: &str) -> Option<ToolArtifact> {
        match version {
            "8.7" => Some(ToolArtifact {
                category: ToolCategory::Gradle,
                name: "gradle",
                version: "8.7",
                url: "https://services.gradle.org/distributions/gradle-8.7-bin.zip",
                sha256: "544c35d6bd849ae8a5ed0bcea39ba677dc40f49df7d1835561582da2009b961d",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["bin/gradle", "bin/gradle.bat"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            "8.8" => Some(ToolArtifact {
                category: ToolCategory::Gradle,
                name: "gradle",
                version: "8.8",
                url: "https://services.gradle.org/distributions/gradle-8.8-bin.zip",
                sha256: "a4b4158601f8636cdeeab09bd76afb640030bb5b144aafe261a5e8af027dc612",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["bin/gradle", "bin/gradle.bat"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_r8(version: &str) -> Option<ToolArtifact> {
        match version {
            "9.5.26" => Some(ToolArtifact {
                category: ToolCategory::R8,
                name: "r8",
                version: "9.5.26",
                url: "https://dl.google.com/dl/android/maven2/com/android/tools/r8/9.5.26/r8-9.5.26.jar",
                sha256: "3502f087388ce95bbf5254555927f09385dc2636873964d0bd53f3eaf4fb49ad",
                archive_kind: ArchiveKind::RawBinary,
                strip_components: 0,
                binary_rel_paths: &["r8.jar"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            "8.5.35" => Some(ToolArtifact {
                category: ToolCategory::R8,
                name: "r8",
                version: "8.5.35",
                url: "https://dl.google.com/dl/android/maven2/com/android/tools/r8/8.5.35/r8-8.5.35.jar",
                sha256: "4733945987ee0a840fafc34080b135259e01678412e07212b23f706334290294",
                archive_kind: ArchiveKind::RawBinary,
                strip_components: 0,
                binary_rel_paths: &["r8.jar"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_desugar_jdk_libs(version: &str) -> Option<ToolArtifact> {
        match version {
            "2.1.4" => Some(ToolArtifact {
                category: ToolCategory::DesugarJdkLibs,
                name: "desugar_jdk_libs",
                version: "2.1.4",
                url: "https://dl.google.com/android/maven2/com/android/tools/desugar_jdk_libs/2.1.4/desugar_jdk_libs-2.1.4.jar",
                sha256: "d7604feeef3a771d5d616b68fcc2c42f516243311ea286cfd41e1962829a6d28",
                archive_kind: ArchiveKind::RawBinary,
                strip_components: 0,
                binary_rel_paths: &["desugar_jdk_libs.jar"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_desugar_jdk_libs_config(version: &str) -> Option<ToolArtifact> {
        match version {
            "2.1.4" => Some(ToolArtifact {
                category: ToolCategory::DesugarJdkLibsConfig,
                name: "desugar_jdk_libs_configuration",
                version: "2.1.4",
                url: "https://dl.google.com/android/maven2/com/android/tools/desugar_jdk_libs_configuration/2.1.4/desugar_jdk_libs_configuration-2.1.4.jar",
                sha256: "5d770c1ba225c2cdf09086bf662954ae765ee481f901b02cd7b1dc3c8077c3f4",
                archive_kind: ArchiveKind::Zip,
                strip_components: 0,
                binary_rel_paths: &["META-INF/desugar/d8/desugar.json"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_kotlinc(version: &str) -> Option<ToolArtifact> {
        match version {
            "2.1.0" => Some(ToolArtifact {
                category: ToolCategory::Kotlinc,
                name: "kotlinc",
                version: "2.1.0",
                url: "https://github.com/JetBrains/kotlin/releases/download/v2.1.0/kotlin-compiler-2.1.0.zip",
                sha256: "b6698d5728ad8f9edcdd01617d638073191d8a03139cc538a391b4e3759ad297",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["bin/kotlinc", "bin/kotlinc.bat"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_platform_tools(host_platform: TargetPlatform) -> Option<ToolArtifact> {
        match host_platform {
            TargetPlatform::Linux => Some(ToolArtifact {
                category: ToolCategory::PlatformTools,
                name: "platform-tools",
                version: "35.0.0",
                url: "https://dl.google.com/android/repository/platform-tools_r35.0.0-linux.zip",
                sha256: "62fc977c1b7622ef8dbd6fe1312987d9b139aa8a0b06e88573c1b60129399d49",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["adb"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            TargetPlatform::Macos => Some(ToolArtifact {
                category: ToolCategory::PlatformTools,
                name: "platform-tools",
                version: "35.0.0",
                url: "https://dl.google.com/android/repository/platform-tools_r35.0.0-darwin.zip",
                sha256: "85c75ac31556dc95712cf1bdec592098e6c5067dc485356591d85932178bf8cd",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["adb"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            TargetPlatform::Windows => Some(ToolArtifact {
                category: ToolCategory::PlatformTools,
                name: "platform-tools",
                version: "35.0.0",
                url: "https://dl.google.com/android/repository/platform-tools_r35.0.0-windows.zip",
                sha256: "7ab78a8f8b305ae4d0de647d99c43599744de61a0838d3a47bda0cdffefee87e",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["adb.exe"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_android_platform(api_level: u32) -> Option<ToolArtifact> {
        match api_level {
            34 => Some(ToolArtifact {
                category: ToolCategory::PlatformTools,
                name: "platforms/android-34",
                version: "34",
                url: "https://dl.google.com/android/repository/platform-34-ext7_r03.zip",
                sha256: "16fdb74c55e59ae3ef52def135aec713508467bd56d7dabcd8c9be31fa8b20f3",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["android.jar"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            35 => Some(ToolArtifact {
                category: ToolCategory::PlatformTools,
                name: "platforms/android-35",
                version: "35",
                url: "https://dl.google.com/android/repository/platform-35_r02.zip",
                sha256: "0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0",
                archive_kind: ArchiveKind::Zip,
                strip_components: 1,
                binary_rel_paths: &["android.jar"],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn get_ndk() -> ToolArtifact {
        let (platform_slug, sha1_sum) =
            match TargetPlatform::detect_host().unwrap_or(TargetPlatform::Linux) {
                TargetPlatform::Linux => ("linux", "fcdad75a765a46a9cf6560353f480db251d14765"),
                TargetPlatform::Macos => ("darwin", "cebe7af9bdadc67b8f919d549e9ab269831fd2c9"),
                TargetPlatform::Windows => ("windows", "c7ea35ffe916082876611da1a6d5618d15430c29"),
                _ => ("linux", "fcdad75a765a46a9cf6560353f480db251d14765"),
            };

        ToolArtifact {
            category: ToolCategory::Ndk,
            name: "Android.ndk",
            version: "26.3.11579264",
            url: match platform_slug {
                "linux" => "https://dl.google.com/android/repository/android-ndk-r26d-linux.zip",
                "darwin" => "https://dl.google.com/android/repository/android-ndk-r26d-darwin.zip",
                "windows" => {
                    "https://dl.google.com/android/repository/android-ndk-r26d-windows.zip"
                }
                _ => "https://dl.google.com/android/repository/android-ndk-r26d-linux.zip",
            },
            sha256: sha1_sum,
            archive_kind: ArchiveKind::Zip,
            strip_components: 1,
            binary_rel_paths: &[],
            extraction_policy: ExtractionPolicy::default(),
        }
    }

    pub fn get_sysroot(name: &str) -> Option<ToolArtifact> {
        match name {
            "iPhoneSimulator.sdk" => Some(ToolArtifact {
                category: ToolCategory::Sysroot,
                name: "iPhoneSimulator.sdk",
                version: "16.5",
                url: "",
                sha256: "",
                archive_kind: ArchiveKind::TarXz,
                strip_components: 1,
                binary_rel_paths: &[],
                extraction_policy: ExtractionPolicy {
                    skip_symlinks_on_non_linux: false,
                    skip_colons_on_windows: true,
                },
            }),
            "iPhoneOS.sdk" => Some(ToolArtifact {
                category: ToolCategory::Sysroot,
                name: "iPhoneOS.sdk",
                version: "16.5",
                url: "https://github.com/theos/sdks/releases/download/master-146e41f/iPhoneOS16.5.sdk.tar.xz",
                sha256: "5e0fd3f01266cce4ce012d4a99b38eb56578fca40d09edc81cd83dee958202fb",
                archive_kind: ArchiveKind::TarXz,
                strip_components: 1,
                binary_rel_paths: &[],
                extraction_policy: ExtractionPolicy {
                    skip_symlinks_on_non_linux: false,
                    skip_colons_on_windows: true,
                },
            }),
            "MacOSX.sdk" => None,
            "Windows.sdk" => None,
            "Linux.sysroot-x86_64" => Some(ToolArtifact {
                category: ToolCategory::Sysroot,
                name: "Linux.sysroot-x86_64",
                version: "2.31",
                url: "https://commondatastorage.googleapis.com/chrome-linux-sysroot/52d61d4446ffebfaa3dda2cd02da4ab4876ff237853f46d273e7f9b666652e1d",
                sha256: "52d61d4446ffebfaa3dda2cd02da4ab4876ff237853f46d273e7f9b666652e1d",
                archive_kind: ArchiveKind::TarXz,
                strip_components: 0,
                binary_rel_paths: &[],
                extraction_policy: ExtractionPolicy::default(),
            }),
            "Linux.sysroot-aarch64" => Some(ToolArtifact {
                category: ToolCategory::Sysroot,
                name: "Linux.sysroot-aarch64",
                version: "2.31",
                url: "https://commondatastorage.googleapis.com/chrome-linux-sysroot/c7176a4c7aacbf46bda58a029f39f79a68008d3dee6518f154dcf5161a5486d8",
                sha256: "c7176a4c7aacbf46bda58a029f39f79a68008d3dee6518f154dcf5161a5486d8",
                archive_kind: ArchiveKind::TarXz,
                strip_components: 0,
                binary_rel_paths: &[],
                extraction_policy: ExtractionPolicy::default(),
            }),
            _ => None,
        }
    }

    pub fn all_known_artifacts() -> Vec<ToolArtifact> {
        let mut list = Vec::new();
        for (p, a) in [
            (TargetPlatform::Linux, TargetArch::X64),
            (TargetPlatform::Linux, TargetArch::Arm64),
            (TargetPlatform::Macos, TargetArch::Arm64),
            (TargetPlatform::Macos, TargetArch::X64),
            (TargetPlatform::Windows, TargetArch::X64),
        ] {
            if let Some(jdk) = Self::get_jdk(p, a) {
                list.push(jdk);
            }
        }
        for v in ["8.7", "8.8"] {
            if let Some(g) = Self::get_gradle(v) {
                list.push(g);
            }
        }
        for v in ["9.5.26", "8.5.35"] {
            if let Some(r8) = Self::get_r8(v) {
                list.push(r8);
            }
        }
        if let Some(k) = Self::get_kotlinc("2.1.0") {
            list.push(k);
        }
        for p in [
            TargetPlatform::Linux,
            TargetPlatform::Macos,
            TargetPlatform::Windows,
        ] {
            if let Some(pt) = Self::get_platform_tools(p) {
                list.push(pt);
            }
        }
        if let Some(desugar_libs) = Self::get_desugar_jdk_libs("2.1.4") {
            list.push(desugar_libs);
        }
        if let Some(desugar_cfg) = Self::get_desugar_jdk_libs_config("2.1.4") {
            list.push(desugar_cfg);
        }
        list.push(Self::get_ndk());
        for api in [34, 35] {
            if let Some(platform) = Self::get_android_platform(api) {
                list.push(platform);
            }
        }
        for s in [
            "iPhoneSimulator.sdk",
            "iPhoneOS.sdk",
            "MacOSX.sdk",
            "Windows.sdk",
            "Linux.sysroot-x86_64",
            "Linux.sysroot-aarch64",
        ] {
            if let Some(sys) = Self::get_sysroot(s) {
                list.push(sys);
            }
        }
        list
    }

    pub fn expected_sha256_for_filename(filename: &str) -> Option<&'static str> {
        if filename == "platform-35_r01.zip" {
            return Some("ec20a0a65704e1b2554f8e3eebf588ef260569673c848c01aff2e29c28734cf4");
        }
        for artifact in Self::all_known_artifacts() {
            if artifact.archive_filename() == filename && !artifact.sha256.is_empty() {
                return Some(artifact.sha256);
            }
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NdkVersionInfo {
    pub revision: Option<String>,
    pub release_name: Option<String>,
}

pub fn read_ndk_version(ndk_root: &Path) -> Option<NdkVersionInfo> {
    let props_path = ndk_root.join("source.properties");
    let content = std::fs::read_to_string(props_path).ok()?;
    let mut revision = None;
    let mut release_name = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(rev) = line.strip_prefix("Pkg.Revision") {
            let rev = rev.trim_start_matches([' ', '=']);
            revision = Some(rev.trim().to_string());
        } else if let Some(rel) = line.strip_prefix("Pkg.ReleaseName") {
            let rel = rel.trim_start_matches([' ', '=']);
            release_name = Some(rel.trim().to_string());
        }
    }
    if revision.is_some() || release_name.is_some() {
        Some(NdkVersionInfo {
            revision,
            release_name,
        })
    } else {
        None
    }
}

pub fn is_ndk_version_compatible(ndk_root: &Path, expected_version: Option<&str>) -> bool {
    let Some(expected) = expected_version else {
        return true;
    };
    if let Some(info) = read_ndk_version(ndk_root) {
        if let Some(rev) = &info.revision {
            if rev == expected || rev.starts_with(expected) || expected.starts_with(rev) {
                return true;
            }
        }
        if let Some(rel) = &info.release_name {
            if rel.eq_ignore_ascii_case(expected)
                || rel
                    .trim_start_matches('r')
                    .eq_ignore_ascii_case(expected.trim_start_matches('r'))
            {
                return true;
            }
        }
    }
    false
}

pub fn read_jdk_version(jdk_root: &Path) -> Option<String> {
    let release_path = jdk_root.join("release");
    let content = std::fs::read_to_string(release_path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(ver) = line.strip_prefix("JAVA_VERSION=") {
            let ver = ver.trim_matches(|c| c == '"' || c == '\'');
            return Some(ver.trim().to_string());
        }
    }
    None
}

pub fn read_platform_tools_version(pt_root: &Path) -> Option<String> {
    let props_path = pt_root.join("source.properties");
    let content = std::fs::read_to_string(props_path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(rev) = line.strip_prefix("Pkg.Revision") {
            let rev = rev.trim_start_matches([' ', '=']);
            return Some(rev.trim().to_string());
        }
    }
    None
}

pub fn is_version_compatible(actual: Option<&str>, expected: Option<&str>) -> bool {
    match (actual, expected) {
        (_, None) => true,
        (Some(act), Some(exp)) => act == exp || act.starts_with(exp) || exp.starts_with(act),
        (None, Some(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_platform_catalog_entries() {
        let p34 = ToolCatalog::get_android_platform(34).expect("Android 34 must be in catalog");
        assert_eq!(p34.archive_filename(), "platform-34-ext7_r03.zip");
        assert_eq!(
            p34.sha256,
            "16fdb74c55e59ae3ef52def135aec713508467bd56d7dabcd8c9be31fa8b20f3"
        );
        assert_eq!(p34.strip_components, 1);
        assert!(p34.binary_rel_paths.contains(&"android.jar"));

        let p35 = ToolCatalog::get_android_platform(35).expect("Android 35 must be in catalog");
        assert_eq!(p35.archive_filename(), "platform-35_r02.zip");
        assert_eq!(
            p35.sha256,
            "0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0"
        );
        assert_eq!(p35.strip_components, 1);
        assert!(p35.binary_rel_paths.contains(&"android.jar"));

        assert!(ToolCatalog::get_android_platform(99).is_none());
    }

    #[test]
    fn test_expected_sha256_for_filename() {
        assert_eq!(
            ToolCatalog::expected_sha256_for_filename("platform-35_r02.zip"),
            Some("0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0")
        );
        assert_eq!(
            ToolCatalog::expected_sha256_for_filename("platform-35_r01.zip"),
            Some("ec20a0a65704e1b2554f8e3eebf588ef260569673c848c01aff2e29c28734cf4")
        );
        assert_eq!(
            ToolCatalog::expected_sha256_for_filename("platform-34-ext7_r03.zip"),
            Some("16fdb74c55e59ae3ef52def135aec713508467bd56d7dabcd8c9be31fa8b20f3")
        );
    }
}
