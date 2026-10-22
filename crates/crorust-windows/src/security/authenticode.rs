use anyhow::{ensure, Context, Result};
use crorust_common::identity_keypair::IdentityKeypair;
use rasn::prelude::*;
use rasn_cms::pkcs7_compat::{EncapsulatedContentInfo, SignedData};
use rasn_cms::{
    AlgorithmIdentifier, CertificateChoices, ContentInfo, IssuerAndSerialNumber, SignerIdentifier,
    SignerInfo, CONTENT_SIGNED_DATA,
};
use rasn_pkix::Attribute;
use rsa::{PaddingScheme, PublicKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub const P7X_MAGIC_HEADER: u32 = 0x504b4358;
pub const SPC_INDIRECT_DATA_OID: ConstOid = ConstOid(&[1, 3, 6, 1, 4, 1, 311, 2, 1, 4]);
pub const SPC_SIPINFO_OID: ConstOid = ConstOid(&[1, 3, 6, 1, 4, 1, 311, 2, 1, 30]);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthenticodeDigests {
    pub axci: [u8; 32],

    pub axbm: [u8; 32],

    pub axct: [u8; 32],

    pub axcd: [u8; 32],

    pub axpc: [u8; 32],
}

impl AuthenticodeDigests {
    pub fn read_from_spc(r: &mut impl Read) -> Result<Self> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        ensure!(&magic == b"APPX", "Invalid SpcIndirectData payload magic");

        let mut axpc = [0u8; 32];
        let mut axcd = [0u8; 32];
        let mut axct = [0u8; 32];
        let mut axbm = [0u8; 32];
        let mut axci = [0u8; 32];

        let mut tag = [0u8; 4];
        for _ in 0..5 {
            if r.read_exact(&mut tag).is_err() {
                break;
            }
            match &tag {
                b"AXPC" => r.read_exact(&mut axpc)?,
                b"AXCD" => r.read_exact(&mut axcd)?,
                b"AXCT" => r.read_exact(&mut axct)?,
                b"AXBM" => r.read_exact(&mut axbm)?,
                b"AXCI" => r.read_exact(&mut axci)?,
                _ => break,
            }
        }

        Ok(Self {
            axci,
            axbm,
            axct,
            axcd,
            axpc,
        })
    }

    pub fn encode_spc_payload(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(184);
        out.extend_from_slice(b"APPX");
        out.extend_from_slice(b"AXPC");
        out.extend_from_slice(&self.axpc);
        out.extend_from_slice(b"AXCD");
        out.extend_from_slice(&self.axcd);
        out.extend_from_slice(b"AXCT");
        out.extend_from_slice(&self.axct);
        out.extend_from_slice(b"AXBM");
        out.extend_from_slice(&self.axbm);
        out.extend_from_slice(b"AXCI");
        out.extend_from_slice(&self.axci);
        out
    }
}

#[derive(AsnType, Clone, Debug, Eq, Decode, Encode, PartialEq)]
struct MsixSubjectInterfacePackageContent {
    format_version: u32,
    sip_guid: OctetString,
    reserved_word_1: u32,
    reserved_word_2: u32,
    reserved_word_3: u32,
    reserved_word_4: u32,
    reserved_word_5: u32,
}

impl Default for MsixSubjectInterfacePackageContent {
    fn default() -> Self {
        Self {
            format_version: 0x0101_0000,
            sip_guid: OctetString::from(vec![
                0x4b, 0xdf, 0xc5, 0x0a, 0x07, 0xce, 0xe2, 0x4d, 0xb7, 0x6e, 0x23, 0xc8, 0x39, 0xa0,
                0x9f, 0xd1,
            ]),
            reserved_word_1: 0,
            reserved_word_2: 0,
            reserved_word_3: 0,
            reserved_word_4: 0,
            reserved_word_5: 0,
        }
    }
}

#[derive(AsnType, Clone, Debug, Eq, Decode, Encode, PartialEq)]
struct MsixSubjectInterfacePackage {
    oid: ObjectIdentifier,
    data: MsixSubjectInterfacePackageContent,
}

impl Default for MsixSubjectInterfacePackage {
    fn default() -> Self {
        Self {
            oid: SPC_SIPINFO_OID.into(),
            data: Default::default(),
        }
    }
}

