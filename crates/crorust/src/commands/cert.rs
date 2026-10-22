use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};

#[cfg(feature = "platform-apple")]
use app_store_connect::certs_api::CertificateType;

#[derive(Subcommand, Debug, Clone)]
pub enum CertSubcommand {
    /// Generate an Apple App Store Connect unified API key JSON from a private key (.p8)
    #[cfg(feature = "platform-apple")]
    #[clap(name = "apple-api-key")]
    CreateAppleApiKey {
        /// App Store Connect Issuer ID (UUID)
        #[clap(long)]
        issuer_id: String,

        /// App Store Connect Key ID (10-character alphanumeric ID)
        #[clap(long)]
        key_id: String,

        /// Path to the downloaded AuthKey private key (.p8)
        #[clap(long)]
        private_key: PathBuf,

        /// Destination path to write the unified API key JSON
        #[clap(long)]
        api_key: PathBuf,
    },

    /// Fetch and generate an Apple code-signing certificate using an App Store Connect API key
    #[cfg(feature = "platform-apple")]
    #[clap(name = "apple-cert")]
    GenerateAppleCert {
        /// Path to the App Store Connect API key JSON
        #[clap(long)]
        api_key: PathBuf,

        /// Certificate type to generate
        #[clap(long)]
        r#type: CertificateType,

        /// Destination path to write the generated PEM certificate
        #[clap(long)]
        pem: PathBuf,
    },

    /// Inspect an X.509 certificate file (.pem, .cer, .der) and display metadata
    #[clap(name = "inspect")]
    Inspect {
        /// Path to the certificate file to inspect
        file: PathBuf,
    },

    /// Export the built-in development RSA2048 signing certificate to a PEM file
    #[clap(name = "export-debug")]
    ExportDebug {
        /// Platform debug certificate to export: 'android' or 'windows'
        #[clap(long, default_value = "android")]
        platform: String,

        /// Output path for the exported PEM certificate
        #[clap(long)]
        out: PathBuf,
    },
}

pub fn run(action: CertSubcommand) -> Result<()> {
    match action {
        #[cfg(feature = "platform-apple")]
        CertSubcommand::CreateAppleApiKey {
            issuer_id,
            key_id,
            private_key,
            api_key,
        } => {
            create_apple_api_key(&issuer_id, &key_id, &private_key, &api_key)?;
            println!(
                "Successfully generated Apple API key at: {}",
                api_key.display()
            );
        }

        #[cfg(feature = "platform-apple")]
        CertSubcommand::GenerateAppleCert {
            api_key,
            r#type,
            pem,
        } => {
            app_store_connect::certs_api::generate_signing_certificate(&api_key, r#type, &pem)?;
            println!(
                "Successfully generated Apple signing certificate at: {}",
                pem.display()
            );
        }

        CertSubcommand::Inspect { file } => {
            inspect_certificate(&file)?;
        }

        CertSubcommand::ExportDebug { platform, out } => {
            export_debug_certificate(&platform, &out)?;
        }
    }
    Ok(())
}

pub fn create_apple_api_key(
    issuer_id: &str,
    key_id: &str,
    private_key: &Path,
    api_key: &Path,
) -> Result<()> {
    #[cfg(feature = "platform-apple")]
    {
        app_store_connect::UnifiedApiKey::from_ecdsa_pem_path(issuer_id, key_id, private_key)?
            .write_json_file(api_key)?;
        Ok(())
    }
    #[cfg(not(feature = "platform-apple"))]
    {
        let _ = (issuer_id, key_id, private_key, api_key);
        bail!("Apple platform features are disabled in this build");
    }
}

pub fn inspect_certificate(cert_path: &Path) -> Result<()> {
    if !cert_path.exists() {
        bail!("Certificate file not found: {}", cert_path.display());
    }

    let bytes = std::fs::read(cert_path)
        .with_context(|| format!("Failed to read certificate at {}", cert_path.display()))?;

    use sha2::{Digest, Sha256};
    let sha256_hash = Sha256::digest(&bytes);
    let fingerprint = hex::encode(sha256_hash);

    println!("Certificate Inspection: {}", cert_path.display());
    println!("  File Size:     {} bytes", bytes.len());
    println!("  SHA-256 Sum:   {}", fingerprint);

    if let Ok(pem_str) = std::str::from_utf8(&bytes) {
        if pem_str.contains("-----BEGIN") {
            println!("  Encoding:      PEM format");
            for line in pem_str.lines() {
                if line.starts_with("-----BEGIN") {
                    println!("  Header:        {}", line);
                }
            }
        }
    }

    Ok(())
}

pub fn export_debug_certificate(platform: &str, out: &Path) -> Result<()> {
    match platform.to_ascii_lowercase().as_str() {
        "android" | "windows" => {}
        other => {
            bail!(
                "Unknown platform '{}'. Supported platforms: android, windows",
                other
            );
        }
    };

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(
        out,
        crorust_common::identity_keypair::DEFAULT_DEBUG_PEM.as_bytes(),
    )
    .with_context(|| format!("Failed to write debug certificate to {}", out.display()))?;

    println!(
        "Exported built-in {} debug certificate to: {}",
        platform,
        out.display()
    );
    Ok(())
}
