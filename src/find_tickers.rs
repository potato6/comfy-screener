use reqwest;
use serde_json;
use std::fs;
use std::error::Error;

/// Fetch exchange info and write storage/exchange_info.json
pub async fn fetch_exchange_info() -> Result<(), Box<dyn Error>> {
    // Create HTTP client
    let client = reqwest::Client::new();

    // Make GET request to Binance API
    let response = client
        .get("https://fapi.binance.com/fapi/v1/exchangeInfo")
        .send()
        .await?;

    // Check if response is successful
    let response = response.error_for_status()?;

    // Get response text
    let json_text = response.text().await?;

    // Parse JSON to preserve structure
    let json_value: serde_json::Value = serde_json::from_str(&json_text)?;

    // Pretty print JSON with indentation
    let pretty_json = serde_json::to_string_pretty(&json_value)?;

    // Create storage directory (next to current working dir / exe dir depending on how you run)
    let current_dir = std::env::current_dir()?;
    let storage_dir = current_dir.join("storage");
    fs::create_dir_all(&storage_dir)?;

    // Write to file
    let output_file = storage_dir.join("exchange_info.json");
    tokio::fs::write(&output_file, pretty_json).await?;

    println!("Exchange info saved to {:?}", output_file);
    Ok(())
}
