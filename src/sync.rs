use anyhow::Context;
use anyhow::Result;
use filetime::{FileTime, set_file_times};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

#[derive(Debug)]
pub struct Block {
    pub offset: u64,
    pub size: usize,
    pub weak_checksum: u32,
    pub strong_checksum: [u8; 32],
}

/// Result returned by the sync process, measured in bytes.
pub struct TransferResult {
    pub new_bytes: usize,
    pub reused_bytes: usize,
}

/// Common functionality including checksum calculation, file copying, and metadata preservation.
pub struct Syncer {
    pub block_size: usize,
    pub preserve_metadata: bool,
    pub delete_extraneous: bool,
    pub compress: bool,
}

impl Default for Syncer {
    fn default() -> Self {
        Self::new()
    }
}

impl Syncer {
    pub fn new() -> Self {
        Self {
            block_size: 1024,
            preserve_metadata: false,
            delete_extraneous: false,
            compress: false,
        }
    }

    pub fn calculate_weak_checksum(&self, data: &[u8]) -> u32 {
        let mut a: u32 = 0;
        let mut b: u32 = 0;

        for &byte in data {
            a = a.wrapping_add(byte as u32);
            b = b.wrapping_add(a);
        }
        (a & 0xffff) | ((b & 0xffff) << 16)
    }

    pub fn calculate_strong_checksum(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    pub fn update_weak_checksum(
        &self,
        old_byte: u8,
        new_byte: u8,
        old_sum: u32,
        len: usize,
    ) -> u32 {
        let a_old = old_sum & 0xffff;
        let b_old = (old_sum >> 16) & 0xffff;
        let a_new = a_old
            .wrapping_sub(old_byte as u32)
            .wrapping_add(new_byte as u32);
        let b_new = b_old
            .wrapping_sub((len as u32).wrapping_mul(old_byte as u32))
            .wrapping_add(a_new);
        (a_new & 0xffff) | ((b_new & 0xffff) << 16)
    }

    pub fn calculate_checksums(&self, path: &Path) -> Result<Vec<Block>> {
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let mut blocks = Vec::new();
        let mut offset: u64 = 0;
        let mut buffer = vec![0; self.block_size];
        while offset < file_size {
            let read_size = if offset + self.block_size as u64 > file_size {
                (file_size - offset) as usize
            } else {
                self.block_size
            };
            buffer.resize(read_size, 0);
            file.read_exact(&mut buffer)?;
            let weak = self.calculate_weak_checksum(&buffer);
            let strong = self.calculate_strong_checksum(&buffer);
            blocks.push(Block {
                offset,
                size: read_size,
                weak_checksum: weak,
                strong_checksum: strong,
            });
            offset += read_size as u64;
        }
        Ok(blocks)
    }

    pub fn copy_file(&self, src: &Path, dst: &Path) -> Result<TransferResult> {
        fs::copy(src, dst)
            .with_context(|| format!("Failed to copy file from {:?} to {:?}", src, dst))?;

        if self.preserve_metadata {
            let src_meta = fs::metadata(src)
                .with_context(|| format!("Failed to get metadata for source file: {:?}", src))?;
            fs::set_permissions(dst, src_meta.permissions()).with_context(|| {
                format!("Failed to set permissions for destination file: {:?}", dst)
            })?;
            let atime = FileTime::from_last_access_time(&src_meta);
            let mtime = FileTime::from_last_modification_time(&src_meta);
            set_file_times(dst, atime, mtime).with_context(|| {
                format!("Failed to set file times for destination file: {:?}", dst)
            })?;
        }
        let src_size = fs::metadata(src)
            .with_context(|| format!("Failed to get metadata for source file: {:?}", src))?
            .len() as usize;
        Ok(TransferResult {
            new_bytes: src_size,
            reused_bytes: 0,
        })
    }

    /// Compress data using gzip compression
    pub fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        if !self.compress {
            return Ok(data.to_vec());
        }

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        encoder.finish().map_err(Into::into)
    }

    /// Decompress data that was compressed with gzip
    pub fn decompress_data(&self, compressed_data: &[u8]) -> Result<Vec<u8>> {
        if !self.compress {
            return Ok(compressed_data.to_vec());
        }

        let mut decoder = GzDecoder::new(compressed_data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }
}
