//! # crorust-windows: Pure-Rust Windows MSIX Packaging & Authenticode Engine
//!
//! Assembles Windows MSIX application packages conforming to ECMA-376 Open Packaging
//! Conventions (OPC), computes 64KB block maps, indexes resources via MRM PRI, and applies
//! Authenticode PKCS#7 signatures without Windows SDK tools.
//!
//! ```mermaid
//! flowchart TB
//!     subgraph Assembly["MSIX Container Assembly (OPC)"]
//!         Exe[Windows PE/COFF .exe] --> Cont[WindowsPackageContainer]
//!         Man[AppxManifest.xml] --> Cont
//!         CT["[Content_Types].xml"] --> Cont
//!         PRI[resources.pri] --> Cont
//!     end
//!
//!     subgraph Verification["Block Map & Hashing"]
//!         Cont --> BM["AppxBlockMap.xml<br/><i>64KB SHA-256 block hashes</i>"]
//!     end
//!
//!     subgraph Security["Authenticode Signing"]
//!         BM --> Auth["compute_authenticode_envelope<br/><i>PKCS#7 / CMS Signature</i>"]
//!         Key[RSA Key & X.509 Cert] --> Auth
//!         Auth --> P7X["AppxSignature.p7x"]
//!     end
//!
//!     P7X --> MSIX[Production .msix Package]
//! ```
//!
//! ## Core Modules
//! - [`mrm`]: Modern Resource Management (MRM) PRI file compiler.
//! - [`package`]: Open Packaging Conventions (OPC) container builder and 64KB block map engine.
//! - [`portable`]: Portable Windows executable packaging.
//! - [`security`]: Authenticode PKCS#7 signature construction and verification.

pub mod mrm;
pub mod package;
pub mod portable;
pub mod security;

pub use mrm::{AssetVariantRecord, MrmPriPackage, MrmPriPackageBuilder};
pub use package::{
    assemble_windows_package, hash_payload_blocks_parallel, BlockRecord,
    ManifestApplicationCollection, ManifestApplicationEntry, ManifestCapability,
    ManifestDependencyCollection, ManifestResourceCollection, ManifestResourceLanguage,
    ManifestVisualElements, MsixBlockMapBuilder, MsixBlockMapDescriptor, OpcContentTypesBuilder,
    OpcContentTypesManifest, PackageDisplayProperties, PackageIdentityDescriptor,
    PackageOutputDescriptor, TargetPlatformFamilyConstraint, WindowsPackageContainer,
    WindowsPackageManifestDocument, WindowsPackageSpec, WindowsPayloadEntry, BLOCK_SIZE, DEBUG_PEM,
    EMBEDDED_DEBUG_CERTIFICATE_PEM,
};
pub use security::{
    compute_authenticode_envelope, parse_authenticode_signature, read_p7x_from_archive,
    AuthenticodeDigests,
};
