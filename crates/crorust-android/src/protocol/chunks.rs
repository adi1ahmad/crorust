use super::chunk_stream::ResourceValueType;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom, Write};

#[inline]
fn read_u8(r: &mut impl Read) -> Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

#[inline]
fn read_u16_le(r: &mut impl Read) -> Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

#[inline]
fn read_u32_le(r: &mut impl Read) -> Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

#[inline]
fn read_i32_le(r: &mut impl Read) -> Result<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

#[inline]
fn write_u8(w: &mut impl Write, val: u8) -> Result<()> {
    w.write_all(&[val])?;
    Ok(())
}

#[inline]
fn write_u16_le(w: &mut impl Write, val: u16) -> Result<()> {
    w.write_all(&val.to_le_bytes())?;
    Ok(())
}

#[inline]
fn write_u32_le(w: &mut impl Write, val: u32) -> Result<()> {
    w.write_all(&val.to_le_bytes())?;
    Ok(())
}

#[inline]
fn write_i32_le(w: &mut impl Write, val: i32) -> Result<()> {
    w.write_all(&val.to_le_bytes())?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceIdentifier(pub u32);

impl ResourceIdentifier {
    pub const fn new(pkg_id: u8, type_id: u8, entry_index: u16) -> Self {
        Self(((pkg_id as u32) << 24) | ((type_id as u32) << 16) | (entry_index as u32))
    }

    pub const fn package_id(self) -> u8 {
        ((self.0 & 0xFF00_0000) >> 24) as u8
    }

    pub const fn type_id(self) -> u8 {
        ((self.0 & 0x00FF_0000) >> 16) as u8
    }

    pub const fn entry_index(self) -> u16 {
        (self.0 & 0x0000_FFFF) as u16
    }
}

impl From<u32> for ResourceIdentifier {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl From<ResourceIdentifier> for u32 {
    fn from(r: ResourceIdentifier) -> Self {
        r.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TypedResourceValue {
    pub struct_byte_size: u16,
    pub padding_zero: u8,
    pub data_type_tag: u8,
    pub data_payload: u32,
}

impl TypedResourceValue {
    pub fn new(type_tag: ResourceValueType, payload: u32) -> Self {
        Self {
            struct_byte_size: 8,
            padding_zero: 0,
            data_type_tag: type_tag as u8,
            data_payload: payload,
        }
    }

    pub fn read(r: &mut impl Read) -> Result<Self> {
        let struct_byte_size = read_u16_le(r)?;
        let padding_zero = read_u8(r)?;
        let data_type_tag = read_u8(r)?;
        let data_payload = read_u32_le(r)?;
        Ok(Self {
            struct_byte_size,
            padding_zero,
            data_type_tag,
            data_payload,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        write_u16_le(w, self.struct_byte_size)?;
        write_u8(w, self.padding_zero)?;
        write_u8(w, self.data_type_tag)?;
        write_u32_le(w, self.data_payload)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XmlNodeHeader {
    pub source_line_number: u32,
    pub comment_string_index: i32,
}

impl XmlNodeHeader {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let source_line_number = read_u32_le(r)?;
        let comment_string_index = read_i32_le(r)?;
        Ok(Self {
            source_line_number,
            comment_string_index,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        write_u32_le(w, self.source_line_number)?;
        write_i32_le(w, self.comment_string_index)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XmlNamespaceRecord {
    pub prefix_string_index: i32,
    pub uri_string_index: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XmlStartElementHeader {
    pub namespace_uri_index: i32,
    pub element_name_index: i32,
    pub attr_byte_offset: u16,
    pub attr_record_size: u16,
    pub attr_record_count: u16,
    pub id_attribute_index: u16,
    pub class_attribute_index: u16,
    pub style_attribute_index: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XmlEndElementHeader {
    pub namespace_uri_index: i32,
    pub element_name_index: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XmlAttributeRecord {
    pub namespace_uri_index: i32,
    pub attribute_name_index: i32,
    pub raw_string_index: i32,
    pub typed_value_entry: TypedResourceValue,
}

impl XmlAttributeRecord {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let namespace_uri_index = read_i32_le(r)?;
        let attribute_name_index = read_i32_le(r)?;
        let raw_string_index = read_i32_le(r)?;
        let typed_value_entry = TypedResourceValue::read(r)?;
        Ok(Self {
            namespace_uri_index,
            attribute_name_index,
            raw_string_index,
            typed_value_entry,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        write_i32_le(w, self.namespace_uri_index)?;
        write_i32_le(w, self.attribute_name_index)?;
        write_i32_le(w, self.raw_string_index)?;
        self.typed_value_entry.write(w)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePackageHeader {
    pub package_id: u32,
    pub package_name: String,
    pub type_strings_offset: u32,
    pub last_public_type_id: u32,
    pub key_strings_offset: u32,
    pub last_public_key_id: u32,
    pub type_id_offset_val: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceTypeSpecHeader {
    pub type_id_number: u8,
    pub reserved_zero_byte: u8,
    pub reserved_zero_short: u16,
    pub total_entry_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceTypeHeader {
    pub type_id_number: u8,
    pub configuration_flags: u8,
    pub reserved_word: u16,
    pub total_entry_count: u32,
    pub entries_byte_offset: u32,
    pub config_payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceTableValue {
    Simple(TypedResourceValue),
    Complex(u32, Vec<ResourceTableMapEntry>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceTableMapEntry {
    pub map_key_id: u32,
    pub entry_value: TypedResourceValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceTableEntry {
    pub entry_byte_size: u16,
    pub entry_flags: u16,
    pub key_string_index: u32,
    pub table_value: ResourceTableValue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AndroidChunk {
    Unknown(u16, Vec<u8>),
    StringPool(Vec<String>, Vec<u8>),
    Table(u32, Vec<AndroidChunk>),
    TablePackage(ResourcePackageHeader, Vec<AndroidChunk>),
    TableType(
        ResourceTypeHeader,
        Vec<u32>,
        Vec<Option<ResourceTableEntry>>,
    ),
    TableTypeSpec(ResourceTypeSpecHeader, Vec<u32>),
    Xml(Vec<AndroidChunk>),
    XmlStartNamespace(XmlNodeHeader, XmlNamespaceRecord),
    XmlEndNamespace(XmlNodeHeader, XmlNamespaceRecord),
    XmlStartElement(
        XmlNodeHeader,
        XmlStartElementHeader,
        Vec<XmlAttributeRecord>,
    ),
    XmlEndElement(XmlNodeHeader, XmlEndElementHeader),
    XmlResourceMap(Vec<u32>),
}

impl AndroidChunk {
    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        match self {
            Self::Unknown(chunk_type, data) => {
                write_u16_le(w, *chunk_type)?;
                write_u16_le(w, 8)?;
                write_u32_le(w, 8 + data.len() as u32)?;
                w.write_all(data)?;
                Ok(())
            }
            Self::StringPool(strings, _styles) => {
                let mut pool = super::string_pool::StringPoolBuilder::new_utf8();
                for s in strings {
                    pool.intern(s);
                }
                let bytes = pool.compile()?;
                w.write_all(&bytes)?;
                Ok(())
            }
            Self::XmlResourceMap(ids) => {
                let header_size: u16 = 8;
                let chunk_size = header_size as u32 + (ids.len() * 4) as u32;
                write_u16_le(w, 0x0180)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                for &id in ids {
                    write_u32_le(w, id)?;
                }
                Ok(())
            }
            Self::XmlStartNamespace(node_header, ns) => {
                let header_size: u16 = 16;
                let chunk_size = header_size as u32 + 8;
                write_u16_le(w, 0x0100)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                node_header.write(w)?;
                write_i32_le(w, ns.prefix_string_index)?;
                write_i32_le(w, ns.uri_string_index)?;
                Ok(())
            }
            Self::XmlEndNamespace(node_header, ns) => {
                let header_size: u16 = 16;
                let chunk_size = header_size as u32 + 8;
                write_u16_le(w, 0x0101)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                node_header.write(w)?;
                write_i32_le(w, ns.prefix_string_index)?;
                write_i32_le(w, ns.uri_string_index)?;
                Ok(())
            }
            Self::XmlStartElement(node_header, elem, attrs) => {
                let header_size: u16 = 16;
                let elem_size = 20u32;
                let attrs_size = (attrs.len() * 20) as u32;
                let chunk_size = header_size as u32 + elem_size + attrs_size;

                write_u16_le(w, 0x0102)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                node_header.write(w)?;

                write_i32_le(w, elem.namespace_uri_index)?;
                write_i32_le(w, elem.element_name_index)?;
                write_u16_le(w, elem.attr_byte_offset)?;
                write_u16_le(w, elem.attr_record_size)?;
                write_u16_le(w, attrs.len() as u16)?;
                write_u16_le(w, elem.id_attribute_index)?;
                write_u16_le(w, elem.class_attribute_index)?;
                write_u16_le(w, elem.style_attribute_index)?;

                for attr in attrs {
                    attr.write(w)?;
                }
                Ok(())
            }
            Self::XmlEndElement(node_header, elem) => {
                let header_size: u16 = 16;
                let chunk_size = header_size as u32 + 8;
                write_u16_le(w, 0x0103)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                node_header.write(w)?;
                write_i32_le(w, elem.namespace_uri_index)?;
                write_i32_le(w, elem.element_name_index)?;
                Ok(())
            }
            Self::Xml(children) => {
                let mut body = Vec::new();
                for child in children {
                    child.write(&mut body)?;
                }
                let header_size: u16 = 8;
                let chunk_size = header_size as u32 + body.len() as u32;
                write_u16_le(w, 0x0003)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                w.write_all(&body)?;
                Ok(())
            }
            Self::Table(pkg_count, packages) => {
                let mut body = Vec::new();
                for pkg in packages {
                    pkg.write(&mut body)?;
                }
                let header_size: u16 = 12;
                let chunk_size = header_size as u32 + body.len() as u32;
                write_u16_le(w, 0x0002)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                write_u32_le(w, *pkg_count)?;
                w.write_all(&body)?;
                Ok(())
            }
            Self::TablePackage(header, children) => {
                let mut body = Vec::new();
                for child in children {
                    child.write(&mut body)?;
                }
                let header_size: u16 = 288;
                let chunk_size = header_size as u32 + body.len() as u32;
                write_u16_le(w, 0x0200)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                write_u32_le(w, header.package_id)?;

                let mut name_u16: Vec<u16> = header.package_name.encode_utf16().collect();
                name_u16.resize(128, 0);
                for cu in name_u16 {
                    write_u16_le(w, cu)?;
                }

                write_u32_le(w, header.type_strings_offset)?;
                write_u32_le(w, header.last_public_type_id)?;
                write_u32_le(w, header.key_strings_offset)?;
                write_u32_le(w, header.last_public_key_id)?;
                write_u32_le(w, header.type_id_offset_val)?;
                w.write_all(&body)?;
                Ok(())
            }
            Self::TableType(header, offsets, entries) => {
                let header_size: u16 = (20 + header.config_payload.len()) as u16;
                let mut entries_bytes = Vec::new();
                for e in entries.iter().flatten() {
                    e.write(&mut entries_bytes)?;
                }
                let is_sparse = (header.configuration_flags & 0x01) != 0;
                let is_offset16 = (header.configuration_flags & 0x02) != 0;

                let (offsets_byte_len, pad_len) = if is_offset16 && !is_sparse {
                    let raw_len = offsets.len() * 2;
                    let pad = if raw_len % 4 != 0 {
                        4 - (raw_len % 4)
                    } else {
                        0
                    };
                    (raw_len, pad)
                } else {
                    (offsets.len() * 4, 0)
                };

                let chunk_size = header_size as u32
                    + offsets_byte_len as u32
                    + pad_len as u32
                    + entries_bytes.len() as u32;

                write_u16_le(w, 0x0201)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                write_u8(w, header.type_id_number)?;
                write_u8(w, header.configuration_flags)?;
                write_u16_le(w, header.reserved_word)?;
                write_u32_le(w, header.total_entry_count)?;
                write_u32_le(w, header.entries_byte_offset)?;
                w.write_all(&header.config_payload)?;

                if is_offset16 && !is_sparse {
                    for &offset in offsets {
                        write_u16_le(w, offset as u16)?;
                    }
                    for _ in 0..pad_len {
                        write_u8(w, 0)?;
                    }
                } else {
                    for &offset in offsets {
                        write_u32_le(w, offset)?;
                    }
                }

                w.write_all(&entries_bytes)?;
                Ok(())
            }
            Self::TableTypeSpec(header, flags) => {
                let header_size: u16 = 16;
                let chunk_size = header_size as u32 + (flags.len() * 4) as u32;
                write_u16_le(w, 0x0202)?;
                write_u16_le(w, header_size)?;
                write_u32_le(w, chunk_size)?;
                write_u8(w, header.type_id_number)?;
                write_u8(w, header.reserved_zero_byte)?;
                write_u16_le(w, header.reserved_zero_short)?;
                write_u32_le(w, header.total_entry_count)?;
                for &f in flags {
                    write_u32_le(w, f)?;
                }
                Ok(())
            }
        }
    }

    pub fn parse(reader: &mut (impl Read + Seek)) -> Result<Self> {
        let chunk_base = reader.stream_position()?;
        let chunk_type = read_u16_le(reader)?;
        let header_size = read_u16_le(reader)?;
        let chunk_size = read_u32_le(reader)?;
        reader.seek(SeekFrom::Start(chunk_base))?;

        let result = match chunk_type {
            0x0001 => {
                let pool = super::string_pool::StringPoolBuilder::parse(reader)?;
                Ok(Self::StringPool(pool.strings().to_vec(), Vec::new()))
            }
            0x0002 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let pkg_count = read_u32_le(reader)?;
                let mut packages = Vec::new();
                let end = chunk_base + chunk_size as u64;
                while reader.stream_position()? < end {
                    packages.push(Self::parse(reader)?);
                }
                Ok(Self::Table(pkg_count, packages))
            }
            0x0200 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let id = read_u32_le(reader)?;
                let mut name_units = [0u16; 128];
                for u in &mut name_units {
                    *u = read_u16_le(reader)?;
                }
                let end_name = name_units.iter().position(|&x| x == 0).unwrap_or(128);
                let name = String::from_utf16_lossy(&name_units[..end_name]);
                let type_strings = read_u32_le(reader)?;
                let last_public_type = read_u32_le(reader)?;
                let key_strings = read_u32_le(reader)?;
                let last_public_key = read_u32_le(reader)?;
                let type_id_offset = read_u32_le(reader)?;

                let end = chunk_base + chunk_size as u64;
                let mut children = Vec::new();
                while reader.stream_position()? < end {
                    children.push(Self::parse(reader)?);
                }

                Ok(Self::TablePackage(
                    ResourcePackageHeader {
                        package_id: id,
                        package_name: name,
                        type_strings_offset: type_strings,
                        last_public_type_id: last_public_type,
                        key_strings_offset: key_strings,
                        last_public_key_id: last_public_key,
                        type_id_offset_val: type_id_offset,
                    },
                    children,
                ))
            }
            0x0201 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let id = read_u8(reader)?;
                let flags = read_u8(reader)?;
                let reserved = read_u16_le(reader)?;
                let entry_count = read_u32_le(reader)?;
                let entries_start = read_u32_le(reader)?;
                let config_len = (header_size as usize).saturating_sub(20);
                let mut config = vec![0u8; config_len];
                reader.read_exact(&mut config)?;

                let (offsets, entries) = if (flags & 0x01) != 0 {
                    // FLAG_SPARSE: entry_count pairs of ResTable_sparseTypeEntry (idx: u16, offset_word: u16)
                    let mut raw_offsets = Vec::with_capacity(entry_count as usize);
                    let mut sparse_items = Vec::with_capacity(entry_count as usize);
                    let mut max_idx = 0usize;
                    for _ in 0..entry_count {
                        let idx = read_u16_le(reader)? as usize;
                        let off_word = read_u16_le(reader)?;
                        let raw_val = ((off_word as u32) << 16) | (idx as u32);
                        raw_offsets.push(raw_val);
                        let byte_offset = (off_word as u32) * 4;
                        sparse_items.push((idx, byte_offset));
                        if idx > max_idx {
                            max_idx = idx;
                        }
                    }

                    let mut entries = vec![None; if entry_count > 0 { max_idx + 1 } else { 0 }];
                    for (idx, byte_offset) in sparse_items {
                        if (entries_start as u64 + byte_offset as u64) < chunk_size as u64 {
                            reader.seek(SeekFrom::Start(
                                chunk_base + entries_start as u64 + byte_offset as u64,
                            ))?;
                            entries[idx] = Some(ResourceTableEntry::read(reader)?);
                        }
                    }
                    (raw_offsets, entries)
                } else if (flags & 0x02) != 0 {
                    // FLAG_OFFSET16: entry_count 16-bit offsets scaled by 4, 0xffff is NO_ENTRY
                    let mut raw_offsets16 = Vec::with_capacity(entry_count as usize);
                    for _ in 0..entry_count {
                        raw_offsets16.push(read_u16_le(reader)?);
                    }

                    let mut entries = Vec::with_capacity(entry_count as usize);
                    for &off16 in &raw_offsets16 {
                        if off16 == 0xffff {
                            entries.push(None);
                        } else {
                            let byte_offset = (off16 as u32) * 4;
                            if (entries_start as u64 + byte_offset as u64) < chunk_size as u64 {
                                reader.seek(SeekFrom::Start(
                                    chunk_base + entries_start as u64 + byte_offset as u64,
                                ))?;
                                entries.push(Some(ResourceTableEntry::read(reader)?));
                            } else {
                                entries.push(None);
                            }
                        }
                    }
                    let raw_offsets = raw_offsets16.into_iter().map(|o| o as u32).collect();
                    (raw_offsets, entries)
                } else {
                    // Standard dense 32-bit offsets
                    let mut raw_offsets = Vec::with_capacity(entry_count as usize);
                    for _ in 0..entry_count {
                        raw_offsets.push(read_u32_le(reader)?);
                    }

                    let mut entries = Vec::with_capacity(entry_count as usize);
                    for &offset in &raw_offsets {
                        if offset == 0xffff_ffff {
                            entries.push(None);
                        } else if (entries_start as u64 + offset as u64) < chunk_size as u64 {
                            reader.seek(SeekFrom::Start(
                                chunk_base + entries_start as u64 + offset as u64,
                            ))?;
                            entries.push(Some(ResourceTableEntry::read(reader)?));
                        } else {
                            entries.push(None);
                        }
                    }
                    (raw_offsets, entries)
                };

                Ok(Self::TableType(
                    ResourceTypeHeader {
                        type_id_number: id,
                        configuration_flags: flags,
                        reserved_word: reserved,
                        total_entry_count: entry_count,
                        entries_byte_offset: entries_start,
                        config_payload: config,
                    },
                    offsets,
                    entries,
                ))
            }
            0x0202 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let id = read_u8(reader)?;
                let res0 = read_u8(reader)?;
                let res1 = read_u16_le(reader)?;
                let entry_count = read_u32_le(reader)?;
                let max_possible = (chunk_size.saturating_sub(header_size as u32) / 4) as usize;
                let cap = (entry_count as usize).min(max_possible);
                let mut flags = Vec::with_capacity(cap);
                for _ in 0..entry_count {
                    flags.push(read_u32_le(reader)?);
                }
                Ok(Self::TableTypeSpec(
                    ResourceTypeSpecHeader {
                        type_id_number: id,
                        reserved_zero_byte: res0,
                        reserved_zero_short: res1,
                        total_entry_count: entry_count,
                    },
                    flags,
                ))
            }
            0x0003 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let end = chunk_base + chunk_size as u64;
                let mut children = Vec::new();
                while reader.stream_position()? < end {
                    children.push(Self::parse(reader)?);
                }
                Ok(Self::Xml(children))
            }
            0x0100 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let node_header = XmlNodeHeader::read(reader)?;
                let prefix = read_i32_le(reader)?;
                let uri = read_i32_le(reader)?;
                Ok(Self::XmlStartNamespace(
                    node_header,
                    XmlNamespaceRecord {
                        prefix_string_index: prefix,
                        uri_string_index: uri,
                    },
                ))
            }
            0x0101 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let node_header = XmlNodeHeader::read(reader)?;
                let prefix = read_i32_le(reader)?;
                let uri = read_i32_le(reader)?;
                Ok(Self::XmlEndNamespace(
                    node_header,
                    XmlNamespaceRecord {
                        prefix_string_index: prefix,
                        uri_string_index: uri,
                    },
                ))
            }
            0x0102 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let node_header = XmlNodeHeader::read(reader)?;
                let ns = read_i32_le(reader)?;
                let name = read_i32_le(reader)?;
                let attribute_start = read_u16_le(reader)?;
                let attribute_size = read_u16_le(reader)?;
                let attribute_count = read_u16_le(reader)?;
                let id_index = read_u16_le(reader)?;
                let class_index = read_u16_le(reader)?;
                let style_index = read_u16_le(reader)?;
                let mut attrs = Vec::with_capacity(attribute_count as usize);
                for _ in 0..attribute_count {
                    attrs.push(XmlAttributeRecord::read(reader)?);
                }
                Ok(Self::XmlStartElement(
                    node_header,
                    XmlStartElementHeader {
                        namespace_uri_index: ns,
                        element_name_index: name,
                        attr_byte_offset: attribute_start,
                        attr_record_size: attribute_size,
                        attr_record_count: attribute_count,
                        id_attribute_index: id_index,
                        class_attribute_index: class_index,
                        style_attribute_index: style_index,
                    },
                    attrs,
                ))
            }
            0x0103 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let node_header = XmlNodeHeader::read(reader)?;
                let ns = read_i32_le(reader)?;
                let name = read_i32_le(reader)?;
                Ok(Self::XmlEndElement(
                    node_header,
                    XmlEndElementHeader {
                        namespace_uri_index: ns,
                        element_name_index: name,
                    },
                ))
            }
            0x0180 => {
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                let count = (chunk_size.saturating_sub(8)) / 4;
                let mut ids = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    ids.push(read_u32_le(reader)?);
                }
                Ok(Self::XmlResourceMap(ids))
            }
            _ => {
                let payload_len = (chunk_size as usize).saturating_sub(8);
                let mut data = vec![0u8; payload_len];
                reader.seek(SeekFrom::Start(chunk_base + 8))?;
                reader.read_exact(&mut data)?;
                Ok(Self::Unknown(chunk_type, data))
            }
        };

        reader.seek(SeekFrom::Start(chunk_base + chunk_size as u64))?;
        result
    }
}

impl ResourceTableEntry {
    pub const FLAG_COMPLEX: u16 = 0x0001;
    pub const FLAG_PUBLIC: u16 = 0x0002;
    pub const FLAG_WEAK: u16 = 0x0004;
    pub const FLAG_COMPACT: u16 = 0x0008;

    pub fn read(r: &mut impl Read) -> Result<Self> {
        let first_word = read_u16_le(r)?;
        let entry_flags = read_u16_le(r)?;

        if (entry_flags & Self::FLAG_COMPACT) != 0 {
            let key_string_index = first_word as u32;
            let data_type_tag = (entry_flags >> 8) as u8;
            let data_payload = read_u32_le(r)?;
            let typed_value = TypedResourceValue {
                struct_byte_size: 8,
                padding_zero: 0,
                data_type_tag,
                data_payload,
            };
            return Ok(Self {
                entry_byte_size: 8,
                entry_flags,
                key_string_index,
                table_value: ResourceTableValue::Simple(typed_value),
            });
        }

        let entry_byte_size = first_word;
        let key_string_index = read_u32_le(r)?;
        let table_value = if (entry_flags & Self::FLAG_COMPLEX) != 0 {
            let parent = read_u32_le(r)?;
            let count = read_u32_le(r)?;
            anyhow::ensure!(
                count <= 0x10000,
                "Resource table map entry count ({count}) exceeds maximum allowed 65536"
            );
            let mut maps = Vec::with_capacity(count.min(1024) as usize);
            for _ in 0..count {
                let map_key_id = read_u32_le(r)?;
                let entry_value = TypedResourceValue::read(r)?;
                maps.push(ResourceTableMapEntry {
                    map_key_id,
                    entry_value,
                });
            }
            ResourceTableValue::Complex(parent, maps)
        } else {
            let entry_value = TypedResourceValue::read(r)?;
            ResourceTableValue::Simple(entry_value)
        };
        Ok(Self {
            entry_byte_size,
            entry_flags,
            key_string_index,
            table_value,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        if (self.entry_flags & Self::FLAG_COMPACT) != 0 {
            write_u16_le(w, self.key_string_index as u16)?;
            write_u16_le(w, self.entry_flags)?;
            match &self.table_value {
                ResourceTableValue::Simple(v) => {
                    write_u32_le(w, v.data_payload)?;
                }
                ResourceTableValue::Complex(_, _) => {
                    anyhow::bail!("Compact entries cannot be complex");
                }
            }
            return Ok(());
        }

        write_u16_le(w, self.entry_byte_size)?;
        write_u16_le(w, self.entry_flags)?;
        write_u32_le(w, self.key_string_index)?;
        match &self.table_value {
            ResourceTableValue::Simple(v) => v.write(w)?,
            ResourceTableValue::Complex(parent, maps) => {
                write_u32_le(w, *parent)?;
                write_u32_le(w, maps.len() as u32)?;
                for m in maps {
                    write_u32_le(w, m.map_key_id)?;
                    m.entry_value.write(w)?;
                }
            }
        }
        Ok(())
    }
}
