use reqwest;
use serde_json;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

/// Fetch exchange info and write storage/exchange_info.json next to the running binary
pub async fn fetch_exchange_info() -> Result<(), Box<dyn Error>> {
    // Create HTTP client
    let client = reqwest::Client::new();

    // Make GET request to Binance API
    let response = client
        .get("https://fapi.binance.com/fapi/v1/exchangeInfo")
        .send()
        .await?;

    // Ensure HTTP status is success
    let response = response.error_for_status()?;

    // Get response text
    let json_text = response.text().await?;

    // Parse then pretty-print to keep structure consistent
    let json_value: serde_json::Value = serde_json::from_str(&json_text)?;
    let pretty_json = serde_json::to_string_pretty(&json_value)?;

    // Determine the "base_dir" the other modules use (directory containing the running binary)
    let exe_path = std::env::current_exe()?;
    let base_dir = exe_path
        .parent()
        .ok_or_else(|| "Could not determine binary directory")?
        .to_path_buf();

    // storage directory next to the executable
    let storage_dir: PathBuf = base_dir.join("storage");
    fs::create_dir_all(&storage_dir)?;

    // Write to file
    let output_file = storage_dir.join("exchange_info.json");
    tokio::fs::write(&output_file, pretty_json).await?;

    println!("Exchange info saved to {:?}", output_file);
    Ok(())
}
