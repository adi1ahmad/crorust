//! # crorust-common: Cross-Cutting Primitives & Systems Utilities
//!
//! Provides memory-alignment ZIP archiving, pure-Rust ELF inspection, Clang VFS overlay
//! synthesis, and cryptographic identity generation.
//!
//! ```mermaid
//! flowchart TB
//!     subgraph Archives["Archive & Alignment"]
//!         Align["AlignedArchiveSink<br/><i>Dynamic page-boundary padding</i>"]
//!         Zip["BoundaryZipArchive<br/><i>Memory-mapped safe ZIP stream</i>"]
//!     end
//!
//!     subgraph Systems["Systems & Compiler Utilities"]
//!         VFS["generate_clang_vfs_overlay<br/><i>Case-insensitive header mapping</i>"]
//!         ELF["read_elf_dt_needed_direct<br/><i>PT_DYNAMIC parsing</i>"]
//!         Link["safe_symlink_or_copy<br/><i>Windows NTFS unprivileged fallback</i>"]
//!     end
//!
//!     subgraph Crypto["Cryptographic Primitives"]
//!         Key["IdentityKeypair<br/><i>RSA-2048 & self-signed X.509</i>"]
//!     end
//! ```
//!
//! ## Core Modules
//! - [`archive_directory`]: Central directory search and EOCD record discovery.
//! - [`boundary_archive`]: Alignment-aware ZIP sink injecting padding into extra fields.
//! - [`elf_resolver`]: Pure-Rust ELF parser querying `DT_NEEDED` dynamic library tags.
//! - [`fs`]: Filesystem utilities, case-fold caching, and Clang YAML VFS overlay emission.
//! - [`identity_keypair`]: RSA PKCS#1 v1.5 signer and self-signed X.509 certificate generation.
//! - [`path`]: Host and target executable path normalization.
//! - [`raster_transform`]: Image resampling and canvas scaling.
//! - [`mod@symlink`]: Cross-platform symlink creation with copy fallback for unprivileged Windows NTFS.
//! - [`temp`]: RAII-guarded temporary path lifecycle manager.

pub mod archive_directory;
pub mod boundary_archive;
pub mod elf_resolver;
pub mod fs;
pub mod identity_keypair;
pub mod path;
pub mod portable;
pub mod process;
pub mod raster_transform;
pub mod symlink;
pub mod temp;
pub mod zip {
    pub use crate::boundary_archive::*;
}

pub use archive_directory::{locate_central_directory_anchor, CentralDirectoryLocator};
pub use boundary_archive::{
    normalize_zip_entry_path, unpack_archive_stream, unpack_archive_stream_with_strip,
    unpack_single_entry_secure, AlignedArchiveSink, ArchiveEntryOffset, ArchiveEntryOptions,
    BoundaryZipArchive, EntryCompressionStrategy, StreamArchiveWriter,
};
pub use elf_resolver::{
    locate_shared_object, query_dt_needed_entries, read_elf_dt_needed_direct,
    scan_directory_for_libraries, trace_dynamic_dependencies, DynamicLibraryResolution,
};
pub use fs::{
    atomic_replace_file, case_fold_lookup, generate_clang_vfs_overlay, hash_file_streaming,
    CaseFoldCache,
};
pub use identity_keypair::IdentityKeypair;
pub use image::imageops::FilterType;
pub use path::{host_executable_name, platform_executable_name, sanitize_path};
pub use portable::assemble_portable;
pub use process::create_platform_command;
pub use raster_transform::{RasterResizer, ResampleConfigurationBuilder, TransformConstraint};
pub use symlink::{copy_dir_all, copy_symlink_entry, safe_symlink_or_copy, symlink};
pub use temp::TempPathGuard;

