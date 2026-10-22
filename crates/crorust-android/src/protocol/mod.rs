pub mod chunk_stream;
pub mod chunk_writer;
pub mod chunks;
pub mod string_pool;

pub use chunk_stream::{ChunkHeader, ChunkTag, ResourceValueType, TypedResourcePayload};
pub use chunk_writer::{AndroidChunkTag, StreamingChunkWriter};
pub use chunks::{
    AndroidChunk, ResourceIdentifier, ResourcePackageHeader, ResourceTableEntry,
    ResourceTableMapEntry, ResourceTableValue, ResourceTypeHeader, ResourceTypeSpecHeader,
    TypedResourceValue, XmlAttributeRecord, XmlEndElementHeader, XmlNamespaceRecord, XmlNodeHeader,
    XmlStartElementHeader,
};
pub use string_pool::{StringPoolBuilder, RES_STRING_POOL_TYPE, UTF8_FLAG};
