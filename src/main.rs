mod comfy_table;
mod cumulative_price_change;
mod find_tickers;
mod klines;
mod storage_utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Step 1: Fetch Metadata
    println!("\n--- Step 1: Fetching Exchange Info ---");
    if let Err(e) = find_tickers::fetch_exchange_info().await {
        eprintln!("Error fetching info: {}", e);
        return Err(e);
    }

    // Step 2: Download Candles
    println!("\n--- Step 2: Fetching Klines ---");
    if let Err(e) = klines::run().await {
        eprintln!("Error fetching klines: {}", e);
    }

    // Step 3: Analyze Data
    println!("\n--- Step 3: Analyzing Price Changes ---");
    if let Err(e) = cumulative_price_change::run().await {
        eprintln!("Error analyzing data: {}", e);
    }

    // Step 4: Display Results
    println!("\n--- Step 4: Displaying Table ---");
    if let Err(e) = comfy_table::run().await {
        eprintln!("Error displaying table: {}", e);
    }

    Ok(())
}
