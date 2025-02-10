use anyhow::Result;
use log::info;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write, SeekFrom},
    path::Path,
    collections::HashMap,
};

use memmap2::MmapMut;

const BLOCK_SIZE: usize = 4;

#[derive(Debug)]
struct Block {
    offset: u64,
    size: usize,
    weak_checksum: u32,
    strong_checksum: [u8; 32],
}

pub struct Syncer {
    source: String,
    destination: String,
}

impl Syncer {
    pub fn new(source: String, destination: String) -> Self {
        Self {
            source,
            destination,
        }
    }

    pub fn sync(&self) -> Result<()> {
        info!("Syncing...");

        // check if source is a file or a directory
        let src_path = Path::new(&self.source);
        let dst_path = Path::new(&self.destination);
        if src_path.is_file() {
            self.sync_file(src_path, dst_path)?;
        } else {
            todo!()
        }

        info!("Sync completed");
        Ok(())
    }

    fn sync_file(&self, src_path: &Path, dst_path: &Path) -> Result<()> {
        info!("Syncing file: {:?} -> {:?}", src_path, dst_path);

        if !dst_path.exists() {
            info!("Destination doesn't exist, performing full copy");
            return self.copy_file(src_path, dst_path);
        }

        // 1. Build checksum blocks for the destination file.
        let dst_blocks = self.calculate_checksums(dst_path)?;
        let mut weak_lookup: HashMap<u32, Vec<&Block>> = HashMap::new();
        let mut strong_lookup: HashMap<[u8; 32], &Block> = HashMap::new();
        for block in &dst_blocks {
            weak_lookup.entry(block.weak_checksum).or_default().push(block);
            strong_lookup.insert(block.strong_checksum, block);
        }

        // 2. Open the source file and prepare a temporary file.
        let mut src_file = File::open(src_path)?;
        let src_size = src_file.metadata()?.len();
        let temp_path = dst_path.with_extension("tmp");
        let temp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_path)?;

        // Set the temporary file size to match the source file.
        // This is needed to safely map the entire file into memory.
        temp_file.set_len(src_size)?;

        // Create a mutable memory map for the temporary file.
        // This allows us to write data directly into memory, reducing explicit write calls.
        let mut mmap = unsafe { MmapMut::map_mut(&temp_file)? };

        // If the source is smaller than one block, do a full copy.
        if src_size < BLOCK_SIZE as u64 {
            return self.copy_file(src_path, dst_path);
        }

        // 3. Read the initial window (of BLOCK_SIZE bytes) from the source file.
        let mut window = vec![0; BLOCK_SIZE];
        src_file.read_exact(&mut window)?;
        let mut weak = self.calculate_weak_checksum(&window);
        let mut offset: u64 = 0;
        let mut last_match: u64 = 0;

