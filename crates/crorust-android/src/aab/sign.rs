use anyhow::{Context, Result};
use crorust_common::{ArchiveEntryOptions, IdentityKeypair};
use rasn::prelude::*;
use rasn_cms::pkcs7_compat::{EncapsulatedContentInfo, SignedData};
use rasn_cms::{
    AlgorithmIdentifier, CertificateChoices, ContentInfo, IssuerAndSerialNumber, SignerIdentifier,
    SignerInfo,
};
use rasn_pkix::Attribute;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn generate_jar_signature(
    entries: &BTreeMap<String, (Vec<u8>, ArchiveEntryOptions)>,
    signer: Option<IdentityKeypair>,
) -> Result<Vec<(String, Vec<u8>)>> {
    let signer = signer.unwrap_or_else(|| IdentityKeypair::debug_identity().clone());

    let mut manifest_mf = String::new();
    manifest_mf.push_str("Manifest-Version: 1.0\r\n");
    manifest_mf.push_str("Created-By: 1.0 (crorust)\r\n\r\n");

    let mut entry_digests = BTreeMap::new();

    for (name, (data, _)) in entries {
        if name.starts_with("META-INF/") {
            continue;
        }
        let digest = Sha256::digest(data);
        let b64_digest = base64::encode(digest);

        let section = format!("Name: {}\r\nSHA-256-Digest: {}\r\n\r\n", name, b64_digest);
        manifest_mf.push_str(&section);

        let section_digest = Sha256::digest(section.as_bytes());
        entry_digests.insert(name.clone(), base64::encode(section_digest));
    }

    let manifest_mf_bytes = manifest_mf.into_bytes();
    let manifest_digest = Sha256::digest(&manifest_mf_bytes);

    let mut cert_sf = String::new();
    cert_sf.push_str("Signature-Version: 1.0\r\n");
    cert_sf.push_str("Created-By: 1.0 (crorust)\r\n");
    cert_sf.push_str(&format!(
        "SHA-256-Digest-Manifest: {}\r\n\r\n",
        base64::encode(manifest_digest)
    ));

    for (name, section_b64) in entry_digests {
        cert_sf.push_str(&format!(
            "Name: {}\r\nSHA-256-Digest: {}\r\n\r\n",
            name, section_b64
        ));
    }

    let cert_sf_bytes = cert_sf.into_bytes();

    let cert_rsa_bytes = build_jar_pkcs7(&signer, &cert_sf_bytes)?;

    Ok(vec![
        ("META-INF/MANIFEST.MF".to_string(), manifest_mf_bytes),
        ("META-INF/CERT.SF".to_string(), cert_sf_bytes),
        ("META-INF/CERT.RSA".to_string(), cert_rsa_bytes),
    ])
}

fn build_jar_pkcs7(signer: &IdentityKeypair, data_to_sign: &[u8]) -> Result<Vec<u8>> {
    let digest = Sha256::digest(data_to_sign);
    let cert = signer.x509_certificate();

    let digest_algorithm = AlgorithmIdentifier {
        algorithm:
            Oid::JOINT_ISO_ITU_T_COUNTRY_US_ORGANIZATION_GOV_CSOR_NIST_ALGORITHMS_HASH_SHA256.into(),
        parameters: Some(Any::new(vec![5, 0])),
    };

    let content_type_oid = ObjectIdentifier::from(Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS7_DATA);
    let content_type_der = rasn::der::encode(&content_type_oid)
        .map_err(|e| anyhow::anyhow!("Failed DER-encoding PKCS#7 ContentType OID: {e}"))?;
    let mut content_type_values = BTreeSet::default();
    content_type_values.insert(Any::new(content_type_der));

    let digest_octet = OctetString::from(digest.to_vec());
    let digest_octet_der = rasn::der::encode(&digest_octet)
        .map_err(|e| anyhow::anyhow!("Failed DER-encoding PKCS#7 MessageDigest: {e}"))?;
    let mut digest_values = BTreeSet::default();
    digest_values.insert(Any::new(digest_octet_der));

    let mut signed_attrs = SetOf::default();
    signed_attrs.insert(Attribute {
        r#type: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_CONTENT_TYPE.into(),
        values: content_type_values,
    });
    signed_attrs.insert(Attribute {
        r#type: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_MESSAGE_DIGEST.into(),
        values: digest_values,
    });

    let encoded_signed_attrs = rasn::der::encode(&signed_attrs)
        .map_err(|e| anyhow::anyhow!("Failed DER-encoding signedAttributes set: {e}"))?;
    let signature = signer
        .sign_payload_digest(&encoded_signed_attrs)
        .context("Failed signing DER-encoded signedAttributes for JAR signature")?;

    let signer_info = SignerInfo {
        version: 1.into(),
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer.clone(),
            serial_number: cert.tbs_certificate.serial_number.clone(),
        }),
        digest_algorithm: digest_algorithm.clone(),
        signed_attrs: Some(signed_attrs),
        signature_algorithm: AlgorithmIdentifier {
            algorithm: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS1.into(),
            parameters: Some(Any::new(vec![5, 0])),
        },
        signature: OctetString::from(signature),
        unsigned_attrs: None,
    };

    let encap_content_info = EncapsulatedContentInfo {
        content_type: ObjectIdentifier::from(Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS7_DATA),
        content: Any::new(
            rasn::der::encode(&OctetString::from(data_to_sign.to_vec()))
                .map_err(|e| anyhow::anyhow!("Failed DER-encoding EncapsulatedContentInfo: {e}"))?,
        ),
    };

    let signed_data = SignedData {
        version: 1.into(),
        digest_algorithms: {
            let mut digest_algorithms = SetOf::default();
            digest_algorithms.insert(digest_algorithm);
            digest_algorithms
        },
        encap_content_info,
        certificates: Some({
            let mut certs = SetOf::default();
            certs.insert(CertificateChoices::Certificate(Box::new(cert.clone())));
            certs
        }),
        crls: None,
        signer_infos: {
            let mut signer_infos = SetOf::default();
            signer_infos.insert(signer_info);
            signer_infos
        },
    };

    let signed_data_der = rasn::der::encode(&signed_data)
        .map_err(|e| anyhow::anyhow!("Failed encoding PKCS#7 SignedData: {e}"))?;
    let content_info = ContentInfo {
        content_type: rasn_cms::CONTENT_SIGNED_DATA.into(),
        content: Any::new(signed_data_der),
    };

    rasn::der::encode(&content_info)
        .map_err(|e| anyhow::anyhow!("Failed encoding PKCS#7 ContentInfo: {e}"))
}

