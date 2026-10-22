use anyhow::{Context, Result};
use rasn_pkix::Certificate;
use rsa::pkcs8::DecodePrivateKey;
use rsa::{PaddingScheme, RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone)]
pub struct IdentityKeypair {
    pub valid_span_utc: (i64, i64),
    pub serial_number: u64,
    pub certificate_chain_der: Vec<u8>,
    pub subject_distinguished_name: String,
    key: RsaPrivateKey,
    pubkey: RsaPublicKey,
    cert: Certificate,
}

impl IdentityKeypair {
    pub fn new(pem: &str) -> Result<Self> {
        let pem_blocks =
            pem::parse_many(pem).context("failed to parse PEM blocks for identity keypair")?;

        let key = if let Some(key_pem) = pem_blocks
            .iter()
            .find(|p| p.tag == "PRIVATE KEY" || p.tag == "RSA PRIVATE KEY")
        {
            RsaPrivateKey::from_pkcs8_der(&key_pem.contents)
                .or_else(|_| rsa::pkcs1::DecodeRsaPrivateKey::from_pkcs1_der(&key_pem.contents))
                .context("failed to decode private key from PKCS#8 or PKCS#1 DER")?
        } else {
            anyhow::bail!("no private key found in PEM payload");
        };

        let (cert_der, cert) = if let Some(cert_pem) =
            pem_blocks.iter().find(|p| p.tag == "CERTIFICATE")
        {
            let parsed_cert = rasn::der::decode::<Certificate>(&cert_pem.contents)
                .map_err(|err| anyhow::anyhow!("failed to parse X.509 certificate DER: {err}"))?;
            (cert_pem.contents.clone(), parsed_cert)
        } else {
            anyhow::bail!("no certificate found in PEM payload");
        };

        let pubkey = RsaPublicKey::from(&key);
        let subject_distinguished_name = format!("{:?}", cert.tbs_certificate.subject);

        Ok(Self {
            valid_span_utc: (0, 0),
            serial_number: 1,
            certificate_chain_der: cert_der,
            subject_distinguished_name,
            key,
            pubkey,
            cert,
        })
    }

    pub fn load_identity_or_default(path: Option<&Path>, default_pem: &str) -> Result<Self> {
        match path {
            Some(p) => Self::load_from_pem_file(p),
            None => Self::new(default_pem),
        }
    }

    pub fn default_debug() -> Result<Self> {
        Self::new(DEFAULT_DEBUG_PEM)
    }

    pub fn debug_identity() -> &'static Self {
        static INSTANCE: std::sync::OnceLock<IdentityKeypair> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| {
            Self::default_debug()
                .expect("Built-in CroRust development keypair must parse successfully")
        })
    }

    pub fn load_from_pem_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).with_context(|| {
            format!("failed to read identity keypair PEM at {}", path.display())
        })?;
        Self::new(&content)
    }

    pub fn sign_payload_digest(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let digest = Sha256::digest(bytes);
        let padding = PaddingScheme::new_pkcs1v15_sign::<sha2::Sha256>();
        self.key
            .sign(padding, &digest)
            .context("RSA PKCS#1 v1.5 signing failed")
    }

    pub fn public_key_der(&self) -> &RsaPublicKey {
        &self.pubkey
    }

    pub fn public_key(&self) -> &RsaPublicKey {
        &self.pubkey
    }

    pub fn private_key(&self) -> &RsaPrivateKey {
        &self.key
    }

    pub fn x509_certificate(&self) -> &Certificate {
        &self.cert
    }
}

impl std::fmt::Debug for IdentityKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityKeypair")
            .field("serial_number", &self.serial_number)
            .field("subject", &self.subject_distinguished_name)
            .finish_non_exhaustive()
    }
}

pub const DEFAULT_DEBUG_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIDPjCCAiagAwIBAgIUIfyO8ywPQi0s8y3/qnbfQavWgikwDQYJKoZIhvcNAQEL
BQAwTzELMAkGA1UEBhMCVVMxHDAaBgNVBAoME0Nyb1J1c3QgRGV2ZWxvcG1lbnQx
IjAgBgNVBAMMGUNyb1J1c3QgRGVidWcgQ2VydGlmaWNhdGUwIBcNMjUwMTAxMDAw
MDAwWhgPMjA1NTAxMDEwMDAwMDBaME8xCzAJBgNVBAYTAlVTMRwwGgYDVQQKDBND
cm9SdXN0IERldmVsb3BtZW50MSIwIAYDVQQDDBlDcm9SdXN0IERlYnVnIENlcnRp
ZmljYXRlMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAx8JH1VlvmLsY
p7RogfCouup1AfABXNhnTp7dhK4T6ENZ+4w/OnaQkuqW8RThT309uDfjbMd5gCJK
h4p1BJV5evDK7tj5nHkgVFJnpj7jAsxkMM9T7LXBn3BiyiB9AOKLE85e2lxg6wNY
7eBUzn3bFIe3Q5Ura0ZMymHn3QuT2ndPLHzLQZyeVLWQ0PI2AN+ShR4drySfmfpS
IIiMmKxdXGK2Nm3rh2uMAgfNGxcs1yqSEqe2Z0KHbeMoVK+l4mYJyDguvyO1zu4n
gXYbmqdKOBSMgm4RLqmLziB3j2niU7rjuD0vid8fgtcewHNJD4lSYrjJX4rJ7xy2
z2yofnfqJQIDAQABoxAwDjAMBgNVHRMBAf8EAjAAMA0GCSqGSIb3DQEBCwUAA4IB
AQA0C3RFs8Q2S2ch5HYq0TgKaIXkyWq9zaSI4MBiXvwBIVqHZgJcaHbG3zaKYEhU
NWKm1oEtAKN2YXYTSrZwCUPPB7alxWVKnFuxFYNJz8faefteoJmjFgkM7xSXcKZs
g7jTJ/M52v0h3eo5JuOKMPcBGM9sKM84kbvKpTGXendf5OuNX/HfQ4h2u554aXk+
TksLNPMF3joOqF2hOZPKjXPRmmZJvG0OXya6mZMOjDex6LVAA9Mt/iu6BmR6P9aG
cQz8lGkQehnNh4rxTZi52oVFPDAKmruRJXfuM46V0lY2Ht6xlh8/H1w1IksGcPcG
j+Ijh7ivu+HU3VwrOaTTeAj7
-----END CERTIFICATE-----

