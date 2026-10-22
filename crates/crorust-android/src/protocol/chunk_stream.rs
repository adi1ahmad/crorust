use anyhow::{bail, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u16)]
pub enum ChunkTag {
    Null = 0x0000,
    StringPool = 0x0001,
    Table = 0x0002,
    Xml = 0x0003,
    XmlStartNamespace = 0x0100,
    XmlEndNamespace = 0x0101,
    XmlStartElement = 0x0102,
    XmlEndElement = 0x0103,
    XmlCdata = 0x0104,
    XmlResourceMap = 0x0180,
    TablePackage = 0x0200,
    TableType = 0x0201,
    TableTypeSpec = 0x0202,
    TableLibrary = 0x0203,
}

impl ChunkTag {
    pub fn parse(code: u16) -> Result<Self> {
        match code {
            0x0000 => Ok(Self::Null),
            0x0001 => Ok(Self::StringPool),
            0x0002 => Ok(Self::Table),
            0x0003 => Ok(Self::Xml),
            0x0100 => Ok(Self::XmlStartNamespace),
            0x0101 => Ok(Self::XmlEndNamespace),
            0x0102 => Ok(Self::XmlStartElement),
            0x0103 => Ok(Self::XmlEndElement),
            0x0104 => Ok(Self::XmlCdata),
            0x0180 => Ok(Self::XmlResourceMap),
            0x0200 => Ok(Self::TablePackage),
            0x0201 => Ok(Self::TableType),
            0x0202 => Ok(Self::TableTypeSpec),
            0x0203 => Ok(Self::TableLibrary),
            other => bail!("Unrecognized Android chunk tag: 0x{:04x}", other),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ChunkHeader {
    pub tag: ChunkTag,
    pub header_size: u16,
    pub chunk_size: u32,
}

impl ChunkHeader {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let code = r.read_u16::<LittleEndian>()?;
        let tag = ChunkTag::parse(code)?;
        let header_size = r.read_u16::<LittleEndian>()?;
        let chunk_size = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            tag,
            header_size,
            chunk_size,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u16::<LittleEndian>(self.tag as u16)?;
        w.write_u16::<LittleEndian>(self.header_size)?;
        w.write_u32::<LittleEndian>(self.chunk_size)?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ResourceValueType {
    Null = 0x00,
    Reference = 0x01,
    Attribute = 0x02,
    String = 0x03,
    Float = 0x04,
    Dimension = 0x05,
    Fraction = 0x06,
    DynamicReference = 0x07,
    DynamicAttribute = 0x08,
    IntDec = 0x10,
    IntHex = 0x11,
    IntBoolean = 0x12,
    IntColorArgb8 = 0x1c,
    IntColorRgb8 = 0x1d,
    IntColorArgb4 = 0x1e,
    IntColorRgb4 = 0x1f,
}

impl ResourceValueType {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0x01 => Self::Reference,
            0x02 => Self::Attribute,
            0x03 => Self::String,
            0x04 => Self::Float,
            0x05 => Self::Dimension,
            0x06 => Self::Fraction,
            0x07 => Self::DynamicReference,
            0x08 => Self::DynamicAttribute,
            0x10 => Self::IntDec,
            0x11 => Self::IntHex,
            0x12 => Self::IntBoolean,
            0x1c => Self::IntColorArgb8,
            0x1d => Self::IntColorRgb8,
            0x1e => Self::IntColorArgb4,
            0x1f => Self::IntColorRgb4,
            _ => Self::Null,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TypedResourcePayload {
    pub payload_byte_size: u16,
    pub format_padding: u8,
    pub value_data_type: u8,
    pub raw_data_payload: u32,
}

impl TypedResourcePayload {
    pub fn read(r: &mut impl Read) -> Result<Self> {
        let payload_byte_size = r.read_u16::<LittleEndian>()?;
        let format_padding = r.read_u8()?;
        let value_data_type = r.read_u8()?;
        let raw_data_payload = r.read_u32::<LittleEndian>()?;
        Ok(Self {
            payload_byte_size,
            format_padding,
            value_data_type,
            raw_data_payload,
        })
    }

    pub fn write(&self, w: &mut impl Write) -> Result<()> {
        w.write_u16::<LittleEndian>(self.payload_byte_size)?;
        w.write_u8(self.format_padding)?;
        w.write_u8(self.value_data_type)?;
        w.write_u32::<LittleEndian>(self.raw_data_payload)?;
        Ok(())
    }
}
