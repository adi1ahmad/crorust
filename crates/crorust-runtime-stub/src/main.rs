use std::env;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, ExitStatus};

pub const SQUASHFS_MAGIC: [u8; 4] = [0x73, 0x71, 0x73, 0x68];

fn main() {
    let code = match run() {
        Ok(status) => status.code().unwrap_or(0),
        Err(err) => {
            eprintln!("crorust-appimage-runtime: error: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> Result<ExitStatus, Box<dyn std::error::Error>> {
    let self_exe = env::current_exe()?;
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--appimage-version") {
        println!(
            "CroRust Clean-Room AppImage Runtime v{}",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(std::process::ExitStatus::default());
    }

    let squashfs_offset = locate_squashfs_offset(&self_exe)?;

    if args.iter().any(|a| a == "--appimage-offset") {
        println!("{squashfs_offset}");
        return Ok(std::process::ExitStatus::default());
    }

    let extract_mode = args.iter().any(|a| a == "--appimage-extract");

    if extract_mode {
        let dest = Path::new("squashfs-root");
        println!("Extracting embedded SquashFS archive to {}", dest.display());
        extract_squashfs(&self_exe, squashfs_offset, dest)?;
        return Ok(std::process::ExitStatus::default());
    }

    let temp_dir = env::temp_dir().join(format!(
        ".mount_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    extract_squashfs(&self_exe, squashfs_offset, &temp_dir)?;

    let apprun_path = temp_dir.join("AppRun");
    if !apprun_path.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(format!(
            "AppRun executable not found in AppDir at {}",
            apprun_path.display()
        )
        .into());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&apprun_path, std::fs::Permissions::from_mode(0o755));
    }

    env::set_var("APPDIR", &temp_dir);
    env::set_var("APPIMAGE", &self_exe);
    if let Ok(cwd) = env::current_dir() {
        env::set_var("OWD", cwd);
    }

    let app_args: Vec<String> = env::args().skip(1).collect();
    let status = Command::new(&apprun_path).args(&app_args).status()?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(status)
}

fn locate_squashfs_offset(exe_path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut file = File::open(exe_path)?;
    let file_len = file.metadata()?.len();

    let mut buffer = [0u8; 4096];
    let mut current_offset = 64 * 1024u64;

    while current_offset < file_len {
        file.seek(SeekFrom::Start(current_offset))?;
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read < 4 {
            break;
        }
        for i in 0..bytes_read - 3 {
            if buffer[i..i + 4] == SQUASHFS_MAGIC {
                return Ok(current_offset + i as u64);
            }
        }
        current_offset += (bytes_read - 3) as u64;
    }

    Err("SquashFS filesystem superblock not found in container".into())
}

fn extract_squashfs(
    exe_path: &Path,
    offset: u64,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(exe_path)?;
    file.seek(SeekFrom::Start(offset))?;

    let reader = BufReader::new(file);
    let filesystem = backhand::FilesystemReader::from_reader(reader)?;

    for node in filesystem.files() {
        let full_path = out_dir.join(node.fullpath.strip_prefix("/").unwrap_or(&node.fullpath));
        match &node.inner {
            backhand::InnerNode::File(file) => {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut file_reader = filesystem.file(file).reader();
                let mut bytes = Vec::new();
                file_reader.read_to_end(&mut bytes)?;
                std::fs::write(&full_path, bytes)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        &full_path,
                        std::fs::Permissions::from_mode(node.header.permissions as u32),
                    );
                }
            }
            backhand::InnerNode::Dir(_) => {
                std::fs::create_dir_all(&full_path)?;
            }
            backhand::InnerNode::Symlink(_link) => {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                #[cfg(unix)]
                {
                    let _ = std::os::unix::fs::symlink(&_link.link, &full_path);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
