use anyhow::{Context, Result};
use crorust_common::archive_directory::CentralDirectoryLocator;
use crorust_common::identity_keypair::IdentityKeypair;
use rasn_pkix::Certificate;
use rayon::prelude::*;
use rsa::pkcs8::{DecodePublicKey, EncodePublicKey};
use rsa::{PaddingScheme, PublicKey, RsaPublicKey};
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const APK_SIGNING_BLOCK_MAGIC: &[u8] = b"APK Sig Block 42";
pub const APK_SIGNING_BLOCK_V2_ID: u32 = 0x7109871a;
pub const APK_SIGNING_BLOCK_V3_ID: u32 = 0xf05368c0;
pub const APK_SIGNING_BLOCK_V4_ID: u32 = 0x42726577;
pub const RSA_PKCS1V15_SHA2_256: u32 = 0x0103;
pub const MAX_CHUNK_SIZE: usize = 1024 * 1024;
pub const CHUNK_DIGEST_PREFIX: u8 = 0xa5;
pub const MERKLE_ROOT_PREFIX: u8 = 0x5a;

#[inline]
fn read_u32_le(r: &mut impl Read) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

#[inline]
fn read_u64_le(r: &mut impl Read) -> Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

#[inline]
fn write_u32_le(w: &mut impl Write, val: u32) -> Result<()> {
    w.write_all(&val.to_le_bytes())?;
    Ok(())
}

#[inline]
fn write_u64_le(w: &mut impl Write, val: u64) -> Result<()> {
    w.write_all(&val.to_le_bytes())?;
    Ok(())
}

const STREAM_BATCH_CHUNKS: usize = 16;

