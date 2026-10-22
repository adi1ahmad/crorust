//! Host Xcode and Apple SDK discovery routines.
//! Searches system paths, xcrun, and environment variables before attempting remote downloads.

use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;

pub struct AppleSdkDiscovery;

impl AppleSdkDiscovery {
    /// Discovers an existing iPhoneOS (device) SDK on the host system.
    pub fn locate_ios_device_sdk() -> Option<PathBuf> {
        // 1. Environment variable override
        if let Ok(path) =
            std::env::var("CRORUST_IOS_SDK").or_else(|_| std::env::var("IPHONEOS_SDK_PATH"))
        {
            let p = PathBuf::from(path);
            if Self::is_valid_apple_sdk(&p) {
                return Some(dunce::simplified(&p).to_path_buf());
            }
        }

        // 2. macOS native xcrun discovery
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("xcrun")
                .args(["--sdk", "iphoneos", "--show-sdk-path"])
                .output()
            {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let p = PathBuf::from(path_str);
                    if Self::is_valid_apple_sdk(&p) {
                        return Some(dunce::simplified(&p).to_path_buf());
                    }
                }
            }
        }

        // 3. Common filesystem paths (macOS or cross-environment mounts)
        let candidates = [
            "/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk",
            "/Applications/Xcode-beta.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk",
            "/opt/theos/sdks/iPhoneOS.sdk",
        ];

        for candidate in candidates {
            let p = PathBuf::from(candidate);
            if Self::is_valid_apple_sdk(&p) {
                return Some(dunce::simplified(&p).to_path_buf());
            }
        }

        None
    }

    /// Discovers an existing iPhoneSimulator SDK on the host system.
    pub fn locate_ios_simulator_sdk() -> Option<PathBuf> {
        // 1. Environment variable override
        if let Ok(path) = std::env::var("CRORUST_IOS_SIMULATOR_SDK")
            .or_else(|_| std::env::var("IPHONESIMULATOR_SDK_PATH"))
        {
            let p = PathBuf::from(path);
            if Self::is_valid_apple_sdk(&p) {
                return Some(dunce::simplified(&p).to_path_buf());
            }
        }

        // 2. macOS native xcrun discovery
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("xcrun")
                .args(["--sdk", "iphonesimulator", "--show-sdk-path"])
                .output()
            {
                if output.status.success() {
                    let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let p = PathBuf::from(path_str);
                    if Self::is_valid_apple_sdk(&p) {
                        return Some(dunce::simplified(&p).to_path_buf());
                    }
                }
            }
        }

        // 3. Common filesystem paths
        let candidates = [
            "/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk",
            "/Applications/Xcode-beta.app/Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk",
        ];

        for candidate in candidates {
            let p = PathBuf::from(candidate);
            if Self::is_valid_apple_sdk(&p) {
                return Some(dunce::simplified(&p).to_path_buf());
            }
        }

        None
    }

    pub fn is_valid_apple_sdk(path: &Path) -> bool {
        path.is_dir()
            && (path.join("System/Library/Frameworks").exists()
                || path.join("usr/lib").exists()
                || path.join("SDKSettings.plist").exists()
                || path.join("SDKSettings.json").exists())
    }
}
