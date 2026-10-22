use anyhow::Result;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

pub const CHUNK_SIZE_1MB: usize = 1_048_576;
pub const CHUNK_DIGEST_PREFIX: u8 = 0xa5;
pub const MERKLE_ROOT_PREFIX: u8 = 0x5a;

pub struct RayonApkSigner;

impl RayonApkSigner {
    pub fn compute_merkle_digest_parallel(file: &mut File, cd_offset: u64) -> Result<[u8; 32]> {
        file.seek(SeekFrom::Start(0))?;
        let total_bytes = cd_offset as usize;
        let chunk_count = total_bytes.div_ceil(CHUNK_SIZE_1MB);

        let mut chunk_offsets: Vec<(u64, usize)> = Vec::with_capacity(chunk_count);
        for i in 0..chunk_count {
            let offset = (i * CHUNK_SIZE_1MB) as u64;
            let len = (total_bytes - i * CHUNK_SIZE_1MB).min(CHUNK_SIZE_1MB);
            chunk_offsets.push((offset, len));
        }

        let mut chunks_data = Vec::with_capacity(chunk_count);
        for (offset, len) in chunk_offsets {
            file.seek(SeekFrom::Start(offset))?;
            let mut buf = vec![0u8; len];
            file.read_exact(&mut buf)?;
            chunks_data.push(buf);
        }

        let chunk_hashes: Vec<[u8; 32]> = chunks_data
            .par_iter()
            .map(|chunk_data| {
                let mut hasher = Sha256::new();
                hasher.update([CHUNK_DIGEST_PREFIX]);
                hasher.update((chunk_data.len() as u32).to_le_bytes());
                hasher.update(chunk_data);
                let mut out = [0u8; 32];
                out.copy_from_slice(&hasher.finalize());
                out
            })
            .collect();

        let mut top_hasher = Sha256::new();
        top_hasher.update([MERKLE_ROOT_PREFIX]);
        top_hasher.update((chunk_hashes.len() as u32).to_le_bytes());
        for h in &chunk_hashes {
            top_hasher.update(h);
        }

        let mut root = [0u8; 32];
        root.copy_from_slice(&top_hasher.finalize());
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_rayon_merkle_digest_parallel() -> Result<()> {
        let mut temp = NamedTempFile::new()?;
        let data = vec![0x42u8; 2 * 1024 * 1024 + 512];
        temp.write_all(&data)?;

        let mut file = File::open(temp.path())?;
        let root = RayonApkSigner::compute_merkle_digest_parallel(&mut file, data.len() as u64)?;
        assert_ne!(root, [0u8; 32], "Root digest must not be zero");
        Ok(())
    }
}