#[inline]
fn hash_single_chunk(chunk_data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([CHUNK_DIGEST_PREFIX]);
    let size_le = (chunk_data.len() as u32).to_le_bytes();
    hasher.update(size_le);
    hasher.update(chunk_data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

fn hash_section_streaming<R: Read + Seek>(
    r: &mut R,
    start_offset: u64,
    total_length: u64,
    digests: &mut Vec<[u8; 32]>,
) -> Result<()> {
    r.seek(SeekFrom::Start(start_offset))?;
    let mut remaining = total_length;

    while remaining > 0 {
        let batch_size = ((STREAM_BATCH_CHUNKS * MAX_CHUNK_SIZE) as u64).min(remaining) as usize;
        let mut batch_buffer = vec![0u8; batch_size];
        r.read_exact(&mut batch_buffer)?;
        remaining -= batch_size as u64;

        let batch_slices: Vec<&[u8]> = batch_buffer.chunks(MAX_CHUNK_SIZE).collect();
        let batch_digests: Vec<[u8; 32]> = batch_slices
            .par_iter()
            .map(|chunk| hash_single_chunk(chunk))
            .collect();
        digests.extend(batch_digests);
    }

    Ok(())
}

pub fn compute_apk_digest_parallel<R: Read + Seek>(
    r: &mut R,
    signing_block_start: u64,
    central_directory_start: u64,
    end_of_central_dir_start: u64,
) -> Result<[u8; 32]> {
    let mut chunk_digests: Vec<[u8; 32]> = Vec::new();

    // 1. Process Section 1: 0 .. signing_block_start
    hash_section_streaming(r, 0, signing_block_start, &mut chunk_digests)?;

    // 2. Process Section 2: central_directory_start .. end_of_central_dir_start
    let cd_length = end_of_central_dir_start
        .checked_sub(central_directory_start)
        .context("Invalid central directory bounds")?;
    hash_section_streaming(r, central_directory_start, cd_length, &mut chunk_digests)?;

    // 3. Process Section 3: End of Central Directory record with patched CD offset
    r.seek(SeekFrom::Start(end_of_central_dir_start))?;
    let mut eocd_record = Vec::new();
    r.read_to_end(&mut eocd_record)?;
    if eocd_record.len() >= 20 {
        let mut eocd_cursor = Cursor::new(&mut eocd_record);
        eocd_cursor.seek(SeekFrom::Start(16))?;
        write_u32_le(&mut eocd_cursor, signing_block_start as u32)?;
    }

    for chunk in eocd_record.chunks(MAX_CHUNK_SIZE) {
        chunk_digests.push(hash_single_chunk(chunk));
    }

    // 4. Compute Top-Level Merkle Root
    let mut root_hasher = Sha256::new();
    root_hasher.update([MERKLE_ROOT_PREFIX]);
    let count_le = (chunk_digests.len() as u32).to_le_bytes();
    root_hasher.update(count_le);
    for sub_digest in &chunk_digests {
        root_hasher.update(sub_digest);
    }

    let mut merkle_root = [0u8; 32];
    merkle_root.copy_from_slice(&root_hasher.finalize());
    Ok(merkle_root)
}

pub fn sign_apk_file(target_path: &Path, identity: Option<&IdentityKeypair>) -> Result<()> {
    let debug_fallback;
    let keypair = match identity {
        Some(k) => k,
        None => {
            debug_fallback = IdentityKeypair::debug_identity().clone();
            &debug_fallback
        }
    };

    let mut apk_reader = BufReader::new(File::open(target_path).with_context(|| {
        format!(
            "Failed to open APK for signing: `{}`",
            target_path.display()
        )
    })?);
    let envelope = inspect_signing_block_envelope(&mut apk_reader)?;
    let file_digest = compute_apk_digest_parallel(
        &mut apk_reader,
        envelope.sb_start,
        envelope.cd_start,
        envelope.cde_start,
    )?;

    let mut signing_block_buffer = Vec::new();
    let mut block_cursor = Cursor::new(&mut signing_block_buffer);
    emit_signing_block(&mut block_cursor, file_digest, keypair)?;

    let parent_dir = target_path.parent().unwrap_or_else(|| Path::new("."));
    let temp_apk_path = parent_dir.join(format!(
        ".tmp-sealed-{}-{}",
        std::process::id(),
        target_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.apk")
    ));

    let stream_job: Result<()> = (|| {
        let mut sink = std::io::BufWriter::new(File::create(&temp_apk_path)?);

        apk_reader.seek(SeekFrom::Start(0))?;
        let mut p1 = (&mut apk_reader).take(envelope.sb_start);
        std::io::copy(&mut p1, &mut sink)?;

        sink.write_all(&signing_block_buffer)?;

        let new_cd_offset = sink.stream_position()?;
        apk_reader.seek(SeekFrom::Start(envelope.cd_start))?;
        let cd_total_size = envelope.cde_start - envelope.cd_start;
        let mut p2 = (&mut apk_reader).take(cd_total_size);
        std::io::copy(&mut p2, &mut sink)?;

        let new_cde_offset = sink.stream_position()?;
        apk_reader.seek(SeekFrom::Start(envelope.cde_start))?;
        std::io::copy(&mut apk_reader, &mut sink)?;

        sink.seek(SeekFrom::Start(new_cde_offset + 16))?;
        write_u32_le(&mut sink, new_cd_offset as u32)?;
        sink.flush()?;
        Ok(())
    })();

    if let Err(err) = stream_job {
        let _ = std::fs::remove_file(&temp_apk_path);
        return Err(err).context("Failed streaming signed APK contents");
    }

    if let Err(err) = crorust_common::fs::atomic_replace_file(&temp_apk_path, target_path) {
        let _ = std::fs::remove_file(&temp_apk_path);
        return Err(err).context("Failed atomic replacement of signed APK");
    }

    Ok(())
}

pub fn verify_apk_signature(path: &Path) -> Result<Vec<Certificate>> {
    let mut reader = BufReader::new(File::open(path)?);
    let env_meta = inspect_signing_block_envelope(&mut reader)?;
    let mut found_v2 = None;
    for rec in &env_meta.blocks {
        if rec.id == APK_SIGNING_BLOCK_V2_ID {
            found_v2 = Some(*rec);
            break;
        }
    }
    let target_v2 = if let Some(rec) = found_v2 {
        reader.seek(SeekFrom::Start(rec.start))?;
        read_v2_signature_block(&mut reader)?
    } else {
        anyhow::bail!("no APK v2 signing block found in package");
    };

    let expected_hash = compute_apk_digest_parallel(
        &mut reader,
        env_meta.sb_start,
        env_meta.cd_start,
        env_meta.cde_start,
    )?;
    let mut certs = Vec::new();

    for signer_item in &target_v2.signers {
        anyhow::ensure!(
            !signer_item.signatures.is_empty(),
            "found no signatures in APK v2 signer"
        );
        for sig_item in &signer_item.signatures {
            anyhow::ensure!(
                sig_item.algorithm == RSA_PKCS1V15_SHA2_256,
                "unsupported signature algorithm 0x{:x}",
                sig_item.algorithm
            );
            let public_key = RsaPublicKey::from_public_key_der(&signer_item.public_key)?;
            let hashed_data = Sha256::digest(&signer_item.signed_data);
            let p_scheme = PaddingScheme::new_pkcs1v15_sign::<sha2::Sha256>();
            public_key.verify(p_scheme, &hashed_data, &sig_item.signature)?;
        }

        let mut data_slice = Cursor::new(&signer_item.signed_data[..]);
        let parsed_data = read_signed_data_payload(&mut data_slice)?;
        anyhow::ensure!(
            !parsed_data.digests.is_empty(),
            "found no digests in APK v2 signed data"
        );
        for (algo_id, d_bytes) in &parsed_data.digests {
            anyhow::ensure!(
                *algo_id == RSA_PKCS1V15_SHA2_256,
                "unsupported digest algorithm 0x{:x}",
                algo_id
            );
            anyhow::ensure!(
                d_bytes.as_slice() == expected_hash,
                "APK v2 signature hash mismatch"
            );
        }
        for der in &parsed_data.certificates {
            let c = rasn::der::decode::<Certificate>(der)
                .map_err(|e| anyhow::anyhow!("failed decoding X.509 certificate: {e}"))?;
            certs.push(c);
        }
    }
    Ok(certs)
}

#[derive(Debug, Default)]
pub struct SigningEnvelopeMetadata {
    pub blocks: Vec<AuxiliarySigningRecord>,
    pub sb_start: u64,
    pub cd_start: u64,
    pub cde_start: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct AuxiliarySigningRecord {
    pub id: u32,
    pub start: u64,
}

pub fn inspect_signing_block_envelope<R: Read + Seek>(
    r: &mut R,
) -> Result<SigningEnvelopeMetadata> {
    let zip_loc = CentralDirectoryLocator::new(r)?;
    let mut meta = SigningEnvelopeMetadata {
        cde_start: zip_loc.cde_start,
        cd_start: zip_loc.cd_start,
        ..Default::default()
    };
    if meta.cd_start < 24 {
        meta.sb_start = meta.cd_start;
        return Ok(meta);
    }
    r.seek(SeekFrom::Start(meta.cd_start - 24))?;
    let mut block_size_remaining = read_u64_le(r)?;
    let mut magic_token = [0u8; 16];
    r.read_exact(&mut magic_token)?;
    if magic_token != APK_SIGNING_BLOCK_MAGIC {
        meta.sb_start = meta.cd_start;
        return Ok(meta);
    }
    let mut current_offset = r.seek(SeekFrom::Current(-(block_size_remaining as i64)))?;
    meta.sb_start = current_offset
        .checked_sub(8)
        .context("Malformed APK Signing Block: start offset underflow")?;

    while block_size_remaining > 24 {
        let entry_length = read_u64_le(r)?;
        let entry_id = read_u32_le(r)?;
        meta.blocks.push(AuxiliarySigningRecord {
            id: entry_id,
            start: current_offset + 12,
        });
        current_offset = r.seek(SeekFrom::Start(current_offset + entry_length + 8))?;
        block_size_remaining = block_size_remaining
            .checked_sub(entry_length.saturating_add(8))
            .context("Malformed APK Signing Block: pair length exceeds remaining_size")?;
    }
    Ok(meta)
}

pub fn emit_signing_block<W: Write + Seek>(
    w: &mut W,
    hash: [u8; 32],
    keypair: &IdentityKeypair,
) -> Result<()> {
    let mut payload = Vec::new();
    build_apk_v2_signature_block(hash, keypair, &mut payload)?;
    let total_size = payload.len() as u64 + 36;
    write_u64_le(w, total_size)?;
    write_u64_le(w, payload.len() as u64 + 4)?;
    write_u32_le(w, APK_SIGNING_BLOCK_V2_ID)?;
    w.write_all(&payload)?;
    write_u64_le(w, total_size)?;
    w.write_all(APK_SIGNING_BLOCK_MAGIC)?;
    Ok(())
}

struct ParsedSignedData {
    pub certificates: Vec<Vec<u8>>,
    pub digests: Vec<(u32, Vec<u8>)>,
}

struct ParsedSigner {
    pub signed_data: Vec<u8>,
    pub signatures: Vec<ParsedSig>,
    pub public_key: Vec<u8>,
}

struct ParsedSig {
    pub algorithm: u32,
    pub signature: Vec<u8>,
}

struct ParsedV2Block {
    pub signers: Vec<ParsedSigner>,
}

fn read_v2_signature_block<R: Read>(r: &mut R) -> Result<ParsedV2Block> {
    let mut signers = Vec::new();
    let mut remaining = read_u32_le(r)? as u64;
    while remaining > 0 {
        let signer_length = read_u32_le(r)?;
        remaining = remaining
            .checked_sub((signer_length as u64).saturating_add(4))
            .context("Malformed APK v2 block: signer_size exceeds remaining_size")?;

        let data_len = read_u32_le(r)?;
        let mut signed_data = vec![0u8; data_len as usize];
        r.read_exact(&mut signed_data)?;

        let mut signatures = Vec::new();
        let mut sigs_remaining = read_u32_le(r)?;
        while sigs_remaining > 0 {
            let single_sig_len = read_u32_le(r)?;
            sigs_remaining = sigs_remaining
                .checked_sub(single_sig_len.saturating_add(4))
                .context("Malformed APK v2 block: sig_size exceeds remaining_sig_size")?;
            let algorithm = read_u32_le(r)?;
            let raw_sig_len = read_u32_le(r)?;
            let mut signature = vec![0u8; raw_sig_len as usize];
            r.read_exact(&mut signature)?;
            signatures.push(ParsedSig {
                algorithm,
                signature,
            });
        }

        let key_len = read_u32_le(r)?;
        let mut public_key = vec![0u8; key_len as usize];
        r.read_exact(&mut public_key)?;

        signers.push(ParsedSigner {
            signed_data,
            signatures,
            public_key,
        });
    }
    Ok(ParsedV2Block { signers })
}

fn read_signed_data_payload<R: Read>(r: &mut R) -> Result<ParsedSignedData> {
    let mut digests = Vec::new();
    let mut certificates = Vec::new();

    let mut remaining_digests = read_u32_le(r)?;
    while remaining_digests > 0 {
        let entry_len = read_u32_le(r)?;
        remaining_digests = remaining_digests
            .checked_sub(entry_len.saturating_add(4))
            .context("Malformed APK v2 SignedData: digest length overflow")?;
        let algorithm = read_u32_le(r)?;
        let digest_len = read_u32_le(r)?;
        let mut digest = vec![0u8; digest_len as usize];
        r.read_exact(&mut digest)?;
        digests.push((algorithm, digest));
    }

    let mut remaining_certs = read_u32_le(r)?;
    while remaining_certs > 0 {
        let cert_entry_len = read_u32_le(r)?;
        remaining_certs = remaining_certs
            .checked_sub(cert_entry_len.saturating_add(4))
            .context("Malformed APK v2 SignedData: cert length overflow")?;
        let mut cert = vec![0u8; cert_entry_len as usize];
        r.read_exact(&mut cert)?;
        certificates.push(cert);
    }

    Ok(ParsedSignedData {
        certificates,
        digests,
    })
}

fn build_apk_v2_signature_block(
    hash: [u8; 32],
    keypair: &IdentityKeypair,
    destination: &mut Vec<u8>,
) -> Result<()> {
    let cert_der = rasn::der::encode(keypair.x509_certificate())
        .map_err(|e| anyhow::anyhow!("Failed encoding signing certificate DER: {e}"))?;

    let mut signed_data = Vec::new();
    let digest_entry_size = 4 + 4 + 32;
    write_u32_le(&mut signed_data, digest_entry_size + 4)?;
    write_u32_le(&mut signed_data, digest_entry_size)?;
    write_u32_le(&mut signed_data, RSA_PKCS1V15_SHA2_256)?;
    write_u32_le(&mut signed_data, 32)?;
    signed_data.write_all(&hash)?;

    write_u32_le(&mut signed_data, (cert_der.len() + 4) as u32)?;
    write_u32_le(&mut signed_data, cert_der.len() as u32)?;
    signed_data.write_all(&cert_der)?;

    write_u32_le(&mut signed_data, 0)?;

    let signature = keypair
        .sign_payload_digest(&signed_data)
        .context("failed to compute RSA signature over APK signed data")?;
    let pubkey_der = keypair.public_key().to_public_key_der()?.as_ref().to_vec();

    let mut signer_bytes = Vec::new();
    write_u32_le(&mut signer_bytes, signed_data.len() as u32)?;
    signer_bytes.write_all(&signed_data)?;

    let sig_entry_size = 4 + 4 + signature.len() as u32;
    write_u32_le(&mut signer_bytes, sig_entry_size + 4)?;
    write_u32_le(&mut signer_bytes, sig_entry_size)?;
    write_u32_le(&mut signer_bytes, RSA_PKCS1V15_SHA2_256)?;
    write_u32_le(&mut signer_bytes, signature.len() as u32)?;
    signer_bytes.write_all(&signature)?;

    write_u32_le(&mut signer_bytes, pubkey_der.len() as u32)?;
    signer_bytes.write_all(&pubkey_der)?;

    write_u32_le(destination, signer_bytes.len() as u32 + 4)?;
    write_u32_le(destination, signer_bytes.len() as u32)?;
    destination.write_all(&signer_bytes)?;

    Ok(())
}
