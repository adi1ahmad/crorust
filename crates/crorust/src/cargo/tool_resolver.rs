//! Locating LLVM helper tools.
//!
//! `cc-rs` (used by build scripts such as `ring`) reads `AR_<triple>` and
//! `CC_<triple>`. If we hand it a bare name that is not on `PATH` the build
//! dies deep inside a dependency with `failed to find tool "llvm-ar"`. We look
//! for the tool ourselves, so we either pass an absolute path or fail early
//! with a message that says how to fix it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

/// A tool we need, plus the other places and names it may be found under.
pub struct ToolSpec {
    pub canonical: &'static str,
    /// Environment variable that pins an exact path, e.g. `CRORUST_LLVM_AR`.
    pub env_override: &'static str,
    /// Distro-style suffixed names, tried after the canonical one.
    pub versioned_names: &'static [&'static str],
    /// Acceptable stand-ins that behave the same for our purposes (e.g. the
    /// system `ar` for `llvm-ar` on Unix). Only used when nothing else exists.
    pub fallback_names: &'static [&'static str],
    /// When the tool is just another LLVM binary under a different name
    /// (`clang-cl` is `clang`, `llvm-lib` is `llvm-ar`), the program whose
    /// symlink we can create under the canonical name.
    pub shim_of: Option<&'static ToolSpec>,
}

pub static CLANG: ToolSpec = ToolSpec {
    canonical: "clang",
    env_override: "CRORUST_CLANG",
    versioned_names: &["clang-19", "clang-18", "clang-17"],
    fallback_names: &[],
    shim_of: None,
};

#[cfg(windows)]
const LLVM_AR_FALLBACKS: &[&str] = &[];
#[cfg(not(windows))]
const LLVM_AR_FALLBACKS: &[&str] = &["ar"];

pub static LLVM_AR: ToolSpec = ToolSpec {
    canonical: "llvm-ar",
    env_override: "CRORUST_LLVM_AR",
    versioned_names: &["llvm-ar-19", "llvm-ar-18", "llvm-ar-17"],
    // Xcode's `ar` and GNU `ar` both produce archives cc-rs can use.
    fallback_names: LLVM_AR_FALLBACKS,
    shim_of: None,
};

pub static LLVM_LIB: ToolSpec = ToolSpec {
    canonical: "llvm-lib",
    env_override: "CRORUST_LLVM_LIB",
    versioned_names: &["llvm-lib-19", "llvm-lib-18", "llvm-lib-17"],
    fallback_names: &[],
    shim_of: Some(&LLVM_AR),
};

pub static CLANG_CXX: ToolSpec = ToolSpec {
    canonical: "clang++",
    env_override: "CRORUST_CLANG_CXX",
    versioned_names: &["clang++-19", "clang++-18", "clang++-17"],
    fallback_names: &[],
    shim_of: None,
};

pub static CLANG_CL: ToolSpec = ToolSpec {
    canonical: "clang-cl",
    env_override: "CRORUST_CLANG_CL",
    versioned_names: &["clang-cl-19", "clang-cl-18", "clang-cl-17"],
    fallback_names: &[],
    shim_of: Some(&CLANG),
};

/// Directories that commonly hold LLVM binaries without being on `PATH`.
pub fn well_known_llvm_dirs() -> Vec<PathBuf> {
    [
        "/opt/homebrew/opt/llvm/bin",
        "/usr/local/opt/llvm/bin",
        "/usr/lib/llvm-19/bin",
        "/usr/lib/llvm-18/bin",
        "/usr/lib/llvm-17/bin",
        r"C:\Program Files\LLVM\bin",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

/// Directory where symlink shims are created.
pub fn default_shim_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("crorust")
        .join("shims")
}

/// Resolves `spec` using the real process environment and `PATH`.
pub fn resolve(spec: &ToolSpec, extra_dirs: &[PathBuf], shim_dir: &Path) -> Result<PathBuf> {
    resolve_with(
        spec,
        std::env::var_os(spec.env_override),
        std::env::var_os("PATH"),
        extra_dirs,
        true,
        shim_dir,
    )
}

/// Same as [`resolve`] but with the environment passed in, so it can be tested
/// without touching process-global state.
pub fn resolve_with(
    spec: &ToolSpec,
    env_override: Option<OsString>,
    path_var: Option<OsString>,
    extra_dirs: &[PathBuf],
    include_well_known: bool,
    shim_dir: &Path,
) -> Result<PathBuf> {
    if let Some(v) = env_override {
        let p = PathBuf::from(&v);
        if p.is_file() {
            return Ok(p);
        }
        bail!(
            "{} is set to `{}` but that file does not exist",
            spec.env_override,
            p.display()
        );
    }

    if let Some(p) = find_primary(spec, path_var.as_deref(), extra_dirs, include_well_known) {
        return Ok(p);
    }

    // Before creating a shim of a base tool, check if the real canonical tool exists
    // in well-known distribution directories (e.g. Homebrew LLVM /opt/homebrew/opt/llvm/bin/clang-cl)
    if include_well_known {
        let wk = well_known_llvm_dirs();
        if let Some(found) = lookup_in_dirs(spec.canonical, &wk) {
            return Ok(found);
        }
    }

    if let Some(base) = spec.shim_of {
        if let Some(src) = find_primary(base, path_var.as_deref(), extra_dirs, include_well_known) {
            if !is_apple_clang(&src) {
                return make_shim(&src, spec.canonical, shim_dir);
            }
        }
    }

    for name in spec.fallback_names {
        if let Some(p) = lookup(name, path_var.as_deref(), &[]) {
            return Ok(p);
        }
    }

    bail!(
        "`{}` was not found. Install LLVM (apt: `sudo apt install llvm clang lld`, \
         brew: `brew install llvm`) or set {} to its full path.",
        spec.canonical,
        spec.env_override
    )
}

fn is_apple_clang(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if path.starts_with("/usr/bin") || path.starts_with("/Applications/Xcode.app") {
            return true;
        }
        if let Ok(output) = std::process::Command::new(path).arg("--version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.contains("Apple clang") || stdout.contains("Apple LLVM");
        }
    }
    let _ = path;
    false
}

