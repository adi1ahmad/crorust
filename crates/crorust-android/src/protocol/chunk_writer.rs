use anyhow::{Context, Result};
use std::io::{Seek, SeekFrom, Write};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u16)]
pub enum AndroidChunkTag {
    Null = 0x0000,
    StringPool = 0x0001,
    ResourceTable = 0x0002,
    XmlDocument = 0x0003,
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

pub struct StreamingChunkWriter<W: Write + Seek> {
    sink: W,
    frame_offsets: Vec<u64>,
}

impl<W: Write + Seek> StreamingChunkWriter<W> {
    pub fn new(sink: W) -> Self {
        Self {
            sink,
            frame_offsets: Vec::with_capacity(16),
        }
    }

    pub fn begin_chunk(&mut self, tag: AndroidChunkTag, header_size: u16) -> Result<()> {
        let pos = self.sink.stream_position()?;
        self.sink.write_all(&(tag as u16).to_le_bytes())?;
        self.sink.write_all(&header_size.to_le_bytes())?;
        self.sink.write_all(&0u32.to_le_bytes())?;
        self.frame_offsets.push(pos);
        Ok(())
    }

    pub fn end_chunk(&mut self) -> Result<u32> {
        let start_pos = self
            .frame_offsets
            .pop()
            .context("Frame underflow in StreamingChunkWriter")?;
        let current_pos = self.sink.stream_position()?;
        let unpadded_size = (current_pos - start_pos) as u32;

        let padding = (4 - (unpadded_size % 4)) % 4;
        if padding > 0 {
            self.sink.write_all(&[0u8; 4][..padding as usize])?;
        }

        let final_pos = self.sink.stream_position()?;
        let total_size = (final_pos - start_pos) as u32;

        self.sink.seek(SeekFrom::Start(start_pos + 4))?;
        self.sink.write_all(&total_size.to_le_bytes())?;
        self.sink.seek(SeekFrom::Start(final_pos))?;

        Ok(total_size)
    }

    pub fn stream_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.sink.write_all(data)?;
        Ok(())
    }

    pub fn finalize(self) -> W {
        self.sink
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_streaming_chunk_writer_patches_frame_sizes() -> Result<()> {
        let mut writer = StreamingChunkWriter::new(Cursor::new(Vec::new()));
        writer.begin_chunk(AndroidChunkTag::XmlDocument, 8)?;
        writer.stream_bytes(b"HELLO_STREAM")?;
        let total_len = writer.end_chunk()?;

        let buf = writer.finalize().into_inner();
        assert_eq!(total_len as usize, buf.len());
        assert_eq!(buf.len() % 4, 0, "Chunk must be 4-byte word aligned");

        let chunk_tag = u16::from_le_bytes([buf[0], buf[1]]);
        let header_len = u16::from_le_bytes([buf[2], buf[3]]);
        let recorded_total = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);

        assert_eq!(chunk_tag, AndroidChunkTag::XmlDocument as u16);
        assert_eq!(header_len, 8);
        assert_eq!(recorded_total, total_len);
        Ok(())
    }
}
