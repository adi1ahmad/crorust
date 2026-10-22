use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

pub struct CentralDirectoryLocator {
    pub cde_start: u64,
    pub cd_start: u64,
}

impl CentralDirectoryLocator {
    pub fn new<R: Read + Seek>(r: &mut R) -> Result<Self> {
        let cde_start = locate_central_directory_anchor(r)?;
        r.seek(SeekFrom::Start(cde_start + 16))?;
        let cd_start = r.read_u32::<LittleEndian>()? as u64;
        Ok(Self {
            cde_start,
            cd_start,
        })
    }
}

pub fn locate_central_directory_anchor<R: Read + Seek>(reader: &mut R) -> Result<u64> {
    const CENTRAL_DIRECTORY_END_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    const HEADER_SIZE: usize = 22;

    let file_length = reader.seek(SeekFrom::End(0))?;
    anyhow::ensure!(
        file_length >= HEADER_SIZE as u64,
        "Invalid zip file: length ({} bytes) is smaller than minimum EOCD header ({} bytes)",
        file_length,
        HEADER_SIZE
    );

    let max_search_window = (HEADER_SIZE as u64) + (u16::MAX as u64);
    let scan_len = std::cmp::min(file_length, max_search_window) as usize;
    let buf_start_pos = file_length - (scan_len as u64);

    reader.seek(SeekFrom::Start(buf_start_pos))?;
    let mut buffer = vec![0u8; scan_len];
    reader.read_exact(&mut buffer)?;

    let max_search_offset = scan_len - HEADER_SIZE;
    let mut candidate_offset = None;

    for offset in (0..=max_search_offset).rev() {
        if buffer[offset..offset + 4] == CENTRAL_DIRECTORY_END_SIGNATURE {
            let comment_len =
                u16::from_le_bytes([buffer[offset + 20], buffer[offset + 21]]) as usize;

            if offset + HEADER_SIZE + comment_len == buffer.len() {
                let eocd_pos = buf_start_pos + offset as u64;
                reader.seek(SeekFrom::Start(eocd_pos))?;
                return Ok(eocd_pos);
            }

            if candidate_offset.is_none() {
                candidate_offset = Some(offset);
            }
        }
    }

    if let Some(offset) = candidate_offset {
        let eocd_pos = buf_start_pos + offset as u64;
        reader.seek(SeekFrom::Start(eocd_pos))?;
        return Ok(eocd_pos);
    }

    anyhow::bail!("Could not find central directory end signature (0x06054b50) in ZIP archive");
}
