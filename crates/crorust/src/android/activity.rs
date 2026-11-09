use std::path::Path;

/// Detects the resolved version of `android-activity` from `Cargo.lock` or `Cargo.toml`.
pub fn detect_android_activity_version(
    package_root: &Path,
    workspace_root: Option<&Path>,
) -> Option<String> {
    // 1. Check workspace or package Cargo.lock (exact resolved version)
    let candidates = [
        workspace_root.map(|w| w.join("Cargo.lock")),
        Some(package_root.join("Cargo.lock")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.is_file() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if let Some(ver) = parse_android_activity_from_lock(&content) {
                    return Some(ver);
                }
            }
        }
    }

    // 2. Fallback: check Cargo.toml if Cargo.lock does not exist
    let manifest_path = package_root.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Some(ver) = parse_android_activity_from_manifest(&content) {
            return Some(ver);
        }
    }

    None
}

/// Parses `android-activity` package version from Cargo.lock content
pub fn parse_android_activity_from_lock(content: &str) -> Option<String> {
    let mut in_android_activity = false;
    let mut recorded_version: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            if in_android_activity {
                if let Some(ver) = recorded_version {
                    return Some(ver);
                }
            }
            in_android_activity = false;
            recorded_version = None;
        } else if trimmed == "name = \"android-activity\"" {
            in_android_activity = true;
            if let Some(ver) = recorded_version {
                return Some(ver);
            }
        } else if let Some(rest) = trimmed.strip_prefix("version = \"") {
            if let Some(end) = rest.find('"') {
                let ver = rest[..end].to_string();
                if in_android_activity {
                    return Some(ver);
                }
                recorded_version = Some(ver);
            }
        }
    }

    if in_android_activity {
        return recorded_version;
    }

    None
}

/// Helper to extract declared version for a given dependency crate name from Cargo.toml
fn parse_crate_version_from_manifest(content: &str, crate_name: &str) -> Option<String> {
    let mut in_target_section = false;
    let section_suffix = format!("dependencies.{crate_name}");

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        // Section header check
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_target_section = trimmed.contains(&section_suffix);
            continue;
        }

        if in_target_section {
            if let Some(idx) = trimmed.find("version = \"") {
                let rest = &trimmed[idx + 11..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
        }

        // Inline dependency check with strict boundary: `crate_name` must be followed by space, `=`, or `{`
        if let Some(rest) = trimmed.strip_prefix(crate_name) {
            if rest.starts_with(|c: char| c.is_whitespace() || c == '=' || c == '{') {
                if let Some(idx) = rest.find("version = \"") {
                    let v_rest = &rest[idx + 11..];
                    if let Some(end) = v_rest.find('"') {
                        return Some(v_rest[..end].to_string());
                    }
                } else if let Some(idx) = rest.find("= \"") {
                    let v_rest = &rest[idx + 3..];
                    if let Some(end) = v_rest.find('"') {
                        return Some(v_rest[..end].to_string());
                    }
                }
            }
        }
    }
    None
}

/// Fallback parser for Cargo.toml when Cargo.lock is not yet created.
/// Inspects direct `android-activity` dependency, and if absent, inspects `winit` to infer the
/// underlying `android-activity` version.
pub fn parse_android_activity_from_manifest(content: &str) -> Option<String> {
    // 1. Direct android-activity dependency check
    if let Some(ver) = parse_crate_version_from_manifest(content, "android-activity") {
        return Some(ver);
    }

    // 2. Transitive winit dependency check
    if let Some(winit_ver) = parse_crate_version_from_manifest(content, "winit") {
        let clean = winit_ver.trim_start_matches(|c: char| !c.is_ascii_digit());
        let parts: Vec<&str> = clean.split('.').collect();
        let major: u32 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor: u32 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);

        if major > 0 || (major == 0 && minor >= 30) {
            // winit 0.30+ (and future 1.0+) uses android-activity >= 0.6.1
            return Some("0.6.1".to_string());
        } else if major == 0 && minor == 29 {
            // winit 0.29 uses android-activity 0.5.x
            return Some("0.5.2".to_string());
        }
    }

    None
}

/// Maps an `android-activity` crate version to the compatible Google `games-activity` AAR release.
/// - `android-activity 0.5.x` and `0.6.0` -> `2.0.2` (uses 2-arg `onTouchEventNative`)
/// - `android-activity >= 0.6.1`          -> `4.4.2` (uses 17-arg `onTouchEventNative`)
/// - Default when unspecified (None)     -> `4.4.2` (latest modern stable Google Game SDK release)
pub fn resolve_recommended_games_activity_version(
    android_activity_ver: Option<&str>,
) -> &'static str {
    let raw_ver = match android_activity_ver {
        Some(v) => v.trim(),
        None => return "4.4.2",
    };

    let is_exact = raw_ver.starts_with('=');
    let has_caret = raw_ver.starts_with('^');
    let clean = raw_ver.trim_start_matches(|c: char| !c.is_ascii_digit());

    let parts: Vec<&str> = clean.split('.').collect();
    let major: u32 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);

    if major == 0 {
        if minor <= 5 {
            "2.0.2"
        } else if minor == 6 {
            // In Cargo SemVer, "0.6" or "^0.6.0" resolves to latest 0.6.x (0.6.4+), which uses 4.4.2.
            // Only an exact pin to 0.6.0 ("=0.6.0") or explicit literal "0.6.0" (without ^) uses 2.0.2.
            if parts.len() < 3 {
                "4.4.2"
            } else {
                let patch: u32 = parts
                    .get(2)
                    .and_then(|p| {
                        let numeric: String =
                            p.chars().take_while(|c| c.is_ascii_digit()).collect();
                        numeric.parse().ok()
                    })
                    .unwrap_or(0);

                if patch == 0 && (is_exact || !has_caret) {
                    "2.0.2"
                } else {
                    "4.4.2"
                }
            }
        } else {
            "4.4.2"
        }
    } else {
        "4.4.2"
    }
}
