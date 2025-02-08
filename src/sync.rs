use anyhow::Result;
use log::{info, warn};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

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
        
        for entry in WalkDir::new(&self.source) {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                self.sync_file(path)?;
            }
        }

        info!("Sync completed");
        Ok(())
    }

    fn sync_file(&self, source_path: &Path) -> Result<()> {
        // TODO: Implement file sync logic
        Ok(())
    }

} 