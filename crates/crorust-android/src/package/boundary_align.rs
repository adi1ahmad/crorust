use anyhow::{Context, Result};
use std::fs::File;
use std::path::Path;
use zip::ZipArchive;

pub const ANDROID_DEX_ALIGNMENT: u32 = 4;

pub const ANDROID_SO_ALIGNMENT: u32 = 16384;

pub fn verify_apk_boundary_alignments(apk_path: &Path) -> Result<bool> {
    let file = File::open(apk_path)
        .with_context(|| format!("failed to open APK at {}", apk_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("invalid zip container at {}", apk_path.display()))?;

    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        let data_start = entry.data_start();

        if name.ends_with(".so") && name.starts_with("lib/") {
            anyhow::ensure!(
                data_start % (ANDROID_SO_ALIGNMENT as u64) == 0,
                "Shared library '{}' at offset {} is not aligned to 16KB boundary ({})",
                name,
                data_start,
                ANDROID_SO_ALIGNMENT
            );
        } else if name.starts_with("classes") && name.ends_with(".dex") {
            anyhow::ensure!(
                data_start % (ANDROID_DEX_ALIGNMENT as u64) == 0,
                "Dalvik executable '{}' at offset {} is not aligned to 4-byte boundary ({})",
                name,
                data_start,
                ANDROID_DEX_ALIGNMENT
            );
        }
    }

    Ok(true)
}
