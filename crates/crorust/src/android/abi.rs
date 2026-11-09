use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// JNI method signature variant for GameActivity's onTouchEventNative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameActivityJniAbi {
    /// 2-argument signature used in Google AGDK 1.x / 2.x:
    /// `(JLandroid/view/MotionEvent;)Z`
    TwoArg,
    /// 17-argument signature used in Google AGDK 3.x / 4.x:
    /// `(JLandroid/view/MotionEvent;IIIIIJJIIIIIIFF)Z`
    SeventeenArg,
}

pub const SIG_17_ARG: &[u8] = b"(JLandroid/view/MotionEvent;IIIIIJJIIIIIIFF)Z";
pub const SIG_2_ARG: &[u8] = b"(JLandroid/view/MotionEvent;)Z";

/// Checks if a byte slice contains a target needle by looking for the first byte
/// and comparing slices, avoiding quadratic sliding window iterator overhead.
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    let first = needle[0];
    let mut offset = 0;
    while let Some(pos) = haystack[offset..].iter().position(|&b| b == first) {
        let start = offset + pos;
        if start + needle.len() <= haystack.len() {
            if &haystack[start..start + needle.len()] == needle {
                return true;
            }
            offset = start + 1;
        } else {
            break;
        }
    }
    false
}

/// Scans an I/O stream in 64KB chunks for both 17-arg and 2-arg GameActivity JNI signatures
/// in a single streaming pass, bounding memory allocation to 64KB and short-circuiting on discovery.
fn stream_detect_game_activity_abi<R: Read>(
    mut reader: R,
) -> std::io::Result<Option<GameActivityJniAbi>> {
    const CHUNK_SIZE: usize = 64 * 1024;
    let max_needle_len = SIG_17_ARG.len().max(SIG_2_ARG.len());
    let mut buffer = vec![0u8; CHUNK_SIZE + max_needle_len - 1];
    let mut carryover = 0;

    loop {
        let bytes_read = reader.read(&mut buffer[carryover..carryover + CHUNK_SIZE])?;
        if bytes_read == 0 {
            break;
        }

        let total_valid = carryover + bytes_read;
        let window_slice = &buffer[..total_valid];

        // Check 17-arg first (modern AGDK 3.x / 4.x)
        if contains_subslice(window_slice, SIG_17_ARG) {
            return Ok(Some(GameActivityJniAbi::SeventeenArg));
        }
        // Check 2-arg (legacy AGDK 1.x / 2.x)
        if contains_subslice(window_slice, SIG_2_ARG) {
            return Ok(Some(GameActivityJniAbi::TwoArg));
        }

        if total_valid >= max_needle_len - 1 {
            carryover = max_needle_len - 1;
            let start = total_valid - carryover;
            buffer.copy_within(start..total_valid, 0);
        } else {
            carryover = total_valid;
        }
    }

    Ok(None)
}

/// Scans a compiled native shared library (.so) for GameActivity JNI method signatures using single-pass streaming I/O.
pub fn detect_native_game_activity_abi(so_path: &Path) -> Result<Option<GameActivityJniAbi>> {
    let file = File::open(so_path).with_context(|| {
        format!(
            "Failed to open shared library `{}` for JNI ABI scan",
            so_path.display()
        )
    })?;

    stream_detect_game_activity_abi(BufReader::new(file)).with_context(|| {
        format!(
            "I/O error while scanning `{}` for GameActivity JNI ABI",
            so_path.display()
        )
    })
}

/// Scans a Dalvik Executable (.dex) file on disk for GameActivity JNI method signatures
/// using single-pass streaming I/O, bounding heap memory consumption to 64KB.
pub fn detect_dex_file_game_activity_abi(dex_path: &Path) -> Result<Option<GameActivityJniAbi>> {
    let file = File::open(dex_path).with_context(|| {
        format!(
            "Failed to open DEX file `{}` for JNI ABI scan",
            dex_path.display()
        )
    })?;

    stream_detect_game_activity_abi(BufReader::new(file)).with_context(|| {
        format!(
            "I/O error while scanning DEX file `{}` for GameActivity JNI ABI",
            dex_path.display()
        )
    })
}

