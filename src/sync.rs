use anyhow::Result;
use log::info;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::Path,
    collections::HashMap,
};
use filetime::{FileTime, set_file_times};

use memmap2::MmapMut;
#[derive(Debug)]

struct Block {
    offset: u64,
    size: usize,
    weak_checksum: u32,
    strong_checksum: [u8; 32],
}

/// Result returned by the sync process, measured in bytes.
pub struct TransferResult {
    pub new_bytes: usize,
    pub reused_bytes: usize,
}

pub struct Syncer {
    source: String,
    destination: String,
    block_size: usize,
    preserve_metadata: bool,
    delete_extraneous: bool,
}

impl Syncer {
    pub fn new(source: String, destination: String) -> Self {
        Self {
            source,
            destination,
            block_size: 1024,
            preserve_metadata: false,
            delete_extraneous: false,
        }
    }

    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size;
        self
    }

    pub fn with_preserve_metadata(mut self, preserve: bool) -> Self {
        self.preserve_metadata = preserve;
        self
    }

    pub fn with_delete_extraneous(mut self, delete: bool) -> Self {
        self.delete_extraneous = delete;
        self
    }

    pub fn sync(&self) -> Result<TransferResult> {
        info!("Syncing...");
        let src_path = Path::new(&self.source);
        let dst_path = Path::new(&self.destination);
        let result = if src_path.is_file() {
            self.sync_file(src_path, dst_path)?
        } else if src_path.is_dir() {
            self.sync_dir(src_path, dst_path)?
        } else {
            return Err(anyhow::anyhow!("Unsupported source type"));
        };
        info!("Sync completed");
        Ok(result)
    }

    fn sync_file(&self, src_path: &Path, dst_path: &Path) -> Result<TransferResult> {
        info!("Syncing file: {:?} -> {:?}", src_path, dst_path);

        if !dst_path.exists() {
            info!("Destination doesn't exist, performing full copy");
            return self.copy_file(src_path, dst_path);
        }

        let dst_blocks = self.calculate_checksums(dst_path)?;
        let mut weak_lookup: HashMap<u32, Vec<&Block>> = HashMap::new();
        for block in &dst_blocks {
            weak_lookup.entry(block.weak_checksum).or_default().push(block);
        }

        let mut src_file = File::open(src_path)?;
        let src_size = src_file.metadata()?.len();
        let temp_path = dst_path.with_extension("tmp");
        let temp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_path)?;

        temp_file.set_len(src_size)?;

        let mut mmap = unsafe { MmapMut::map_mut(&temp_file)? };

        if src_size < self.block_size as u64 {
            return self.copy_file(src_path, dst_path);
        }

        let mut window = vec![0; self.block_size];
        src_file.read_exact(&mut window)?;
        let mut weak = self.calculate_weak_checksum(&window);
        let mut offset: u64 = 0;
        let mut last_match: u64 = 0;
        let mut reused_bytes = 0usize;

        while offset + self.block_size as u64 <= src_size {
            if let Some(candidates) = weak_lookup.get(&weak) {
                let strong = self.calculate_strong_checksum(&window);
                if let Some(&block) = candidates.iter().find(|b| b.strong_checksum == strong) {
                    if offset > last_match {
                        src_file.seek(SeekFrom::Start(last_match))?;
                        let mut unmatched = vec![0; (offset - last_match) as usize];
                        src_file.read_exact(&mut unmatched)?;
                        mmap[last_match as usize..offset as usize].copy_from_slice(&unmatched);
                    }
                    {
                        let mut dst_file = File::open(dst_path)?;
                        dst_file.seek(SeekFrom::Start(block.offset))?;
                        let mut block_data = vec![0; block.size];
                        dst_file.read_exact(&mut block_data)?;
                        mmap[offset as usize..(offset + self.block_size as u64) as usize]
                            .copy_from_slice(&block_data);
                    }
                    reused_bytes += block.size;
                    offset += self.block_size as u64;
                    last_match = offset;
                    if offset + self.block_size as u64 <= src_size {
                        src_file.seek(SeekFrom::Start(offset))?;
                        src_file.read_exact(&mut window)?;
                        weak = self.calculate_weak_checksum(&window);
                    } else {
                        break;
                    }
                    continue;
                }
            }
            offset += 1;
            if offset + self.block_size as u64 <= src_size {
                let old_byte = window[0];
                window.copy_within(1.., 0);
                src_file.seek(SeekFrom::Start(offset + self.block_size as u64 - 1))?;
                src_file.read_exact(&mut window[self.block_size - 1..self.block_size])?;
                weak = self.update_weak_checksum(old_byte, window[self.block_size - 1], weak, self.block_size);
            } else {
                break;
            }
        }
        if last_match < src_size {
            src_file.seek(SeekFrom::Start(last_match))?;
            let mut remainder = Vec::new();
            src_file.read_to_end(&mut remainder)?;
            mmap[last_match as usize..].copy_from_slice(&remainder);
        }
        mmap.flush()?;
        fs::rename(temp_path, dst_path)?;
        if self.preserve_metadata {
            let src_meta = fs::metadata(src_path)?;
            fs::set_permissions(dst_path, src_meta.permissions())?;
            let atime = FileTime::from_last_access_time(&src_meta);
            let mtime = FileTime::from_last_modification_time(&src_meta);
            set_file_times(dst_path, atime, mtime)?;
        }
        let total_bytes = src_size as usize;
        let new_bytes = total_bytes.saturating_sub(reused_bytes);
        Ok(TransferResult { new_bytes, reused_bytes })
    }

    /// Recursively syncs a directory from `src_dir` to `dst_dir`.
    pub fn sync_dir(&self, src_dir: &Path, dst_dir: &Path) -> Result<TransferResult> {
        info!("Syncing directory: {:?} -> {:?}", src_dir, dst_dir);
        if !dst_dir.exists() {
            fs::create_dir_all(dst_dir)?;
        }
        use std::collections::HashSet;
        let mut src_names = HashSet::new();
        let mut total_reused_bytes = 0usize;
        for entry in fs::read_dir(src_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            src_names.insert(file_name.clone());
            let path = entry.path();
            let dest_path = dst_dir.join(&file_name);
            if path.is_file() {
                let res = self.sync_file(&path, &dest_path)?;
                total_reused_bytes += res.reused_bytes;
            } else if path.is_dir() {
                let res = self.sync_dir(&path, &dest_path)?;
                total_reused_bytes += res.reused_bytes;
            } else {
                info!("Skipping unsupported file type: {:?}", path);
            }
        }
        if self.delete_extraneous {
            for entry in fs::read_dir(dst_dir)? {
                let entry = entry?;
                if !src_names.contains(&entry.file_name()) {
                    let extra_path = entry.path();
                    if extra_path.is_file() {
                        fs::remove_file(&extra_path)?;
                    } else if extra_path.is_dir() {
                        fs::remove_dir_all(&extra_path)?;
                    }
                }
            }
        }
        let total_bytes = fs::metadata(src_dir)?.len() as usize;
        let new_bytes = total_bytes.saturating_sub(total_reused_bytes);
        Ok(TransferResult { new_bytes, reused_bytes: total_reused_bytes })
    }

    fn calculate_checksums(&self, path: &Path) -> Result<Vec<Block>> {
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

    fn copy_file(&self, src: &Path, dst: &Path) -> Result<TransferResult> {
        fs::copy(src, dst)?;
        if self.preserve_metadata {
            let src_meta = fs::metadata(src)?;
            fs::set_permissions(dst, src_meta.permissions())?;
            let atime = FileTime::from_last_access_time(&src_meta);
            let mtime = FileTime::from_last_modification_time(&src_meta);
            set_file_times(dst, atime, mtime)?;
        }
        let src_size = fs::metadata(src)?.len() as usize;
        Ok(TransferResult { new_bytes: src_size, reused_bytes: 0 })
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
