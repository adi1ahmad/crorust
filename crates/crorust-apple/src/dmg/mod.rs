use anyhow::{Context, Result};
use apple_codesign::dmg::DmgSigner;
use apple_codesign::{SettingsScope, SigningSettings};
use crorust_common::IdentityKeypair;
use std::fs::OpenOptions;
use std::path::Path;

use crate::codesign::prepare_signing_key_and_cert;

pub fn sign_disk_image(path: &Path, signer: &IdentityKeypair, bundle_id: &str) -> Result<()> {
    tracing::info!(dmg = %path.display(), "Signing DMG disk image");
    let mut f = OpenOptions::new().read(true).write(true).open(path)?;
    let mut signing_settings = SigningSettings::default();
    let (key, cert) = prepare_signing_key_and_cert(signer)?;
    signing_settings.set_signing_key(&key, cert);
    signing_settings.chain_apple_certificates();
    signing_settings
        .set_team_id_from_signing_certificate()
        .context("signing certificate is missing team id")?;
    signing_settings.set_time_stamp_url("http://timestamp.apple.com/ts01")?;
    signing_settings.set_binary_identifier(SettingsScope::Main, bundle_id);
    DmgSigner::default().sign_file(&signing_settings, &mut f)?;
    Ok(())
}