-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDHwkfVWW+Yuxin
tGiB8Ki66nUB8AFc2GdOnt2ErhPoQ1n7jD86dpCS6pbxFOFPfT24N+Nsx3mAIkqH
inUElXl68Mru2PmceSBUUmemPuMCzGQwz1PstcGfcGLKIH0A4osTzl7aXGDrA1jt
4FTOfdsUh7dDlStrRkzKYefdC5Pad08sfMtBnJ5UtZDQ8jYA35KFHh2vJJ+Z+lIg
iIyYrF1cYrY2beuHa4wCB80bFyzXKpISp7ZnQodt4yhUr6XiZgnIOC6/I7XO7ieB
dhuap0o4FIyCbhEuqYvOIHePaeJTuuO4PS+J3x+C1x7Ac0kPiVJiuMlfisnvHLbP
bKh+d+olAgMBAAECggEABzVQakWVs4zCJo6isXTI07JRSwZiG4nOOzJGmWjbI30j
lTe7siN9JPOaJsZBqyCkDYPzN6Ic3EpgfFZJYApNq+mXKRLVFiYqkyd9M1dwtTUk
tNjbv8INgGIJ0h665AhlLwwBJnR6etXeTzOZlLgt6sLZLwFbcfanAyvmJ8xFYD4M
tAkjb0hpVovgwcgzqllB0xoeNRsOjztFplmmyeeu308vTQhbI6gB12dVRotmlaJp
xrW8GsZv8Sa+K63SuF59MJlQOregJz84cSyqqE35lUeKllfwfeQuhi2hzJs50MZl
W2DgDqZjBasCFxjf+5Wh9ys63I/CbqLCkESdYBHusQKBgQDoPPBmnUh1EqaWL+qI
k4pEp9xo7o2K36z5PF3O8UGt9M+qGN8vFIr++xMr4fgYsKMYjh+K6FnVFNO7rd1G
bL9pMlVJkgM43mZWgKGmIX3b6vO/ORLShaGkEGK+Czqe3CURnnTr6IbFfaZItALE
T5tj9lySQo1zMHieHCde4E6MhwKBgQDcMpvMRqw72HeNVT9IHgrPJgHRIDHXz81S
8TuNfpdkghoBaUYRy/mx/gYqO7G+ctJmS8P0yUEepIt7A+rwRxe2YeVOrlJC2NTT
/dK+vOHUVGpg4Vrjxbp/Uyhjx8QlchPhyQzukqWtiRWhnrarC279iifwaYogBgFZ
wXTJtkrK8wKBgQCa8r262UnUwsLfBc1HnpjsQcsvP6q0c5RnHj3AqvsVY3eCl+Gg
+eXbRg1+g+ASKRhIc9rbxTbaknDKkbBHIpt7Rpfc5Ey+DrTdFinEPChLKLHEbgH+
8CXu3vfisH9GKr+Yx85e79+ruK/PZjtKW4rj4SXWeA0EQ9zWMRabmLEP1QKBgAKY
BgXN/1FlJ2Ne0bQDxpTSGctg2bV1GTIU7WKpTxc8ARprJwwmnvZdy6zLrzhTGe9i
jRhhFodJ4Oq+DRJvg/P+Czl2DDoFW9xzbPwaNX7Oz8H8ZZmxRpUhYaRrQgA+68UJ
HUHiMFlu9/ZDbFtvcwtBiV7nn1zpNTLdeIa3wwHJAoGANAK09uilYmWAXElIfAzn
DtxnjIRgAR3k/fb1JoRf3KKL7PRDBbJ2jQo3ciP4liEmGy7kd9yxxoTHMkbs1PiH
LGQtIl5hQKRyYNHMxINWOM1xCUzwfO6r4kfNqiU8uY/xmKyKxhazNeT1XL++Kkdk
gubxDfSt3tMzvhbKR8cI5Fo=
-----END PRIVATE KEY-----
"#;