#[derive(AsnType, Clone, Debug, Eq, Decode, Encode, PartialEq)]
struct MsixIndirectDataEnvelopeContent {
    oid: Vec<Open>,
    payload: OctetString,
}

impl MsixIndirectDataEnvelopeContent {
    fn new(raw_bytes: Vec<u8>) -> Self {
        Self {
            oid: vec![
                Open::ObjectIdentifier(
                    Oid::JOINT_ISO_ITU_T_COUNTRY_US_ORGANIZATION_GOV_CSOR_NIST_ALGORITHMS_HASH_SHA256
                        .into(),
                ),
                Open::Null,
            ],
            payload: OctetString::from(raw_bytes),
        }
    }
}

#[derive(AsnType, Clone, Debug, Eq, Decode, Encode, PartialEq)]
struct MsixIndirectDataEnvelope {
    sip_info: MsixSubjectInterfacePackage,
    content: MsixIndirectDataEnvelopeContent,
}

#[derive(AsnType, Clone, Debug, Eq, Decode, Encode, PartialEq)]
struct MsixSignedPayload {
    indirect_data: MsixIndirectDataEnvelope,
}

pub fn compute_authenticode_envelope(
    signer: &IdentityKeypair,
    digests: &AuthenticodeDigests,
) -> Result<Vec<u8>> {
    let appx_payload = MsixSignedPayload {
        indirect_data: MsixIndirectDataEnvelope {
            sip_info: Default::default(),
            content: MsixIndirectDataEnvelopeContent::new(digests.encode_spc_payload()),
        },
    };
    let payload_der = rasn::der::encode(&appx_payload)
        .map_err(|e| anyhow::anyhow!("Failed encoding MsixSignedPayload: {e}"))?;

    let encapsulated_info = EncapsulatedContentInfo {
        content_type: SPC_INDIRECT_DATA_OID.into(),
        content: Any::new(payload_der),
    };

    let pkcs7_signed_data = build_pkcs7_signed_data(signer, encapsulated_info)?;
    let envelope_info = ContentInfo {
        content_type: CONTENT_SIGNED_DATA.into(),
        content: Any::new(
            rasn::der::encode(&pkcs7_signed_data)
                .map_err(|e| anyhow::anyhow!("Failed encoding PKCS#7 signed data: {e}"))?,
        ),
    };

    let mut output_p7x = Vec::new();
    output_p7x.extend_from_slice(&P7X_MAGIC_HEADER.to_be_bytes());
    output_p7x.extend(
        rasn::der::encode(&envelope_info)
            .map_err(|e| anyhow::anyhow!("Failed encoding PKCS#7 content info: {e}"))?,
    );

    Ok(output_p7x)
}

#[allow(clippy::mutable_key_type)]
fn build_pkcs7_signed_data(
    signing_key: &IdentityKeypair,
    encapsulated_info: EncapsulatedContentInfo,
) -> Result<SignedData> {
    let payload_slice = &encapsulated_info.content.as_bytes()[8..];
    let content_digest = Sha256::digest(payload_slice);
    let signing_cert = signing_key.x509_certificate();

    let sha256_alg = AlgorithmIdentifier {
        algorithm:
            Oid::JOINT_ISO_ITU_T_COUNTRY_US_ORGANIZATION_GOV_CSOR_NIST_ALGORITHMS_HASH_SHA256.into(),
        parameters: Some(Any::new(vec![5, 0])),
    };

    let spc_oid = ObjectIdentifier::from(SPC_INDIRECT_DATA_OID);
    let mut type_val_set = BTreeSet::default();
    type_val_set.insert(Any::new(rasn::der::encode(&spc_oid).map_err(|e| {
        anyhow::anyhow!("Failed DER-encoding SpcIndirectData OID: {e}")
    })?));

    let digest_octets = OctetString::from(content_digest.to_vec());
    let mut digest_val_set = BTreeSet::default();
    digest_val_set
        .insert(Any::new(rasn::der::encode(&digest_octets).map_err(
            |e| anyhow::anyhow!("Failed DER-encoding MessageDigest: {e}"),
        )?));

    let mut signed_attrs = SetOf::default();
    signed_attrs.insert(Attribute {
        r#type: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_CONTENT_TYPE.into(),
        values: type_val_set,
    });
    signed_attrs.insert(Attribute {
        r#type: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_MESSAGE_DIGEST.into(),
        values: digest_val_set,
    });

    let der_attrs = rasn::der::encode(&signed_attrs)
        .map_err(|e| anyhow::anyhow!("Failed DER-encoding signed_attrs: {e}"))?;
    let rsa_sig = signing_key
        .sign_payload_digest(&der_attrs)
        .context("Failed signing PKCS#7 signed_attrs for MSIX")?;

    let signer_record = SignerInfo {
        version: 1.into(),
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: signing_cert.tbs_certificate.issuer.clone(),
            serial_number: signing_cert.tbs_certificate.serial_number.clone(),
        }),
        digest_algorithm: sha256_alg.clone(),
        signed_attrs: Some(signed_attrs),
        signature_algorithm: AlgorithmIdentifier {
            algorithm: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS1.into(),
            parameters: Some(Any::new(vec![5, 0])),
        },
        signature: OctetString::from(rsa_sig.to_vec()),
        unsigned_attrs: None,
    };

    Ok(SignedData {
        version: 1.into(),
        digest_algorithms: {
            let mut set = SetOf::default();
            set.insert(sha256_alg);
            set
        },
        encap_content_info: encapsulated_info,
        certificates: Some({
            let mut set = SetOf::default();
            set.insert(CertificateChoices::Certificate(Box::new(
                signing_cert.clone(),
            )));
            set
        }),
        crls: None,
        signer_infos: {
            let mut set = SetOf::default();
            set.insert(signer_record);
            set
        },
    })
}

