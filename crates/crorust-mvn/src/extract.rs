use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct ParallelAarUnpacker;

impl ParallelAarUnpacker {
    pub fn unpack_all_parallel(aar_archive_paths: &[(PathBuf, PathBuf)]) -> Result<()> {
        aar_archive_paths
            .par_iter()
            .try_for_each(|(source_aar, dest_dir)| {
                Self::unpack_single_aar(source_aar, dest_dir).with_context(|| {
                    format!("Failed parallel extraction of `{}`", source_aar.display())
                })
            })
    }

    pub fn unpack_single_aar(source_aar: &Path, dest_dir: &Path) -> Result<()> {
        let file = File::open(source_aar)?;
        let mut archive = zip::ZipArchive::new(file)?;
        std::fs::create_dir_all(dest_dir)?;

        for i in 0..archive.len() {
            let mut file_entry = archive.by_index(i)?;
            let relative_name = file_entry
                .enclosed_name()
                .context("Invalid zip entry path in AAR")?
                .to_owned();
            let output_path = dest_dir.join(&relative_name);

            if file_entry.name().ends_with('/') {
                std::fs::create_dir_all(&output_path)?;
            } else {
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out_file = File::create(&output_path)?;
                std::io::copy(&mut file_entry, &mut out_file)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_parallel_aar_unpacking() -> Result<()> {
        let tmp = tempdir()?;
        let mut test_aars = Vec::new();

        for i in 0..3 {
            let aar_path = tmp.path().join(format!("test_{i}.aar"));
            let out_dir = tmp.path().join(format!("unpacked_{i}"));

            let file = File::create(&aar_path)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::FileOptions::default();
            zip.start_file("classes.jar", options)?;
            zip.write_all(b"fake jar bytecode")?;
            zip.start_file("res/values/strings.xml", options)?;
            zip.write_all(b"<resources/>")?;
            zip.finish()?;

            test_aars.push((aar_path, out_dir));
        }

        ParallelAarUnpacker::unpack_all_parallel(&test_aars)?;

        for (_, out_dir) in &test_aars {
            assert!(out_dir.join("classes.jar").exists());
            assert!(out_dir.join("res/values/strings.xml").exists());
            let jar_bytes = std::fs::read(out_dir.join("classes.jar"))?;
            assert_eq!(jar_bytes, b"fake jar bytecode");
        }

        Ok(())
    }
}
