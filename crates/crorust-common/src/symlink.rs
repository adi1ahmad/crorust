use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn safe_symlink_or_copy(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory `{}`", parent.display()))?;
    }

    if dst.exists() || dst.is_symlink() {
        if dst.is_dir() && !dst.is_symlink() {
            let _ = std::fs::remove_dir_all(dst);
        } else {
            let _ = std::fs::remove_file(dst);
        }
    }

    if let Ok(()) = symlink(src, dst) {
        return Ok(());
    }

    tracing::debug!(
        "Symlink failed for `{}` -> `{}`; falling back to copy",
        src.display(),
        dst.display()
    );

    if src.is_dir() {
        copy_dir_all(src, dst)?;
    } else {
        std::fs::copy(src, dst).with_context(|| {
            format!("Failed to copy `{}` to `{}`", src.display(), dst.display())
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = src.metadata() {
                let _ = std::fs::set_permissions(
                    dst,
                    std::fs::Permissions::from_mode(meta.permissions().mode()),
                );
            }
        }
    }

    Ok(())
}

pub fn copy_dir_all(source: &Path, dest: &Path) -> Result<()> {
    let mut visited = HashSet::new();
    if let Ok(canon_dest) = dunce::canonicalize(dest) {
        visited.insert(canon_dest);
    }
    copy_dir_all_guarded(source, dest, &mut visited)
}

fn copy_dir_all_guarded(source: &Path, dest: &Path, visited: &mut HashSet<PathBuf>) -> Result<()> {
    if let Ok(canon_source) = dunce::canonicalize(source) {
        if let Ok(canon_dest) = dunce::canonicalize(dest) {
            if canon_dest.starts_with(&canon_source) {
                tracing::debug!(
                    "Ancestor-to-descendant directory copy cycle prevented: `{}` -> `{}`",
                    source.display(),
                    dest.display()
                );
                return Ok(());
            }
        }
        if !visited.insert(canon_source) {
            tracing::debug!(
                "Cyclic directory traversal prevented in copy_dir_all at `{}`",
                source.display()
            );
            return Ok(());
        }
    }

    // Snapshot source entries before creating `dest` so that if `dest` is inside `source`,
    // `dest` is never observed as a child entry of `source`.
    let entries: Vec<std::fs::DirEntry> =
        std::fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;

    std::fs::create_dir_all(dest)?;
    if let Ok(canon_dest) = dunce::canonicalize(dest) {
        visited.insert(canon_dest);
    }

    for entry in entries {
        let file_name = entry.file_name();
        let src_child = source.join(&file_name);
        let dst_child = dest.join(&file_name);
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_all_guarded(&src_child, &dst_child, visited)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_child, &dst_child)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = entry.metadata() {
                    let _ = std::fs::set_permissions(
                        &dst_child,
                        std::fs::Permissions::from_mode(meta.permissions().mode()),
                    );
                }
            }
        } else if file_type.is_symlink() {
            let raw_target = std::fs::read_link(&src_child).with_context(|| {
                format!(
                    "Failed to read symlink target for `{}`",
                    src_child.display()
                )
            })?;

            let resolved_src = if raw_target.is_relative() {
                source.join(&raw_target)
            } else {
                raw_target.clone()
            };

            copy_symlink_entry_guarded(&raw_target, &resolved_src, &dst_child, visited)?;
        }
    }
    Ok(())
}

pub fn copy_symlink_entry(raw_target: &Path, resolved_src: &Path, dst: &Path) -> Result<()> {
    let mut visited = HashSet::new();
    copy_symlink_entry_guarded(raw_target, resolved_src, dst, &mut visited)
}

fn copy_symlink_entry_guarded(
    raw_target: &Path,
    resolved_src: &Path,
    dst: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory `{}`", parent.display()))?;
    }

    if dst.exists() || dst.is_symlink() {
        if dst.is_dir() && !dst.is_symlink() {
            let _ = std::fs::remove_dir_all(dst);
        } else {
            let _ = std::fs::remove_file(dst);
        }
    }

    let symlink_result = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(raw_target, dst)
        }
        #[cfg(windows)]
        {
            if resolved_src.is_dir() {
                std::os::windows::fs::symlink_dir(raw_target, dst)
            } else {
                std::os::windows::fs::symlink_file(raw_target, dst)
            }
        }
    };

    if symlink_result.is_ok() {
        return Ok(());
    }

    tracing::debug!(
        "Recreating symlink `{}` -> `{}` failed ({:?}); falling back to copy from `{}`",
        dst.display(),
        raw_target.display(),
        symlink_result.err(),
        resolved_src.display()
    );

    if !resolved_src.exists() && !resolved_src.is_symlink() {
        anyhow::bail!(
            "Cannot copy dangling symlink `{}`: target `{}` does not exist and symlink creation failed",
            dst.display(),
            resolved_src.display()
        );
    }

    if resolved_src.is_dir() {
        copy_dir_all_guarded(resolved_src, dst, visited)?;
    } else {
        std::fs::copy(resolved_src, dst).with_context(|| {
            format!(
                "Failed to copy resolved symlink target `{}` to `{}`",
                resolved_src.display(),
                dst.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = resolved_src.metadata() {
                let _ = std::fs::set_permissions(
                    dst,
                    std::fs::Permissions::from_mode(meta.permissions().mode()),
                );
            }
        }
    }

    Ok(())
}

pub fn symlink(target: &Path, dest: &Path) -> Result<()> {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, dest)?;
    #[cfg(windows)]
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, dest)?;
    } else {
        std::os::windows::fs::symlink_file(target, dest)?;
    }
    Ok(())
}
