use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

// --- CONFIGURATION STRUCTS ---

#[derive(Serialize, Deserialize, Debug)]
pub struct KlineConfig {
    pub limit: u32,
    pub interval: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub klines: KlineConfig,

    // filtering logic
    #[serde(default)]
    pub filters: HashMap<String, String>,
}

// STORAGE MANAGER
pub struct AsyncStorageManager {
    pub base_dir: PathBuf,
}

impl AsyncStorageManager {
    pub async fn new_relative<P: AsRef<Path>>(relative_path: P) -> anyhow::Result<Self> {
        let exe_path = std::env::current_exe()?;
        let base_dir = exe_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Could not find binary directory"))?
            .join(relative_path);

        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).await?;
        }

        Ok(Self { base_dir })
    }

    pub async fn save<T: Serialize>(&self, filename: &str, data: &T) -> anyhow::Result<()> {
        let file_name = format!("{}.json", filename);
        let final_path = self.base_dir.join(&file_name);
        let tmp_path = self.base_dir.join(format!("{}.tmp", file_name));

        // Using pretty print as requested
        let json_bytes = serde_json::to_vec_pretty(data)?;

        tokio::fs::write(&tmp_path, json_bytes).await?;
        tokio::fs::rename(tmp_path, final_path).await?;
        Ok(())
    }

    pub async fn load<T: DeserializeOwned>(&self, filename: &str) -> anyhow::Result<T> {
        let path = self.base_dir.join(format!("{}.json", filename));
        let content = fs::read(path).await?;
        let data = serde_json::from_slice(&content)?;
        Ok(data)
    }
}
