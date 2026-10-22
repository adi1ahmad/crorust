pub mod apk_container;
pub mod authenticity;
pub mod boundary_align;
pub mod container;
pub mod rayon_merkle;

pub use apk_container::{
    build_axml_tree, find_entry_point, AndroidPackageContainer, AppEntryPoint, AxmlAttribute,
    AxmlElement,
};
pub use authenticity::{
    compute_apk_digest_parallel, inspect_signing_block_envelope, sign_apk_file,
    verify_apk_signature, AuxiliarySigningRecord, SigningEnvelopeMetadata,
};
pub use boundary_align::{
    verify_apk_boundary_alignments, ANDROID_DEX_ALIGNMENT, ANDROID_SO_ALIGNMENT,
};
pub use container::{
    assemble_android_package, AndroidPackageSpec, NativeLibraryArtifact, PackageOutputDescriptor,
};
pub use rayon_merkle::RayonApkSigner;
