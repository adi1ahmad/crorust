use std::path::Path;

#[cfg(windows)]
extern "system" {
    fn GetShortPathNameW(lpszLongPath: *const u16, lpszShortPath: *mut u16, cchBuffer: u32) -> u32;
}

#[cfg(windows)]
fn to_short_path_if_has_spaces(path: &Path) -> std::path::PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let path_str = path.to_string_lossy();
    if !path_str.contains(' ') {
        return path.to_path_buf();
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let needed = GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0);
        if needed > 0 {
            let mut buf = vec![0u16; needed as usize];
            let written = GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), needed);
            if written > 0 && written <= needed {
                buf.truncate(written as usize);
                return std::path::PathBuf::from(std::ffi::OsString::from_wide(&buf));
            }
        }
    }
    path.to_path_buf()
}

pub fn sanitize_path(path: &Path) -> String {
    let simplified = dunce::simplified(path);
    #[cfg(windows)]
    let simplified_buf = to_short_path_if_has_spaces(simplified);
    #[cfg(windows)]
    let simplified = simplified_buf.as_path();

    let s = simplified.to_string_lossy().replace('\\', "/");

    if let Some(stripped) = s.strip_prefix("//?/UNC/") {
        format!("//{stripped}")
    } else if let Some(stripped) = s.strip_prefix("//?/") {
        stripped.to_string()
    } else {
        s
    }
}

pub fn platform_executable_name(name: &str, is_windows: bool) -> String {
    if is_windows {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

pub fn host_executable_name(name: &str) -> String {
    platform_executable_name(name, cfg!(target_os = "windows"))
}
