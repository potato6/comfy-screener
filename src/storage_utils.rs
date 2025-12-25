use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

// CONFIGURATION STRUCTS
// into these Rust types without manual parsing.

#[derive(Serialize, Deserialize, Debug)]
pub struct KlineConfig {
    pub limit: u32,       // e.g., 500 candles
    pub interval: String, // e.g., "1m", "15m", "4h"
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TradingConfig {
    pub quote_asset: String,   // e.g., "USDT"
    pub contract_type: String, // e.g., "PERPETUAL"
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    // Nested structs organize the config logically
    pub klines: KlineConfig,
    pub trading: TradingConfig,
}

// STORAGE MANAGER

pub struct AsyncStorageManager {
    // Stores the absolute path to the storage directory (e.g., ".../target/debug/storage")
    pub base_dir: PathBuf,
}

impl AsyncStorageManager {
    /// **Constructor: new_relative**
    /// Creates a new manager instance. It calculates the storage path relative
    /// to where the binary executable is running.
    pub async fn new_relative<P: AsRef<Path>>(relative_path: P) -> anyhow::Result<Self> {
        // 1. Locate the running executable
        let exe_path = std::env::current_exe()?;

        // 2. Resolve the parent directory and append the relative path (e.g., "storage")
        let base_dir = exe_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not find binary directory"))?
            .join(relative_path);

        // 3. Create the directory immediately upon startup.
        //    Doing this here prevents us from having to check if the folder exists
        //    every single time we try to save a file later.
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).await?;
        }

        Ok(Self { base_dir })
    }

    /// **Generic Save Function**
    /// Takes any struct that implements `Serialize` and saves it to a JSON file.
    /// Implements an "Atomic Write" strategy to prevent data corruption.
    pub async fn save<T: Serialize>(&self, filename: &str, data: &T) -> anyhow::Result<()> {
        let file_name = format!("{}.json", filename);
        let final_path = self.base_dir.join(&file_name);

        // We write to a .tmp file first. If the program crashes while writing,
        // the original file remains untouched and valid.
        let tmp_path = self.base_dir.join(format!("{}.tmp", file_name));

        // Serialization
        // CHANGE: Used to be `to_vec` (minified).
        // Now using `to_vec_pretty` to make it human-readable.
        let json_bytes = serde_json::to_vec_pretty(data)?;

        // 1. Write data to the temporary file
        tokio::fs::write(&tmp_path, json_bytes).await?;

        // 2. Atomically rename the temp file to the final name.
        tokio::fs::rename(tmp_path, final_path).await?;

        Ok(())
    }

    /// **Generic Load Function**
    /// Takes a filename and a target Type (T), reads the file, and deserializes it.
    /// T must implement `DeserializeOwned` (meaning it can be created purely from the data).
    pub async fn load<T: DeserializeOwned>(&self, filename: &str) -> anyhow::Result<T> {
        let path = self.base_dir.join(format!("{}.json", filename));

        // Read directly into bytes (`Vec<u8>`) instead of a String.
        // `read_to_string` forces a UTF-8 validation scan which is slow and unnecessary
        // because serde_json will scan the bytes anyway during parsing.
        let content = fs::read(path).await?;

        // Parse the raw bytes into the specific Rust struct (T)
        let data = serde_json::from_slice(&content)?;
        Ok(data)
    }
}