        // 4. Sliding window: scan the source file.
        while offset + BLOCK_SIZE as u64 <= src_size {
            if let Some(candidates) = weak_lookup.get(&weak) {
                let strong = self.calculate_strong_checksum(&window);
                if let Some(&block) = candidates.iter().find(|b| b.strong_checksum == strong) {
                    // A matching block has been found.
                    // First, copy any unmatched data from last_match to current offset.
                    if offset > last_match {
                        src_file.seek(SeekFrom::Start(last_match))?;
                        let mut unmatched = vec![0; (offset - last_match) as usize];
                        src_file.read_exact(&mut unmatched)?;
                        // Write unmatched data directly into the mmap slice.
                        mmap[last_match as usize..offset as usize].copy_from_slice(&unmatched);
                    }
                    // Then, copy the matching block from the destination file.
                    {
                        let mut dst_file = File::open(dst_path)?;
                        dst_file.seek(SeekFrom::Start(block.offset))?;
                        let mut block_data = vec![0; block.size];
                        dst_file.read_exact(&mut block_data)?;
                        // Copy the existing block data into the mmap at the proper offset.
                        // In this sliding window loop, the block size is assumed to be BLOCK_SIZE.
                        mmap[offset as usize..(offset + BLOCK_SIZE as u64) as usize]
                            .copy_from_slice(&block_data);
                    }
                    offset += BLOCK_SIZE as u64;
                    last_match = offset;
                    // Read the next window from the source file, if available.
                    if offset + BLOCK_SIZE as u64 <= src_size {
                        src_file.seek(SeekFrom::Start(offset))?;
                        src_file.read_exact(&mut window)?;
                        weak = self.calculate_weak_checksum(&window);
                    } else {
                        break;
                    }
                    continue;
                }
            }
            // No match: slide the window one byte.
            offset += 1;
            if offset + BLOCK_SIZE as u64 <= src_size {
                let old_byte = window[0];
                window.copy_within(1.., 0);
                src_file.seek(SeekFrom::Start(offset + BLOCK_SIZE as u64 - 1))?;
                src_file.read_exact(&mut window[BLOCK_SIZE - 1..BLOCK_SIZE])?;
                weak = self.update_weak_checksum(old_byte, window[BLOCK_SIZE - 1], weak, BLOCK_SIZE);
            } else {
                break;
            }
        }
        // 5. Write any remaining data from the source file into the mmap.
        if last_match < src_size {
            src_file.seek(SeekFrom::Start(last_match))?;
            let mut remainder = Vec::new();
            src_file.read_to_end(&mut remainder)?;
            mmap[last_match as usize..].copy_from_slice(&remainder);
        }
        // 6. Flush the mmap to ensure all data is written to disk,
        // and then replace the destination file with the temporary file.
        mmap.flush()?;
        fs::rename(temp_path, dst_path)?;
        Ok(())
    }

    fn calculate_checksums(&self, path: &Path) -> Result<Vec<Block>> {
        let mut file = File::open(path)?;
        let file_size = file.metadata()?.len();
        let mut blocks = Vec::new();
        let mut offset: u64 = 0;
        let mut buffer = vec![0; BLOCK_SIZE];
        while offset < file_size {
            let read_size = if offset + BLOCK_SIZE as u64 > file_size {
                (file_size - offset) as usize
            } else {
                BLOCK_SIZE
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

    fn copy_file(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::copy(src, dst)?;
        Ok(())
    }

    fn calculate_weak_checksum(&self, data: &[u8]) -> u32 {
        let mut a: u32 = 0;
        let mut b: u32 = 0;
        
        for &byte in data {
            a = a.wrapping_add(byte as u32);
            b = b.wrapping_add(a);
        }
        (a & 0xffff) | ((b & 0xffff) << 16)
    }

    fn calculate_strong_checksum(&self, data: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    fn update_weak_checksum(&self, old_byte: u8, new_byte: u8, old_sum: u32, len: usize) -> u32 {
        let a_old = old_sum & 0xffff;
        let b_old = (old_sum >> 16) & 0xffff;
        let a_new = a_old.wrapping_sub(old_byte as u32).wrapping_add(new_byte as u32);
        let b_new = b_old.wrapping_sub((len as u32).wrapping_mul(old_byte as u32)).wrapping_add(a_new);
        (a_new & 0xffff) | ((b_new & 0xffff) << 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_files(name: &str, src_content: &[u8], dst_content: &[u8]) -> (String, String) {
        let src_path = format!("test_src_{}", name);
        let dst_path = format!("test_dst_{}", name);
        
        let mut src_file = File::create(&src_path).unwrap();
        src_file.write_all(src_content).unwrap();
        src_file.flush().unwrap();
        drop(src_file);

        let mut dst_file = File::create(&dst_path).unwrap();
        dst_file.write_all(dst_content).unwrap();
        dst_file.flush().unwrap();
        drop(dst_file);

        (src_path, dst_path)
    }

    fn cleanup_test_files(src_path: &str, dst_path: &str) {
        let _ = fs::remove_file(src_path);
        let _ = fs::remove_file(dst_path);
    }

    fn verify_content(path: &str, expected: &[u8]) {
        let mut file = File::open(path).unwrap();
        let mut buffer = vec![0; expected.len()];
        file.read_exact(&mut buffer).unwrap();
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_basic_sync() {
        let (src, dst) = setup_test_files("basic", b"0123456789", b"012345a789");
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, b"0123456789");
        cleanup_test_files(&src, &dst);
    }

    #[test]
    fn test_empty_destination() {
        let (src, dst) = setup_test_files("empty", b"0123456789", b"");
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, b"0123456789");
        cleanup_test_files(&src, &dst);
    }

    #[test]
    fn test_identical_files() {
        let (src, dst) = setup_test_files("identical", b"0123456789", b"0123456789");
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, b"0123456789");
        cleanup_test_files(&src, &dst);
    }

    #[test]
    fn test_multiple_changes() {
        let (src, dst) = setup_test_files("multiple_changes", b"0123456789", b"01a34b6c89");
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, b"0123456789");
        cleanup_test_files(&src, &dst);
    }

    #[test]
    fn test_longer_files() {
        let src_content = "The quick brown fox jumps over the lazy dog".as_bytes();
        let dst_content = "The quick brown cat jumps over the lazy dog".as_bytes();
        let (src, dst) = setup_test_files("longer_files", src_content, dst_content);
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, src_content);
        cleanup_test_files(&src, &dst);
    }

    #[test]
    fn test_different_sizes() {
        let (src, dst) = setup_test_files("different_sizes", b"0123456789", b"01234");
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, b"0123456789");
        cleanup_test_files(&src, &dst);
    }

    #[test]
    fn test_binary_data() {
        let src_content = &[0, 1, 2, 3, 255, 254, 253, 252];
        let dst_content = &[0, 1, 2, 3, 0, 254, 253, 252];
        let (src, dst) = setup_test_files("binary_data", src_content, dst_content);
        let syncer = Syncer::new(src.clone(), dst.clone());
        syncer.sync().unwrap();
        verify_content(&dst, src_content);
        cleanup_test_files(&src, &dst);
    }
}
