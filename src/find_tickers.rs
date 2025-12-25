use crate::storage_utils::AsyncStorageManager;
use serde_json::Value;

pub async fn fetch_exchange_info() -> anyhow::Result<()> {
    // 1. Setup the manager (points to binary_dir/storage)
    let storage = AsyncStorageManager::new_relative("storage").await?;

    // 2. Fetch the data
    let client = reqwest::Client::new();
    let response = client
        .get("https://fapi.binance.com/fapi/v1/exchangeInfo")
        .send()
        .await?
        .error_for_status()?;

    // 3. Parse into generic Value (or a specific Struct if you have one)
    let json_value: Value = response.json().await?;

    // 4. Save using the manager
    storage.save("exchange_info", &json_value).await?;

    println!("Exchange info saved successfully to {:?}", storage.base_dir);
    Ok(())
}
