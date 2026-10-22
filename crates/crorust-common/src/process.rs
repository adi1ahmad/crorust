use std::path::Path;
use std::process::Command;

/// Creates a cross-platform Command that correctly handles Windows batch files
/// (.bat and .cmd) by routing through cmd.exe /C, while executing binary
/// executables directly across all platforms without shell overhead.
pub fn create_platform_command<P: AsRef<Path>>(binary_path: P) -> Command {
    let path = binary_path.as_ref();
    let is_batch = cfg!(target_os = "windows")
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd"))
            .unwrap_or(false);

    if is_batch {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(path);
        cmd
    } else {
        Command::new(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_platform_command_batch_detection() {
        let cmd = create_platform_command(Path::new("test.bat"));
        if cfg!(target_os = "windows") {
            assert_eq!(cmd.get_program(), "cmd");
        } else {
            assert_eq!(cmd.get_program(), "test.bat");
        }
    }

    #[test]
    fn test_create_platform_command_binary_passthrough() {
        let cmd = create_platform_command(Path::new("test.exe"));
        assert_eq!(cmd.get_program(), "test.exe");
    }
}