/// Scans GameActivity.class bytes or Dalvik Executable (classes.dex) bytes for GameActivity JNI method signatures.
pub fn detect_class_game_activity_abi(class_bytes: &[u8]) -> Option<GameActivityJniAbi> {
    if contains_subslice(class_bytes, SIG_17_ARG) {
        return Some(GameActivityJniAbi::SeventeenArg);
    }
    if contains_subslice(class_bytes, SIG_2_ARG) {
        return Some(GameActivityJniAbi::TwoArg);
    }
    None
}

/// Maps a Google `androidx.games:games-activity` version string to its expected JNI ABI variant.
pub fn resolve_abi_for_games_activity_version(version: &str) -> GameActivityJniAbi {
    let clean = version.trim_start_matches(|c: char| !c.is_ascii_digit());
    let parts: Vec<&str> = clean.split('.').collect();
    let major: u32 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);

    if major == 0 || major <= 2 {
        GameActivityJniAbi::TwoArg
    } else {
        GameActivityJniAbi::SeventeenArg
    }
}

/// Validates that the native binary and the packed GameActivity class share the exact same JNI ABI.
pub fn validate_game_activity_abi(
    so_path: &Path,
    class_or_dex_bytes: Option<&[u8]>,
    explicit_version: Option<&str>,
) -> Result<()> {
    let resolved_class_abi = class_or_dex_bytes.and_then(detect_class_game_activity_abi);
    validate_game_activity_abi_with_resolved(so_path, resolved_class_abi, explicit_version)
}

/// Validates that the native binary and the resolved GameActivity ABI share the exact same JNI ABI variant.
pub fn validate_game_activity_abi_with_resolved(
    so_path: &Path,
    bytecode_abi: Option<GameActivityJniAbi>,
    explicit_version: Option<&str>,
) -> Result<()> {
    // 1. Scan the native binary:
    let native_abi = match detect_native_game_activity_abi(so_path)? {
        Some(abi) => abi,
        None => return Ok(()), // Native binary does not use GameActivity
    };

    // 2. Resolve expected bytecode ABI either from pre-resolved DEX ABI or explicit version:
    let target_abi = match (bytecode_abi, explicit_version) {
        (Some(abi), _) => abi,
        (None, Some(ver)) => resolve_abi_for_games_activity_version(ver),
        (None, None) => return Ok(()), // No bytecode or explicit version available to mismatch against
    };

    if native_abi != target_abi {
        let (native_desc, target_desc, suggested_ver) = match (native_abi, target_abi) {
            (GameActivityJniAbi::SeventeenArg, GameActivityJniAbi::TwoArg) => (
                "17-argument (AGDK 4.x / 3.x)",
                "2-argument (AGDK 2.x)",
                "4.4.2",
            ),
            (GameActivityJniAbi::TwoArg, GameActivityJniAbi::SeventeenArg) => (
                "2-argument (AGDK 2.x)",
                "17-argument (AGDK 4.x / 3.x)",
                "2.0.2",
            ),
            _ => unreachable!(),
        };

        let native_sig = if native_abi == GameActivityJniAbi::SeventeenArg {
            "(JLandroid/view/MotionEvent;IIIIIJJIIIIIIFF)Z"
        } else {
            "(JLandroid/view/MotionEvent;)Z"
        };

        let target_sig = if target_abi == GameActivityJniAbi::SeventeenArg {
            "(JLandroid/view/MotionEvent;IIIIIJJIIIIIIFF)Z"
        } else {
            "(JLandroid/view/MotionEvent;)Z"
        };

        bail!(
            "GameActivity Native JNI ABI Mismatch!\n\
             Native shared library `{}` requires {} signature:\n  {}\n\
             but the resolved `androidx.games:games-activity` provides {} signature:\n  {}\n\
             Current configured version: {:?}\n\
             Recommended fix: Set `game_activity = \"{}\"` in your `manifest.toml` or omit explicit pinning to enable auto-resolution.",
            so_path.display(),
            native_desc,
            native_sig,
            target_desc,
            target_sig,
            explicit_version.unwrap_or("auto-detected"),
            suggested_ver
        );
    }

    Ok(())
}
