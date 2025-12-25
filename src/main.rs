use std::process;

mod comfy_table;
mod cumulative_price_change;
mod find_tickers;
mod klines;

pub mod storage_utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║  Crypto Price Change Analysis Pipeline ║");
    println!("╚════════════════════════════════════════╝\n");

    // Step 1: Fetch exchange info and save to storage/exchange_info.json
    if let Err(e) = find_tickers::fetch_exchange_info().await {
        eprintln!("❌ Failed at Step 1: {}", e);
        process::exit(1);
    }

    if let Err(e) = klines::run().await {
        eprintln!("❌ Failed at Step 2: {}", e);
        process::exit(1);
    }

    // Step 3: Calculate cumulative price changes
    if let Err(e) = cumulative_price_change::run().await {
        eprintln!("Error running analysis: {}", e);
    }

    // Step 4: Display results in formatted table
    if let Err(e) = comfy_table::run() {
        eprintln!("❌ Failed at Step 4: {}", e);
        process::exit(1);
    }

    Ok(())
}