pub use ::zip::read::ZipFile;
pub use rasn_pkix::Certificate;

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    const TEST_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCjKIEYb/5zSPiE\ndk7DOTo5+xwEHGfq1Nk+vKPs5FeiKsoXLrL55sC4+I/D2ErA+l0xKXPfe78p/qY9\noimria4qyGKliYbsulxJAEfDu3ps7IKJZ4qPVJHJrpONlCI2uioMsByqOB0bbOpW\nQmBODawoQ2Gx4iEdWPSrqKl6IKFzufGxYJ5wybJA9jxp8TzKr1iPMb2CLFtxZb5b\n4cBIGVrB+Iodycf3Islfa2coe3/H5Q1F2xEoCq0wj2V0riIJNEQRNevXuvumm6Bt\nTirqitAKWQC1kk1OBBzy09TnD2qMQpidFhhNK+sXdl0aS8OLdo5SO8SEsYUh76/1\nnxTpE59ZAgMBAAECggEAAgUcewxY6L26qO4ribZ1ynW5yjG9Vh6SPXj5qRNHI/h2\nZczT1az2DhwysHSnMUYwPO1z1CycqGURExKExE1JMRmnwf+mwlvIzHUSzmEt8fcI\nIzIbWKyt/OKazE4NEeMiZeOQyUk2UAp8SLrb5R99dgamiRyg9xNsvQrSpVLN2OhL\nfoP6jQhh1hFZ0ZZ+Ic9KJHwXsPUTrzF+Geq2eIV1iKFtSndvg/9vKyhoUvnw91x7\ntAjMUXPoTTbFXl73JTSNwL2dI1AVf1wZ00g3wfZl6SM4aURwavZyrs0Jwxngx+SU\n5PjNEEA9Nv8eI/7eE1JGLaVkG+YwV11xdS6LuktbQQKBgQDO5wvQU2il6nzvn5Fm\n6H182r/RtdkwRRVQbnqQXV6BGgFjHUBMfAdg+VH229zOpYL2u3KSkJ63u8i5Oinf\n/N04cy9ubOEiam10XXRfWFRmEGxcYOWPYLTYKO8Ywk7E8qTTdRYB/By/Y6CvFkFI\nrsPFbGmS4JxCso+FneGGQ163QQKBgQDJ4BIhwwFo9PXihWpL3oMeOyySIxXf9MRy\nDr+bHiQszh2XR50rC78bYmhSo2GeYlSAJ0HuFf1oxxAVIa7Oytj4rCWvaPd84tvi\nCC+aNxkmkDmw1j2LMx62adQmi81sScuWNOoHjMsH0Tr0sc6/HHrH+fPfmDx3EcaG\n0+TeAo86GQKBgQCib4o62AEEoX/MsL36yPQcHbES9224E8SUDOh0hpk3Si9knQZq\nBcXVQGg1Io/MPO0AwFFG9/tjzC3eHefaTi9vrh7C1wA5gMzF2skOQc6hAqvQKtKP\nKiFQw3JTQdmNjeIRkY7hqAj5yUHRs+s1IN/aEy8ffO+w/XboM/dm7WVjAQKBgBkT\nsYzqGWm9TDIIWqz0BhuiRCYfH5+DW0fD1z9gniPGstxQ9BAd1G6FaeoiC4VON6r3\ntXPq3WC/oZrOZVWg0xUI5WGFy4CC+cd5NtiUUZZ9wfzakCiTgBQuz0C+ixURka2m\nAxBPCAV/kzBESidpVUM0JHw5AAHWfm/PMJX3gW05AoGBAMSa2VqCbYg6NEw5tCoG\nbrZTUgHElNzLDgIdmX0iqwGRVfkh5j/S48keNE1+XFljelmdl7Ivww/OW3LlqUZ5\nCTt3XocnB2uxXtpDwjmRJIBOYkWTAQEf1RnmR7EDSiK4hlDGtvM94ulntdlDSbHI\nlIJvpU0xiFn9EQYl4ORYLfG+\n-----END PRIVATE KEY-----\n-----BEGIN CERTIFICATE-----\nMIIDZzCCAk+gAwIBAgIUFJAR4YTdIM08o/S/gbaVTCgHUWAwDQYJKoZIhvcNAQEL\nBQAwQzELMAkGA1UEBhMCVVMxCzAJBgNVBAgMAkNBMRAwDgYDVQQKDAdDcm9ydXN0\nMRUwEwYDVQQDDAxDcm9ydXN0IFRlc3QwHhcNMjYxMDE3MTQxNjQzWhcNMjcxMDE3\nMTQxNjQzWjBDMQswCQYDVQQGEwJVUzELMAkGA1UECAwCQ0ExEDAOBgNVBAoMB0Ny\nb3J1c3QxFTATBgNVBAMMDENyb3J1c3QgVGVzdDCCASIwDQYJKoZIhvcNAQEBBQAD\nggEPADCCAQoCggEBAKMogRhv/nNI+IR2TsM5Ojn7HAQcZ+rU2T68o+zkV6Iqyhcu\nsvnmwLj4j8PYSsD6XTEpc997vyn+pj2iKauJrirIYqWJhuy6XEkAR8O7emzsgoln\nio9Ukcmuk42UIja6KgywHKo4HRts6lZCYE4NrChDYbHiIR1Y9KuoqXogoXO58bFg\nnnDJskD2PGnxPMqvWI8xvYIsW3FlvlvhwEgZWsH4ih3Jx/ciyV9rZyh7f8flDUXb\nESgKrTCPZXSuIgk0RBE169e6+6aboG1OKuqK0ApZALWSTU4EHPLT1OcPaoxCmJ0W\nGE0r6xd2XRpLw4t2jlI7xISxhSHvr/WfFOkTn1kCAwEAAaNTMFEwHQYDVR0OBBYE\nFNmjVNJJ0Crfyr1hPz2BL1pX6niZMB8GA1UdIwQYMBaAFNmjVNJJ0Crfyr1hPz2B\nL1pX6niZMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQELBQADggEBAC+4M52H\nI3XphHCLoWj5zSumosTQQHWS77iKzxPHoW8Cs9Kag9JsO/Cb012yfItqG3EzdhbJ\n0XIv9BVzb98CAFpss+Ff3LLxZB3B5T0SnOKpvp6XUAJvfYSyHbntDcy0PTN0MZyL\niswjWptSMz0mN2Vme7HTWDM/x3EEf1EQY7fAYWCXBSJm0pjROSbCxBomgGPhxGQa\nM55bxJ5NGWOe4Y+7cbgPtzMzAE/xjQK5poVd7OvtXfhuaAu7Pj5KFfwBXMwh4eD6\nae3kj7a6U1rOWB9bVAZJpN9JBZtYnDewnxOAENxlpGY6TQRMrY8UmjJNVdANbNdi\nP6f3JY0VoYOzw/U=\n-----END CERTIFICATE-----";

    #[test]
    fn test_signer_sign_success_and_verify() {
        let signer = IdentityKeypair::new(TEST_PEM).unwrap();
        let payload = b"Hello, world cryptographic test payload!";
        let sig = signer
            .sign_payload_digest(payload)
            .expect("signing should succeed");
        assert_eq!(sig.len(), 256);

        let digest = sha2::Sha256::digest(payload);
        let padding = rsa::PaddingScheme::new_pkcs1v15_sign::<sha2::Sha256>();
        rsa::PublicKey::verify(signer.public_key_der(), padding, &digest, &sig)
            .expect("signature must verify successfully with public key");
    }

    #[test]
    fn test_signer_missing_key_or_cert_fails() {
        let err_no_key = IdentityKeypair::new(
            "-----BEGIN CERTIFICATE-----\nMIIB...fake\n-----END CERTIFICATE-----\n",
        );
        assert!(err_no_key.is_err(), "Must fail when private key is missing");

        let err_no_cert = IdentityKeypair::new(
            "-----BEGIN RSA PRIVATE KEY-----\nMIIE...fake\n-----END RSA PRIVATE KEY-----\n",
        );
        assert!(
            err_no_cert.is_err(),
            "Must fail when certificate is missing"
        );
    }
}
