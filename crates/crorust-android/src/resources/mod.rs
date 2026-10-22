pub mod attribute_index;
pub mod symbol_catalog;
pub mod table_compiler;

pub use attribute_index::{
    lookup_framework_attribute, lookup_framework_color, lookup_framework_id,
    lookup_framework_resource,
};
pub use symbol_catalog::{ResourceIdentifierReference, ResourceSymbols};
pub use table_compiler::{
    compile_mipmap_resources, compile_resources, MipmapResourceRecord, ResourceCatalogEntry,
    ResourceCatalogTable,
};
