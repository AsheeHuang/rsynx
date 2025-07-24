use crate::sync::{Block, Syncer, TransferResult};
use anyhow::Context;
use anyhow::Result;
use filetime::{FileTime, set_file_times};
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use memmap2::MmapMut;
use std::cmp::min;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// LocalSyncer implements local file/directory synchronization using shared Syncer functionality.
pub struct LocalSyncer {
    syncer: Syncer,
    source: String,
    destination: String,
}

impl LocalSyncer {
    pub fn new(source: String, destination: String) -> Self {
        Self {
            syncer: Syncer::new(),
            source,
            destination,
        }
    }

    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.syncer.block_size = block_size;
        self
    }

    pub fn with_preserve_metadata(mut self, preserve: bool) -> Self {
        self.syncer.preserve_metadata = preserve;
        self
    }

    pub fn with_delete_extraneous(mut self, delete: bool) -> Self {
        self.syncer.delete_extraneous = delete;
        self
    }

    pub fn sync(&self) -> Result<TransferResult> {
        info!("Local syncing...");
        let src_path = Path::new(&self.source);
        let dst_path = Path::new(&self.destination);
        let result = if src_path.is_file() {
            self.sync_file(src_path, dst_path)?
        } else if src_path.is_dir() {
            self.sync_dir(src_path, dst_path)?
        } else {
            return Err(anyhow::anyhow!("Unsupported source type"));
        };
        info!("Local sync completed");
        Ok(result)
    }

    fn sync_file(&self, src_path: &Path, dst_path: &Path) -> Result<TransferResult> {
        info!("Syncing file: {:?} -> {:?}", src_path, dst_path);

        if !dst_path.exists() {
            info!("Destination doesn't exist, performing full copy");
            return self.syncer.copy_file(src_path, dst_path);
        }

        let dst_blocks = self.syncer.calculate_checksums(dst_path)?;
        let mut weak_lookup: HashMap<u32, Vec<&Block>> = HashMap::new();
        for block in &dst_blocks {
            weak_lookup
                .entry(block.weak_checksum)
                .or_default()
                .push(block);
        }

        let mut src_file = File::open(src_path)?;
        let src_size = src_file.metadata()?.len();

        if src_size < self.syncer.block_size as u64 {
            return self.syncer.copy_file(src_path, dst_path);
        }

        // Create progress bar
        let pb = ProgressBar::new(src_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .expect("Failed to set progress bar template")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Syncing {}", src_path.display()));

        let temp_path = dst_path.with_extension("tmp");
        let temp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;
        temp_file.set_len(src_size)?;

        let mut mmap = unsafe { MmapMut::map_mut(&temp_file)? };
        let mut window = vec![0; min(self.syncer.block_size, src_size as usize)];
        src_file.read_exact(&mut window)?;
        let mut weak = self.syncer.calculate_weak_checksum(&window);
        let mut offset: u64 = 0;
        let mut last_match: u64 = 0;
        let mut reused_bytes = 0usize;

        while offset + self.syncer.block_size as u64 <= src_size {
            if let Some(candidates) = weak_lookup.get(&weak) {
                let strong = self.syncer.calculate_strong_checksum(&window);
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
                        mmap[offset as usize..(offset + self.syncer.block_size as u64) as usize]
                            .copy_from_slice(&block_data);
                    }
                    reused_bytes += block.size;
                    offset += self.syncer.block_size as u64;
                    last_match = offset;
                    pb.set_position(offset);
                    if offset + self.syncer.block_size as u64 <= src_size {
                        src_file.seek(SeekFrom::Start(offset))?;
                        src_file.read_exact(&mut window)?;
                        weak = self.syncer.calculate_weak_checksum(&window);
                    } else {
                        break;
                    }
                    continue;
                }
            }
            offset += 1;
            pb.set_position(offset);
            if offset + self.syncer.block_size as u64 <= src_size {
                let old_byte = window[0];
                window.copy_within(1.., 0);
                src_file.seek(SeekFrom::Start(offset + self.syncer.block_size as u64 - 1))?;
                src_file
                    .read_exact(&mut window[self.syncer.block_size - 1..self.syncer.block_size])?;
                weak = self.syncer.update_weak_checksum(
                    old_byte,
                    window[self.syncer.block_size - 1],
                    weak,
                    self.syncer.block_size,
                );
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

        if self.syncer.preserve_metadata {
            let src_meta = fs::metadata(src_path)?;
            fs::set_permissions(&temp_path, src_meta.permissions()).with_context(|| {
                format!(
                    "Failed to set permissions for temporary file: {:?}",
                    temp_path
                )
            })?;
            let atime = FileTime::from_last_access_time(&src_meta);
            let mtime = FileTime::from_last_modification_time(&src_meta);
            set_file_times(&temp_path, atime, mtime).with_context(|| {
                format!(
                    "Failed to set file times for temporary file: {:?}",
                    temp_path
                )
            })?;
        }

        fs::rename(temp_path.clone(), dst_path)?;

        // Complete progress bar
        pb.finish_with_message(format!(
            "Synced {} ({} bytes)",
            src_path.display(),
            src_size
        ));

        let total_bytes = src_size as usize;
        let new_bytes = total_bytes.saturating_sub(reused_bytes);
        Ok(TransferResult {
            new_bytes,
            reused_bytes,
        })
    }

    fn sync_dir(&self, src_dir: &Path, dst_dir: &Path) -> Result<TransferResult> {
        info!("Syncing directory: {:?} -> {:?}", src_dir, dst_dir);
        if !dst_dir.exists() {
            fs::create_dir_all(dst_dir)?;
        }
        let mut src_names = HashSet::new();
        let mut total_reused_bytes = 0usize;
        let mut total_bytes = 0usize;

        for entry in fs::read_dir(src_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            src_names.insert(file_name.clone());
            let path = entry.path();
            let dest_path = dst_dir.join(&file_name);

            let entry_size = if path.is_file() {
                let res = self.sync_file(&path, &dest_path)?;
                total_reused_bytes += res.reused_bytes;
                fs::metadata(&path)?.len() as usize
            } else if path.is_dir() {
                let res = self.sync_dir(&path, &dest_path)?;
                total_reused_bytes += res.reused_bytes;
                res.new_bytes + res.reused_bytes
            } else {
                info!("Skipping unsupported file type: {:?}", path);
                0
            };

            total_bytes += entry_size;
        }
        if self.syncer.delete_extraneous {
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
        let new_bytes = total_bytes.saturating_sub(total_reused_bytes);
        Ok(TransferResult {
            new_bytes,
            reused_bytes: total_reused_bytes,
        })
    }
}
