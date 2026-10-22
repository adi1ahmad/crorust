use anyhow::{Context, Result};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::{Builder, Header};

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);

    let template_source = manifest_dir.join("../../templates/crorust.ruxt");
    let bundled_archive = manifest_dir.join("assets/template.tar.zst");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let archive_path = out_dir.join("template.tar.zst");

    if template_source.is_dir() {
        println!("cargo:rerun-if-changed={}", template_source.display());

        let zstd_file = File::create(&archive_path)
            .with_context(|| format!("Failed to create archive at '{}'", archive_path.display()))?;
        let zstd_writer = zstd::Encoder::new(zstd_file, 19)?;
        let mut tar_builder = Builder::new(zstd_writer);

        pack_directory_deterministic(&template_source, &template_source, &mut tar_builder)?;
        let zstd_writer = tar_builder.into_inner()?;
        let mut zstd_file = zstd_writer.finish()?;
        zstd_file.flush()?;
        drop(zstd_file);

        // Also refresh the bundled asset inside crates/crorust/assets/ for crates.io packaging:
        if let Some(parent) = bundled_archive.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::copy(&archive_path, &bundled_archive);
    } else if bundled_archive.is_file() {
        println!("cargo:rerun-if-changed={}", bundled_archive.display());
        fs::copy(&bundled_archive, &archive_path).with_context(|| {
            format!(
                "Failed to copy bundled template archive from '{}'",
                bundled_archive.display()
            )
        })?;
    } else if env::var("CRORUST_BOOTSTRAP").as_deref() == Ok("1") {
        println!(
            "cargo:warning=CRORUST_BOOTSTRAP=1 detected: Neither template directory nor bundled archive found. Creating placeholder archive."
        );
        let empty: Vec<u8> = Vec::new();
        let compressed = zstd::encode_all(&empty[..], 0)?;
        fs::write(&archive_path, compressed)?;
    } else {
        panic!(
            "\n\n========================================================================\n\
             FATAL: Template archive source not found!\n\
             Neither the template directory ('{}') nor the bundled archive ('{}') exists.\n\
             crorust requires either templates/crorust.ruxt or assets/template.tar.zst to build.\n\
             To force a stub archive for bootstrapping, set CRORUST_BOOTSTRAP=1.\n\
             ========================================================================\n",
            template_source.display(),
            bundled_archive.display()
        );
    }

    Ok(())
}

fn pack_directory_deterministic<W: std::io::Write>(
    base_dir: &Path,
    current_dir: &Path,
    tar: &mut Builder<W>,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current_dir)?.filter_map(Result::ok).collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if path.is_dir() {
            if file_name_str == "target"
                || file_name_str == ".cache"
                || file_name_str == ".direnv"
                || file_name_str == ".git"
                || file_name_str.starts_with("result")
            {
                continue;
            }
            pack_directory_deterministic(base_dir, &path, tar)?;
        } else {
            if file_name_str == "flake.lock" || file_name_str.ends_with(".part") {
                continue;
            }

            println!("cargo:rerun-if-changed={}", path.display());

            let rel_path = path.strip_prefix(base_dir)?;
            let mut file = File::open(&path)?;
            let metadata = entry.metadata()?;

            let is_executable = is_template_file_executable(&path, &metadata)?;
            let canonical_mode = if is_executable { 0o755 } else { 0o644 };

            let mut header = Header::new_gnu();
            header.set_size(metadata.len());
            header.set_mode(canonical_mode);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_cksum();

            tar.append_data(&mut header, rel_path, &mut file)?;
        }
    }
    Ok(())
}

fn is_template_file_executable(path: &Path, metadata: &fs::Metadata) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        if mode & 0o111 != 0 {
            return Ok(true);
        }
    }

    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        if ext.eq_ignore_ascii_case("sh") || ext.eq_ignore_ascii_case("bash") {
            return Ok(true);
        }
    }

    if metadata.len() >= 2 {
        if let Ok(mut file) = File::open(path) {
            use std::io::Read;
            let mut magic = [0u8; 2];
            if file.read_exact(&mut magic).is_ok() && &magic == b"#!" {
                return Ok(true);
            }
        }
    }

    Ok(false)
}
