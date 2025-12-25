use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct AsyncStorageManager {
    pub base_dir: PathBuf,
}

impl AsyncStorageManager {
    /// Initialize and ensure directory exists immediately
    pub async fn new_relative<P: AsRef<Path>>(relative_path: P) -> anyhow::Result<Self> {
        let exe_path = std::env::current_exe()?;
        let base_dir = exe_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not find binary directory"))?
            .join(relative_path);

        // Optimization: Create directory ONCE during init, not every save
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).await?;
        }
        
        Ok(Self { base_dir })
    }

    /// Generic save with ATOMIC WRITE strategy
    pub async fn save<T: Serialize>(&self, filename: &str, data: &T) -> anyhow::Result<()> {
        let file_name = format!("{}.json", filename);
        let final_path = self.base_dir.join(&file_name);
        
        // 1. Write to a temporary file first (e.g., "exchange_info.json.tmp")
        let tmp_filename = format!("{}.tmp", file_name);
        let tmp_path = self.base_dir.join(&tmp_filename);

        // Optimization: Use to_vec_pretty (bytes) instead of to_string (utf-8 string)
        let json_bytes = serde_json::to_vec_pretty(data)?;
        
        // 2. Write data to the temp file
        fs::write(&tmp_path, json_bytes).await?;

        // 3. Atomically rename tmp -> final. 
        // If the program crashes before this line, the old file is untouched.
        fs::rename(tmp_path, final_path).await?;
        
        Ok(())
    }

    pub async fn load<T: DeserializeOwned>(&self, filename: &str) -> anyhow::Result<T> {
        let path = self.base_dir.join(format!("{}.json", filename));
        
        // Optimization: Read directly into bytes
        let content = fs::read(path).await?;
        
        // Parse from bytes (slightly faster than from_str)
        let data = serde_json::from_slice(&content)?;
        Ok(data)
    }
}
