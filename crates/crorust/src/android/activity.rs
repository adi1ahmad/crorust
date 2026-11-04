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

/// Fallback parser for Cargo.toml when Cargo.lock is not yet created
pub fn parse_android_activity_from_manifest(content: &str) -> Option<String> {
    let mut in_android_activity_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        // Section header check
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_android_activity_section = trimmed.contains("dependencies.android-activity");
            continue;
        }

        if in_android_activity_section {
            if let Some(idx) = trimmed.find("version = \"") {
                let rest = &trimmed[idx + 11..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
        }

        // Inline dependency check with strict boundary: `android-activity` must be followed by space, `=`, or `{`
        if let Some(rest) = trimmed.strip_prefix("android-activity") {
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

/// Maps an `android-activity` crate version to the compatible Google `games-activity` AAR release.
/// - `android-activity 0.5.x` / `0.6.x` -> `2.0.2` (uses 2-arg `onTouchEventNative`)
/// - `android-activity >= 0.7.x`       -> `3.0.5` (uses 17-arg `onTouchEventNative`)
/// - Default when unspecified (None)   -> `3.0.5` (latest modern Google Game SDK release)
pub fn resolve_recommended_games_activity_version(
    android_activity_ver: Option<&str>,
) -> &'static str {
    let ver_str = match android_activity_ver {
        Some(v) => v.trim_start_matches(|c: char| !c.is_ascii_digit()),
        None => return "3.0.5",
    };

    let parts: Vec<&str> = ver_str.split('.').collect();
    let major: u32 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);

    if major == 0 && (minor == 5 || minor == 6) {
        "2.0.2"
    } else {
        "3.0.5"
    }
}
