use crate::sync::{Syncer, TransferResult};
use anyhow::{Context, Result};
use log::info;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::{TcpListener, TcpStream},
    path::Path,
};
use std::collections::HashMap;
use hex;

/// NetworkSyncer implements network synchronization using rsync algorithm, currently only supports file synchronization.
pub struct NetworkSyncer {
    pub syncer: Syncer,
    pub remote_address: String,
    pub remote_port: u16,
    pub source: String,
    pub destination: String,
    pub block_size: usize,
}

impl NetworkSyncer {
    pub fn new(remote_address: String, remote_port: u16, source: String, destination: String) -> Self {
        Self {
            syncer: Syncer::new(),
            remote_address,
            remote_port,
            source,
            destination,
            block_size: 1024,
        }
    }

    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.syncer.block_size = block_size;
        self
    }

    pub fn sync(&self) -> Result<TransferResult> {
        let addr = format!("{}:{}", self.remote_address, self.remote_port);
        let mut stream = TcpStream::connect(&addr)
            .with_context(|| format!("Failed to connect to remote address: {}", addr))?;
        info!("Connected to remote server at {}", addr);

        let src_path = Path::new(&self.source);
        if !src_path.is_file() {
            return Err(anyhow::anyhow!("Only file sync supported for rsync algorithm in NetworkSyncer"));
        }
        let file_size = fs::metadata(src_path)?.len();
        let src_filename = src_path.file_name().ok_or_else(|| anyhow::anyhow!("Source file has no name"))?;
        // Send file sync request, format: FILE <src_filename> <dst_filename> <filesize>
        writeln!(stream, "FILE {} {} {}", src_filename.to_string_lossy(), self.destination, file_size)?;

        // Read server's block summary data
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut first_line = String::new();
        reader.read_line(&mut first_line)?;
        let first_line = first_line.trim_end();
        let mut block_table = Vec::new();
        if first_line == "NOBLK" {
            // Indicates destination file does not exist, cannot be reused 
            // TODO: send all data
        } else if first_line.starts_with("BLK ") {
            // Read all BLK data until BLKEND
            let mut line = first_line.to_string();
            loop {
                if line == "BLKEND" {
                    break;
                }
                // Format: BLK <offset> <size> <weak> <strong_hex>
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() != 5 {
                    return Err(anyhow::anyhow!("Invalid BLK line: {}", line));
                }
                let offset: u64 = parts[1].parse()?;
                let size: usize = parts[2].parse()?;
                let weak: u32 = parts[3].parse()?;
                let strong_hex = parts[4];
                let strong_bytes = hex::decode(strong_hex)
                    .with_context(|| format!("Failed to decode strong checksum from hex: {}", strong_hex))?;
                let mut strong = [0u8; 32];
                strong.copy_from_slice(&strong_bytes);
                block_table.push((offset, size, weak, strong));
                line.clear();
                reader.read_line(&mut line)?;
                line = line.trim_end().to_string();
            }
        } else {
            return Err(anyhow::anyhow!("Invalid response from server: {}", first_line));
        }

        // Build weak checksum lookup table: weak -> Vec<(offset, size, strong)>
        let mut weak_lookup: HashMap<u32, Vec<(u64, usize, [u8;32])>> = HashMap::new();
        for &(offset, size, weak, strong) in &block_table {
            weak_lookup.entry(weak).or_default().push((offset, size, strong));
        }

        // Scan source file using rolling window to generate diff instructions
        enum Instruction {
            Data(Vec<u8>),
            Copy(u64, usize),
        }
        let mut instructions = Vec::new();
        let mut src_file = File::open(src_path)?;
        let block_size = self.syncer.block_size;
        let mut pos: u64 = 0;
        let mut unmatched = Vec::new();

        if file_size < block_size as u64 {
            let mut data = Vec::new();
            src_file.read_to_end(&mut data)?;
            instructions.push(Instruction::Data(data));
            // pos = file_size;
        } else {
            let mut window = vec![0u8; block_size];
            src_file.read_exact(&mut window)?;
            let mut current_weak = self.syncer.calculate_weak_checksum(&window);
            while pos + block_size as u64 <= file_size {
                if let Some(candidates) = weak_lookup.get(&current_weak) {
                    let current_strong = self.syncer.calculate_strong_checksum(&window);
                    if let Some(&(blk_offset, blk_size, _)) = candidates.iter().find(|&&(_, _, s)| s == current_strong) {
                        // Found a match, first send any unmatched data
                        if !unmatched.is_empty() {
                            instructions.push(Instruction::Data(unmatched.clone()));
                            unmatched.clear();
                        }
                        instructions.push(Instruction::Copy(blk_offset, blk_size));
                        pos += block_size as u64;
                        if pos + block_size as u64 <= file_size {
                            src_file.seek(SeekFrom::Start(pos))?;
                            src_file.read_exact(&mut window)?;
                            current_weak = self.syncer.calculate_weak_checksum(&window);
                        } else {
                            break;
                        }
                        continue;
                    }
                }
                // No match found, add first byte of window to unmatched data and slide one byte
                unmatched.push(window[0]);
                pos += 1;
                if pos + block_size as u64 - 1 < file_size {
                    let old_byte = window.remove(0);
                    let mut next_byte = [0u8; 1];
                    src_file.seek(SeekFrom::Start(pos + block_size as u64 - 1))?;
                    src_file.read_exact(&mut next_byte)?;
                    window.push(next_byte[0]);
                    current_weak = self.syncer.update_weak_checksum(old_byte, next_byte[0], current_weak, block_size);
                } else {
                    break;
                }
            }
            // Add remaining data
            if pos < file_size {
                src_file.seek(SeekFrom::Start(pos))?;
                let mut remainder = Vec::new();
                src_file.read_to_end(&mut remainder)?;
                if !unmatched.is_empty() {
                    unmatched.extend(remainder);
                    instructions.push(Instruction::Data(unmatched));
                } else {
                    instructions.push(Instruction::Data(remainder));
                }
            } else if !unmatched.is_empty() {
                instructions.push(Instruction::Data(unmatched));
            }
        }
        
        for ins in instructions {
            match ins {
                Instruction::Data(data) => {
                    writeln!(stream, "DATA {}", data.len())?;
                    stream.write_all(&data)?;
                },
                Instruction::Copy(offset, length) => {
                    writeln!(stream, "COPY {} {}", offset, length)?;
                }
            }
        }
        writeln!(stream, "DONE")?;
        stream.flush()?;
        
        // TODO: reused_bytes calculation
        Ok(TransferResult { new_bytes: file_size as usize, reused_bytes: 0 })
    }

    pub fn serve(port: u16, block_size: usize) -> Result<TransferResult> {
        let listen_addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(listen_addr.clone())
            .with_context(|| format!("Failed to bind to address: {}", listen_addr))?;
        info!("Server listening on {}", listen_addr);
        let (mut stream, addr) = listener.accept()?;
        info!("Accepted connection from {:?}", addr);
        let mut reader = BufReader::new(stream.try_clone()?);
        
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end();
        let mut parts = line.split_whitespace();
        let command = parts.next().unwrap_or("");
        if command != "FILE" {
            return Err(anyhow::anyhow!("Expected FILE command, got: {}", line));
        }
        let _src_filename = parts.next().ok_or_else(|| anyhow::anyhow!("Missing src filename in FILE command"))?;
        let dst_filename = parts.next().ok_or_else(|| anyhow::anyhow!("Missing dst filename in FILE command"))?;
        let _filesize: u64 = parts.next().ok_or_else(|| anyhow::anyhow!("Missing filesize in FILE command"))?.parse()?;
        
        let target = Path::new(dst_filename);
        
        if target.exists() {
            let mut syncer = Syncer::new();
            syncer.block_size = block_size;
            let checksums = syncer.calculate_checksums(&target)?;
            for block in checksums {
                let strong_hex = hex::encode(block.strong_checksum);
                writeln!(stream, "BLK {} {} {} {}", block.offset, block.size, block.weak_checksum, strong_hex)?;
            }
            writeln!(stream, "BLKEND")?;
        } else {
            writeln!(stream, "NOBLK")?;
        }
        stream.flush()?;
        
        let temp_path = target.with_extension("tmp");
        let mut temp_file = File::create(&temp_path)?;
        let mut old_file = if target.exists() {
            Some(File::open(&target)?)
        } else {
            None
        };
        
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let line_trim = line.trim_end();
            if line_trim == "DONE" {
                break;
            }
            let mut parts = line_trim.split_whitespace();
            let cmd = parts.next().unwrap_or("");
            match cmd {
                "DATA" => {
                    let length: usize = parts.next().ok_or_else(|| anyhow::anyhow!("Missing length in DATA command"))?.parse()?;
                    let mut data = vec![0u8; length];
                    reader.read_exact(&mut data)?;
                    temp_file.write_all(&data)?;
                },
                "COPY" => {
                    let offset: u64 = parts.next().ok_or_else(|| anyhow::anyhow!("Missing offset in COPY command"))?.parse()?;
                    let length: usize = parts.next().ok_or_else(|| anyhow::anyhow!("Missing length in COPY command"))?.parse()?;
                    if let Some(ref mut f) = old_file {
                        f.seek(SeekFrom::Start(offset))?;
                        let mut buf = vec![0u8; length];
                        f.read_exact(&mut buf)?;
                        temp_file.write_all(&buf)?;
                    } else {
                        return Err(anyhow::anyhow!("COPY command received but no old file available"));
                    }
                },
                _ => {
                    return Err(anyhow::anyhow!("Unknown command: {}", cmd));
                }
            }
        }
        
        temp_file.flush()?;
        let total_bytes = fs::metadata(&temp_path)?.len() as usize;
        fs::rename(&temp_path, &target)?;
        Ok(TransferResult { new_bytes: total_bytes, reused_bytes: 0 })
    }
}
