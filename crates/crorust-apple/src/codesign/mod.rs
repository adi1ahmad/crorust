use anyhow::{Context, Result};
use apple_codesign::notarization::{
    notary_api::SubmissionResponseStatus, NotarizationUpload, Notarizer,
};
use apple_codesign::stapling::Stapler;
use crorust_common::IdentityKeypair;
use pkcs8::EncodePrivateKey;
use std::path::Path;
use std::time::{Duration, Instant};
use x509_certificate::{CapturedX509Certificate, InMemorySigningKeyPair};

pub const NOTARIZE_TIMEOUT: Duration = Duration::from_secs(900);

pub fn prepare_signing_key_and_cert(
    signer: &IdentityKeypair,
) -> Result<(InMemorySigningKeyPair, CapturedX509Certificate)> {
    let cert_der = rasn::der::encode(signer.x509_certificate())
        .map_err(|e| anyhow::anyhow!("Failed DER-encoding X.509 signing certificate: {e}"))?;
    let cert = CapturedX509Certificate::from_der(cert_der)?;
    let secret = signer
        .private_key()
        .to_pkcs8_der()
        .map_err(|e| anyhow::anyhow!("Failed encoding RSA private key to PKCS#8 DER: {e}"))?;
    let key = InMemorySigningKeyPair::from_pkcs8_der(secret.as_bytes())?;
    Ok((key, cert))
}

pub fn extract_bundle_identifier_from_package(bundle: &Path) -> Result<String> {
    let candidate = bundle.join("Contents").join("Info.plist");
    let plist_path = if candidate.is_file() {
        candidate
    } else {
        bundle.join("Info.plist")
    };
    let bytes = std::fs::read(&plist_path)
        .with_context(|| format!("Failed to read property list at {}", plist_path.display()))?;
    let val: plist::Value = plist::from_reader_xml(&*bytes)
        .with_context(|| format!("Failed parsing Info.plist XML at {}", plist_path.display()))?;
    val.as_dictionary()
        .and_then(|dict| dict.get("CFBundleIdentifier"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .context("CFBundleIdentifier not found in Info.plist")
}

pub fn submit_artifact_for_apple_notarization(path: &Path, api_key: &Path) -> Result<()> {
    tracing::info!(path = %path.display(), "Submitting artifact for Apple notarization");
    let notarizer = Notarizer::from_api_key(api_key)?;
    let submission_id = match notarizer.notarize_path(path, None)? {
        NotarizationUpload::UploadId(id) => id,
        NotarizationUpload::NotaryResponse(resp) => resp.data.id,
    };
    tracing::info!(submission_id = %submission_id, "Apple notarization upload accepted");
    let start_time = Instant::now();
    loop {
        let resp = notarizer.get_submission(&submission_id)?;
        let status = resp.data.attributes.status;
        let elapsed = start_time.elapsed();
        tracing::debug!(
            elapsed_secs = elapsed.as_secs(),
            status = ?status,
            "Polled notarization status"
        );
        if status != SubmissionResponseStatus::InProgress {
            let log = notarizer.fetch_notarization_log(&submission_id)?;
            tracing::info!(target: "crorust_apple::notarize::log", "Apple notarization log:\n{log}");
            resp.into_result()?;
            break;
        }
        anyhow::ensure!(
            elapsed < NOTARIZE_TIMEOUT,
            "Apple notarization timed out after {} seconds (submission_id: {})",
            elapsed.as_secs(),
            submission_id
        );
        std::thread::sleep(Duration::from_secs(3));
    }
    let stapler = Stapler::new()?;
    stapler.staple_path(path)?;
    Ok(())
}
