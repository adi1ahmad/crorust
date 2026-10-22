use anyhow::{ensure, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};

pub const RES_STRING_POOL_TYPE: u16 = 0x0001;
pub const UTF8_FLAG: u32 = 1 << 8;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringPoolBuilder {
    strings: Vec<String>,
    index_cache: BTreeMap<String, u32>,
    utf8: bool,
}

impl StringPoolBuilder {
    pub fn new_utf8() -> Self {
        Self {
            strings: Vec::new(),
            index_cache: BTreeMap::new(),
            utf8: true,
        }
    }

    pub fn new_utf16() -> Self {
        Self {
            strings: Vec::new(),
            index_cache: BTreeMap::new(),
            utf8: false,
        }
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&idx) = self.index_cache.get(s) {
            return idx;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index_cache.insert(s.to_string(), idx);
        idx
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn strings(&self) -> &[String] {
        &self.strings
    }

    pub fn compile(&self) -> Result<Vec<u8>> {
        let mut string_bytes_data = Vec::new();
        let mut offsets = Vec::with_capacity(self.strings.len());

        for s in &self.strings {
            offsets.push(string_bytes_data.len() as u32);
            if self.utf8 {
                let bytes = s.as_bytes();
                let char_len = s.chars().count();

                if char_len <= 127 {
                    string_bytes_data.write_u8(char_len as u8)?;
                } else {
                    string_bytes_data.write_u8(((char_len >> 8) as u8) | 0x80)?;
                    string_bytes_data.write_u8((char_len & 0xff) as u8)?;
                }

                if bytes.len() <= 127 {
                    string_bytes_data.write_u8(bytes.len() as u8)?;
                } else {
                    string_bytes_data.write_u8(((bytes.len() >> 8) as u8) | 0x80)?;
                    string_bytes_data.write_u8((bytes.len() & 0xff) as u8)?;
                }

                string_bytes_data.write_all(bytes)?;
                string_bytes_data.write_u8(0)?;
            } else {
                let utf16: Vec<u16> = s.encode_utf16().collect();
                let u16_len = utf16.len();
                if u16_len <= 32767 {
                    string_bytes_data.write_u16::<LittleEndian>(u16_len as u16)?;
                } else {
                    string_bytes_data
                        .write_u16::<LittleEndian>(((u16_len >> 16) as u16) | 0x8000)?;
                    string_bytes_data.write_u16::<LittleEndian>((u16_len & 0xffff) as u16)?;
                }
                for code_unit in utf16 {
                    string_bytes_data.write_u16::<LittleEndian>(code_unit)?;
                }
                string_bytes_data.write_u16::<LittleEndian>(0)?;
            }
        }

        while string_bytes_data.len() % 4 != 0 {
            string_bytes_data.push(0);
        }

        let header_size: u16 = 28;
        let string_count = self.strings.len() as u32;
        let flags: u32 = if self.utf8 { UTF8_FLAG } else { 0 };
        let strings_start = (header_size as usize + offsets.len() * 4) as u32;
        let total_size = strings_start + string_bytes_data.len() as u32;

        let mut out = Vec::new();
        out.write_u16::<LittleEndian>(RES_STRING_POOL_TYPE)?;
        out.write_u16::<LittleEndian>(header_size)?;
        out.write_u32::<LittleEndian>(total_size)?;
        out.write_u32::<LittleEndian>(string_count)?;
        out.write_u32::<LittleEndian>(0)?;
        out.write_u32::<LittleEndian>(flags)?;
        out.write_u32::<LittleEndian>(strings_start)?;
        out.write_u32::<LittleEndian>(0)?;

        for offset in offsets {
            out.write_u32::<LittleEndian>(offset)?;
        }
        out.write_all(&string_bytes_data)?;

        Ok(out)
    }

    pub fn parse(reader: &mut (impl Read + Seek)) -> Result<Self> {
        let chunk_type = reader.read_u16::<LittleEndian>()?;
        ensure!(
            chunk_type == RES_STRING_POOL_TYPE,
            "Expected StringPool chunk (0x0001), got 0x{:04x}",
            chunk_type
        );
        let header_size = reader.read_u16::<LittleEndian>()?;
        let _chunk_size = reader.read_u32::<LittleEndian>()?;
        let string_count = reader.read_u32::<LittleEndian>()? as usize;
        let _style_count = reader.read_u32::<LittleEndian>()?;
        let flags = reader.read_u32::<LittleEndian>()?;
        let strings_start = reader.read_u32::<LittleEndian>()? as u64;
        let _styles_start = reader.read_u32::<LittleEndian>()?;

        let is_utf8 = (flags & UTF8_FLAG) != 0;

        let mut offsets = Vec::with_capacity(string_count);
        for _ in 0..string_count {
            offsets.push(reader.read_u32::<LittleEndian>()?);
        }

        let chunk_base_offset =
            reader.stream_position()? - (header_size as u64 + (string_count * 4) as u64);
        let absolute_strings_start = chunk_base_offset + strings_start;

        let mut strings = Vec::with_capacity(string_count);
        let mut index_cache = BTreeMap::new();

        for (i, offset) in offsets.into_iter().enumerate() {
            reader.seek(SeekFrom::Start(absolute_strings_start + offset as u64))?;
            let s = if is_utf8 {
                read_utf8_string(reader)?
            } else {
                read_utf16_string(reader)?
            };
            index_cache.insert(s.clone(), i as u32);
            strings.push(s);
        }

        reader.seek(SeekFrom::Start(chunk_base_offset + _chunk_size as u64))?;

        Ok(Self {
            strings,
            index_cache,
            utf8: is_utf8,
        })
    }
}

fn read_utf8_string(reader: &mut impl Read) -> Result<String> {
    let mut b1 = reader.read_u8()?;
    let _char_len = if (b1 & 0x80) != 0 {
        let b2 = reader.read_u8()?;
        ((b1 as usize & 0x7f) << 8) | (b2 as usize)
    } else {
        b1 as usize
    };

    b1 = reader.read_u8()?;
    let byte_len = if (b1 & 0x80) != 0 {
        let b2 = reader.read_u8()?;
        ((b1 as usize & 0x7f) << 8) | (b2 as usize)
    } else {
        b1 as usize
    };

    let mut bytes = vec![0u8; byte_len];
    reader.read_exact(&mut bytes)?;
    let _null = reader.read_u8()?;
    String::from_utf8(bytes).context("Invalid UTF-8 in StringPool")
}

fn read_utf16_string(reader: &mut impl Read) -> Result<String> {
    let u1 = reader.read_u16::<LittleEndian>()?;
    let len = if (u1 & 0x8000) != 0 {
        let u2 = reader.read_u16::<LittleEndian>()?;
        ((u1 as usize & 0x7fff) << 16) | (u2 as usize)
    } else {
        u1 as usize
    };

    let mut units = Vec::with_capacity(len);
    for _ in 0..len {
        units.push(reader.read_u16::<LittleEndian>()?);
    }
    let _null = reader.read_u16::<LittleEndian>()?;
    String::from_utf16(&units).context("Invalid UTF-16 in StringPool")
}
