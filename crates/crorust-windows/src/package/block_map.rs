use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const BLOCK_SIZE: usize = 65536;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MsixBlockMapDescriptor {
    #[serde(rename = "@HashMethod")]
    pub hash_method: String,
    #[serde(rename = "File", default)]
    pub files: Vec<BlockMapFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockMapFile {
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Size")]
    pub size: u64,
    #[serde(rename = "Block", default)]
    pub blocks: Vec<BlockRecord>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockRecord {
    #[serde(rename = "@Hash")]
    pub hash: String,
    #[serde(rename = "@Size", skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct MsixBlockMapBuilder {
    files: Vec<BlockMapFile>,
}

impl MsixBlockMapBuilder {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn add_file(&mut self, dest: &Path, source: &Path) -> Result<()> {
        let f = File::open(source)
            .with_context(|| format!("Failed to read file for block map: {}", source.display()))?;
        let size = f.metadata()?.len();
        let name = dest.to_string_lossy().replace('/', "\\");
        let blocks = hash_file_blocks_streaming(f)?;
        self.files.push(BlockMapFile { name, size, blocks });
        Ok(())
    }

    pub fn add_data(&mut self, dest: &Path, data: &[u8]) {
        let name = dest.to_string_lossy().replace('/', "\\");
        let size = data.len() as u64;
        let blocks = hash_payload_blocks_parallel(data);
        self.files.push(BlockMapFile { name, size, blocks });
    }

    pub fn finish(self) -> MsixBlockMapDescriptor {
        MsixBlockMapDescriptor {
            hash_method: "http://www.w3.org/2001/04/xmlenc#sha256".to_string(),
            files: self.files,
        }
    }
}

pub fn hash_payload_blocks_parallel(data: &[u8]) -> Vec<BlockRecord> {
    let chunks: Vec<&[u8]> = data.chunks(BLOCK_SIZE).collect();
    chunks
        .par_iter()
        .map(|chunk| {
            let digest = Sha256::digest(chunk);
            let hash_base64 = base64::encode(digest);
            BlockRecord {
                hash: hash_base64,
                size: Some(chunk.len() as u64),
            }
        })
        .collect()
}

const STREAM_BATCH_BLOCKS: usize = 16; // 1 MB batch window (16 * 64 KB)

pub fn hash_file_blocks_streaming<R: Read>(mut reader: R) -> Result<Vec<BlockRecord>> {
    let batch_capacity = STREAM_BATCH_BLOCKS * BLOCK_SIZE;
    let mut batch_buf = vec![0u8; batch_capacity];
    let mut blocks = Vec::new();

    loop {
        let mut batch_len = 0;
        while batch_len < batch_capacity {
            let n = reader.read(&mut batch_buf[batch_len..])?;
            if n == 0 {
                break;
            }
            batch_len += n;
        }

        if batch_len == 0 {
            break;
        }

        let chunk_slices: Vec<&[u8]> = batch_buf[..batch_len].chunks(BLOCK_SIZE).collect();
        let batch_records: Vec<BlockRecord> = chunk_slices
            .par_iter()
            .map(|chunk| {
                let digest = Sha256::digest(chunk);
                let hash_base64 = base64::encode(digest);
                BlockRecord {
                    hash: hash_base64,
                    size: Some(chunk.len() as u64),
                }
            })
            .collect();

        blocks.extend(batch_records);

        if batch_len < batch_capacity {
            break;
        }
    }

    Ok(blocks)
}
