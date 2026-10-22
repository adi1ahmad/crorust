use anyhow::{Context, Result};
use std::path::Path;

pub fn assemble_portable(
    source_dir: &Path,
    dest_dir: &Path,
    binaries: &[(&Path, &str)],
) -> Result<()> {
    std::fs::create_dir_all(dest_dir).with_context(|| {
        format!(
            "Failed creating destination directory: {}",
            dest_dir.display()
        )
    })?;

    for (src, name) in binaries {
        let dest = dest_dir.join(name);
        std::fs::copy(src, &dest).with_context(|| {
            format!(
                "Failed copying binary {} to {}",
                src.display(),
                dest.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        }
    }

    if source_dir.exists() && source_dir.is_dir() {
        crate::copy_dir_all(source_dir, dest_dir).with_context(|| {
            format!(
                "Failed copying assets from {} to {}",
                source_dir.display(),
                dest_dir.display()
            )
        })?;
    }

    Ok(())
}