/// Checks for an executable file named `name` in the given directories.
fn lookup_in_dirs(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

fn lookup(
    name: &str,
    path_var: Option<&std::ffi::OsStr>,
    extra_dirs: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(found) = lookup_in_dirs(name, extra_dirs) {
        return Some(found);
    }
    if let Some(p) = path_var {
        if let Ok(found) = which::which_in(name, Some(p), ".") {
            return Some(found);
        }
    }
    None
}

/// Canonical name, then versioned names; prioritized in target-specific extra_dirs,
/// then on PATH, then in well-known distribution locations.
fn find_primary(
    spec: &ToolSpec,
    path_var: Option<&std::ffi::OsStr>,
    extra_dirs: &[PathBuf],
    include_well_known: bool,
) -> Option<PathBuf> {
    let names: Vec<&str> = std::iter::once(spec.canonical)
        .chain(spec.versioned_names.iter().copied())
        .collect();

    // 1. Target-specific extra dirs (e.g. Android NDK toolchain bin) take top priority
    for &name in &names {
        if let Some(found) = lookup_in_dirs(name, extra_dirs) {
            return Some(found);
        }
    }

    // 2. System PATH
    if let Some(p) = path_var {
        for &name in &names {
            if let Ok(found) = which::which_in(name, Some(p), ".") {
                return Some(found);
            }
        }
    }

    // 3. Well-known fallback locations
    if include_well_known {
        let fallback_dirs = well_known_llvm_dirs();
        for &name in &names {
            if let Some(found) = lookup_in_dirs(name, &fallback_dirs) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(unix)]
fn make_shim(src: &Path, name: &str, shim_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(shim_dir)?;
    let link = shim_dir.join(name);
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(&link)?;
    }
    std::os::unix::fs::symlink(src, &link)?;
    Ok(link)
}

#[cfg(not(unix))]
fn make_shim(_src: &Path, name: &str, _shim_dir: &Path) -> Result<PathBuf> {
    bail!("`{name}` was not found and symlink shims are not supported on this host")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake_tool(dir: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn path_of(dir: &Path) -> Option<OsString> {
        Some(dir.as_os_str().to_owned())
    }

    #[test]
    fn prefers_canonical_name_over_versioned() {
        let t = tempfile::tempdir().unwrap();
        let want = fake_tool(t.path(), "llvm-ar");
        fake_tool(t.path(), "llvm-ar-18");
        let got = resolve_with(
            &LLVM_AR,
            None,
            path_of(t.path()),
            &[],
            false,
            &t.path().join("shims"),
        )
        .unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn falls_back_to_versioned_name() {
        let t = tempfile::tempdir().unwrap();
        let want = fake_tool(t.path(), "llvm-ar-18");
        let got = resolve_with(
            &LLVM_AR,
            None,
            path_of(t.path()),
            &[],
            false,
            &t.path().join("shims"),
        )
        .unwrap();
        assert_eq!(got, want);
    }

    /// Resolves a tool from extra search directories when not present on system PATH.
    #[test]
    fn finds_tool_in_extra_dir_when_not_on_path() {
        let empty = tempfile::tempdir().unwrap();
        let ndk = tempfile::tempdir().unwrap();
        let want = fake_tool(ndk.path(), "llvm-ar");
        let got = resolve_with(
            &LLVM_AR,
            None,
            path_of(empty.path()),
            &[ndk.path().to_path_buf()],
            false,
            &empty.path().join("shims"),
        )
        .unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn env_override_wins_over_path() {
        let t = tempfile::tempdir().unwrap();
        fake_tool(t.path(), "llvm-ar");
        let pinned = fake_tool(&t.path().join("pinned"), "my-ar");
        let got = resolve_with(
            &LLVM_AR,
            Some(pinned.clone().into_os_string()),
            path_of(t.path()),
            &[],
            false,
            &t.path().join("shims"),
        )
        .unwrap();
        assert_eq!(got, pinned);
    }

    #[test]
    fn env_override_pointing_nowhere_names_the_variable() {
        let t = tempfile::tempdir().unwrap();
        let err = resolve_with(
            &LLVM_AR,
            Some(OsString::from("/definitely/not/here")),
            path_of(t.path()),
            &[],
            false,
            &t.path().join("shims"),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("CRORUST_LLVM_AR"), "{err}");
    }

    /// Synthesizes a clang-cl shim when only plain clang is available.
    #[test]
    fn clang_cl_is_shimmed_from_clang() {
        let t = tempfile::tempdir().unwrap();
        let clang = fake_tool(t.path(), "clang");
        let shims = t.path().join("shims");
        let got = resolve_with(&CLANG_CL, None, path_of(t.path()), &[], false, &shims).unwrap();
        assert_eq!(got, shims.join("clang-cl"));
        assert_eq!(std::fs::read_link(&got).unwrap(), clang);
    }

    #[test]
    fn llvm_lib_is_shimmed_from_llvm_ar() {
        let t = tempfile::tempdir().unwrap();
        let ar = fake_tool(t.path(), "llvm-ar");
        let shims = t.path().join("shims");
        let got = resolve_with(&LLVM_LIB, None, path_of(t.path()), &[], false, &shims).unwrap();
        assert_eq!(std::fs::read_link(&got).unwrap(), ar);
    }

    #[test]
    fn shim_is_refreshed_when_source_moves() {
        let t = tempfile::tempdir().unwrap();
        let shims = t.path().join("shims");
        let first = tempfile::tempdir().unwrap();
        fake_tool(first.path(), "clang");
        resolve_with(&CLANG_CL, None, path_of(first.path()), &[], false, &shims).unwrap();
        let second = tempfile::tempdir().unwrap();
        let clang2 = fake_tool(second.path(), "clang");
        let got =
            resolve_with(&CLANG_CL, None, path_of(second.path()), &[], false, &shims).unwrap();
        assert_eq!(std::fs::read_link(&got).unwrap(), clang2);
    }

    #[test]
    fn system_ar_is_last_resort_for_llvm_ar() {
        let t = tempfile::tempdir().unwrap();
        let ar = fake_tool(t.path(), "ar");
        let got = resolve_with(
            &LLVM_AR,
            None,
            path_of(t.path()),
            &[],
            false,
            &t.path().join("s"),
        )
        .unwrap();
        assert_eq!(got, ar);
    }

    #[test]
    fn extra_dirs_take_precedence_over_system_path() {
        let t = tempfile::tempdir().unwrap();
        let sys_dir = t.path().join("sys");
        let target_dir = t.path().join("target_ndk");
        std::fs::create_dir_all(&sys_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();

        let sys_clang = fake_tool(&sys_dir, "clang++");
        let target_clang = fake_tool(&target_dir, "clang++");

        let shims = t.path().join("shims");
        let got = resolve_with(
            &CLANG_CXX,
            None,
            path_of(&sys_dir),
            std::slice::from_ref(&target_dir),
            false,
            &shims,
        )
        .unwrap();

        assert_eq!(
            got, target_clang,
            "Target toolchain from extra_dirs must be selected over host PATH"
        );
        assert_ne!(got, sys_clang);
    }

    #[test]
    fn test_clang_cl_prefers_direct_tool_in_extra_dirs_over_shimming() {
        let t = tempfile::tempdir().unwrap();
        let path_dir = t.path().join("path_bin");
        let extra_dir = t.path().join("extra_llvm");
        let shims = t.path().join("shims");

        fake_tool(&path_dir, "clang");
        let direct_cl = fake_tool(&extra_dir, "clang-cl");

        let got = resolve_with(
            &CLANG_CL,
            None,
            path_of(&path_dir),
            std::slice::from_ref(&extra_dir),
            false,
            &shims,
        )
        .unwrap();

        assert_eq!(
            got, direct_cl,
            "Direct clang-cl in tool search dirs must be preferred over creating a shim from clang on PATH"
        );
    }
}
