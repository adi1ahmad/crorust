pub mod axml_encoder;
pub mod manifest_dom;
pub mod merger;

pub use axml_encoder::{compile_manifest, compile_xml, compile_xml_with_symbols};
pub use manifest_dom::{
    ActivityComponentDescriptor, AndroidApplicationDescriptor, AndroidCustomPermissionDescriptor,
    AndroidFeatureDescriptor, AndroidIntentFilterDataDescriptor, AndroidIntentFilterDescriptor,
    AndroidMetaDataDescriptor, AndroidPermissionDescriptor, AndroidProviderDescriptor,
    AndroidSdkDescriptor, AndroidVersionCode, AndroidXmlManifestDescriptor,
};
pub use merger::{collect_aar_file_resources, collect_aar_strings, merge_manifests};

pub fn encode_xml_document_to_axml(
    xml: &str,
    table: &crate::resources::ResourceCatalogTable,
) -> anyhow::Result<crate::protocol::AndroidChunk> {
    compile_xml(xml, table)
}

pub fn synthesize_binary_manifest(
    manifest: &AndroidXmlManifestDescriptor,
) -> anyhow::Result<Vec<u8>> {
    compile_manifest(manifest)
}
