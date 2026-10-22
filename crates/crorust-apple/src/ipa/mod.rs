use anyhow::{Context, Result};
use crorust_common::{ArchiveEntryOptions, BoundaryZipArchive};
use std::path::Path;

pub fn create_ipa(app_dir: &Path, out_ipa: &Path, app_name: &str) -> Result<()> {
    let mut ipa = BoundaryZipArchive::new(out_ipa, false)?;
    let dest_in_zip = Path::new("Payload").join(format!("{app_name}.app"));
    ipa.add_directory(app_dir, &dest_in_zip, ArchiveEntryOptions::Compressed)
        .with_context(|| {
            format!(
                "Failed archiving .app directory into IPA payload: {}",
                app_dir.display()
            )
        })?;
    ipa.finish()?;
    Ok(())
}
