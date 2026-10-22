//! # crorust-android: Pure-Rust Android Packaging & Signing Engine
//!
//! Assembles Android Application Packages (`.apk`) and Android App Bundles (`.aab`)
//! without requiring the Android SDK, Gradle, `aapt2`, or `apksigner`.
//!
//! ```mermaid
//! flowchart TB
//!     subgraph Inputs["Compiler & Asset Inputs"]
//!         SO["libapp.so (Uncompressed Native Library)"]
//!         XML["AndroidManifest.xml (Plaintext XML)"]
//!         Res["Icons & Asset Files"]
//!     end
//!
//!     subgraph Protocols["Pure-Rust Protocol Encoders"]
//!         AXML["AXML Encoder<br/><i>Binary XML chunk stream</i>"]
//!         ARSC["ARSC Compiler<br/><i>Compiled resource table</i>"]
//!         Align["Boundary Aligner<br/><i>4KB / 16KB mmap page padding</i>"]
//!     end
//!
//!     subgraph Signing["APK Signature Scheme v2"]
//!         Merkle["Rayon Parallel Merkle Tree<br/><i>1MB chunk hashing</i>"]
//!         Block["APK Sig Block 42<br/><i>RSA / ECDSA signature</i>"]
//!     end
//!
//!     SO & XML & Res --> Protocols --> Signing --> APK[Production .apk]
//! ```
//!
//! ## Core Modules
//! - [`manifest`]: Binary XML (`AXML`) encoder, manifest merging, and DOM types.
//! - [`package`]: APK container assembly, boundary alignment, and APK Signature Scheme v2.
//! - [`protocol`]: Android binary chunk format primitives and string pool builders.
//! - [`resources`]: Resource table (`resources.arsc`) compiler and framework symbol catalogs.
//! - [`aab`]: Android App Bundle protocol buffer container generator.

pub mod aab;
pub mod manifest;
pub mod package;
pub mod protocol;
pub mod resources;
pub mod utils;

pub use aab::Aab;
pub use manifest::{
    collect_aar_file_resources, collect_aar_strings, compile_manifest, compile_xml,
    compile_xml_with_symbols, merge_manifests, ActivityComponentDescriptor,
    AndroidApplicationDescriptor, AndroidCustomPermissionDescriptor, AndroidFeatureDescriptor,
    AndroidIntentFilterDataDescriptor, AndroidIntentFilterDescriptor, AndroidMetaDataDescriptor,
    AndroidPermissionDescriptor, AndroidProviderDescriptor, AndroidSdkDescriptor,
    AndroidXmlManifestDescriptor,
};
pub use package::{
    assemble_android_package, build_axml_tree, compute_apk_digest_parallel, find_entry_point,
    inspect_signing_block_envelope, sign_apk_file, verify_apk_boundary_alignments,
    verify_apk_signature, AndroidPackageContainer, AndroidPackageSpec, AppEntryPoint,
    AxmlAttribute, AxmlElement, NativeLibraryArtifact, PackageOutputDescriptor, RayonApkSigner,
    ANDROID_DEX_ALIGNMENT, ANDROID_SO_ALIGNMENT,
};
pub use protocol::{
    AndroidChunk, AndroidChunkTag, ChunkHeader, ChunkTag, ResourceIdentifier, ResourceValueType,
    StreamingChunkWriter, StringPoolBuilder, TypedResourceValue,
};
pub use resources::{
    compile_mipmap_resources, compile_resources, lookup_framework_attribute,
    lookup_framework_color, lookup_framework_id, lookup_framework_resource, MipmapResourceRecord,
    ResourceCatalogEntry, ResourceCatalogTable, ResourceIdentifierReference, ResourceSymbols,
};
pub use utils::{AndroidAbi, AndroidVersionCode};

pub use crorust_common::{ArchiveEntryOptions, Certificate, IdentityKeypair};
pub use zip;