pub fn verify_jar_pkcs7(cert_rsa_bytes: &[u8], cert_sf_bytes: &[u8]) -> Result<()> {
    use rsa::{PaddingScheme, PublicKey, RsaPublicKey};

    let content_info: ContentInfo = rasn::der::decode(cert_rsa_bytes)
        .map_err(|e| anyhow::anyhow!("Failed decoding ContentInfo from CERT.RSA: {e}"))?;
    let signed_data: SignedData = rasn::der::decode(content_info.content.as_bytes())
        .map_err(|e| anyhow::anyhow!("Failed decoding SignedData from CERT.RSA: {e}"))?;

    let signer_info = signed_data
        .signer_infos
        .iter()
        .next()
        .context("CERT.RSA SignedData contains no SignerInfo")?;

    let signed_attrs = signer_info
        .signed_attrs
        .as_ref()
        .context("CERT.RSA SignerInfo missing signed_attrs")?;

    let expected_sf_digest = Sha256::digest(cert_sf_bytes);
    let msg_digest_oid =
        ObjectIdentifier::from(Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_MESSAGE_DIGEST);
    let mut found_matching_digest = false;
    for attr in signed_attrs.iter() {
        if attr.r#type == msg_digest_oid {
            if let Some(any_val) = attr.values.iter().next() {
                let octet: OctetString = rasn::der::decode(any_val.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Invalid MessageDigest attribute DER: {e}"))?;
                anyhow::ensure!(
                    octet.as_ref() == expected_sf_digest.as_slice(),
                    "MessageDigest attribute in CERT.RSA does not match SHA-256 of CERT.SF"
                );
                found_matching_digest = true;
            }
        }
    }
    anyhow::ensure!(
        found_matching_digest,
        "CERT.RSA signed_attrs missing PKCS#9 MessageDigest attribute"
    );

    let certs = signed_data
        .certificates
        .as_ref()
        .context("CERT.RSA contains no embedded certificates")?;
    let cert = match certs
        .iter()
        .next()
        .context("Empty certificate set in CERT.RSA")?
    {
        CertificateChoices::Certificate(c) => c,
        _ => anyhow::bail!("Unsupported certificate choice in CERT.RSA"),
    };

    let spki_der = rasn::der::encode(&cert.tbs_certificate.subject_public_key_info)
        .map_err(|e| anyhow::anyhow!("Failed re-encoding SubjectPublicKeyInfo: {e}"))?;
    let pubkey: RsaPublicKey = rsa::pkcs8::DecodePublicKey::from_public_key_der(&spki_der)
        .context("Failed decoding RSA public key from X.509 certificate")?;

    let encoded_attrs = rasn::der::encode(signed_attrs)
        .map_err(|e| anyhow::anyhow!("Failed DER-encoding signed_attrs for verification: {e}"))?;
    let attrs_digest = Sha256::digest(&encoded_attrs);
    let padding = PaddingScheme::new_pkcs1v15_sign::<Sha256>();
    pubkey
        .verify(padding, &attrs_digest, signer_info.signature.as_ref())
        .context("RSA signature verification over DER-encoded signed_attrs failed")?;

    Ok(())
}
