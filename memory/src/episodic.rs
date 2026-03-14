use anyhow::Result;
use sha2::{Sha256, Digest};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone)]
pub struct MerkleBlock {
    pub previous_hash: String,
    pub event_data: String,
    pub block_hash: String,
}

/// Episodic Memory: A hash-chained Merkle Log serving as a tamper-proof audit trail.
pub struct EpisodicMemory {
    log_path: String,
}

impl EpisodicMemory {
    pub fn new(log_path: &str) -> Self {
        // Ensure the file exists
        let _ = OpenOptions::new().create(true).write(true).open(log_path);
        Self {
            log_path: log_path.to_string(),
        }
    }

    /// Appends an event to the chain
    pub fn append_event(&self, event_data: &str) -> Result<MerkleBlock> {
        let last_hash = self.get_last_hash()?;

        let mut hasher = Sha256::new();
        hasher.update(last_hash.as_bytes());
        hasher.update(event_data.as_bytes());
        let block_hash = hex::encode(hasher.finalize());

        let block = MerkleBlock {
            previous_hash: last_hash,
            event_data: event_data.to_string(),
            block_hash: block_hash.clone(),
        };

        let mut file = OpenOptions::new().append(true).open(&self.log_path)?;
        writeln!(file, "{}|{}|{}", block.previous_hash, block.event_data, block.block_hash)?;

        Ok(block)
    }

    fn get_last_hash(&self) -> Result<String> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut last_line = String::new();
        for line in reader.lines() {
            if let Ok(l) = line {
                if !l.trim().is_empty() {
                    last_line = l;
                }
            }
        }

        if last_line.is_empty() {
            // Genesis hash
            Ok("0000000000000000000000000000000000000000000000000000000000000000".to_string())
        } else {
            let parts: Vec<&str> = last_line.split('|').collect();
            if parts.len() == 3 {
                Ok(parts[2].to_string())
            } else {
                anyhow::bail!("Corrupted episodic memory log.")
            }
        }
    }
}