pub fn parse_authenticode_signature(signature_data: &[u8]) -> Result<AuthenticodeDigests> {
    ensure!(
        signature_data.len() > 4 && signature_data[..4] == P7X_MAGIC_HEADER.to_be_bytes(),
        "Invalid AppxSignature.p7x magic header"
    );

    let info: ContentInfo = rasn::der::decode(&signature_data[4..])
        .map_err(|e| anyhow::anyhow!("Failed decoding ContentInfo: {e}"))?;
    let data: SignedData = rasn::der::decode(info.content.as_bytes())
        .map_err(|e| anyhow::anyhow!("Failed decoding SignedData: {e}"))?;

    let signer_info = data
        .signer_infos
        .iter()
        .next()
        .context("AppxSignature.p7x contains no SignerInfo")?;

    let indirect_body = &data.encap_content_info.content.as_bytes()[8..];
    let indirect_digest = Sha256::digest(indirect_body);

    if let Some(ref certs) = data.certificates {
        if let Some(CertificateChoices::Certificate(cert)) = certs.iter().next() {
            let spki_der = rasn::der::encode(&cert.tbs_certificate.subject_public_key_info)
                .map_err(|e| anyhow::anyhow!("Failed encoding SPKI: {e}"))?;
            let pubkey: RsaPublicKey = rsa::pkcs8::DecodePublicKey::from_public_key_der(&spki_der)
                .context("Failed decoding RSA public key from certificate")?;

            let verify_digest = if let Some(ref attrs) = signer_info.signed_attrs {
                let encoded_attrs = rasn::der::encode(attrs)
                    .map_err(|e| anyhow::anyhow!("Failed encoding signed_attrs: {e}"))?;
                Sha256::digest(&encoded_attrs)
            } else {
                indirect_digest
            };

            let padding = PaddingScheme::new_pkcs1v15_sign::<Sha256>();
            pubkey
                .verify(padding, &verify_digest, signer_info.signature.as_ref())
                .context("AppxSignature.p7x RSA signature verification failed")?;
        }
    }

    let payload: MsixSignedPayload = rasn::der::decode(data.encap_content_info.content.as_bytes())
        .map_err(|e| anyhow::anyhow!("Failed decoding MsixSignedPayload: {e}"))?;

    let mut spc_slice = payload.indirect_data.content.payload.as_ref();
    AuthenticodeDigests::read_from_spc(&mut spc_slice)
}

pub fn read_p7x_from_archive(path: &Path) -> Result<AuthenticodeDigests> {
    let f = File::open(path)?;
    let mut zip = ZipArchive::new(f)?;
    let mut r = zip.by_name("AppxSignature.p7x")?;
    let mut bytes = Vec::new();
    r.read_to_end(&mut bytes)?;
    parse_authenticode_signature(&bytes)
}
