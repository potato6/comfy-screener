use anyhow::Result;
use std::process;

mod find_tickers;
mod klines;
mod cumulative_price_change;
mod comfy_table;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║  Crypto Price Change Analysis Pipeline ║");
    println!("╚════════════════════════════════════════╝\n");

    // Step 1: Fetch exchange info and save to storage/exchange_info.json
    println!("📥 Step 1: Fetching ticker information...");
    if let Err(e) = find_tickers::fetch_exchange_info().await {
        eprintln!("❌ Failed at Step 1: {}", e);
        process::exit(1);
    }
    println!("✅ Step 1 complete:  Tickers saved\n");

    // Step 2: Fetch kline data for all symbols
    println!("📊 Step 2: Fetching kline data for all symbols...");
    if let Err(e) = klines::run().await {
        eprintln!("❌ Failed at Step 2: {}", e);
        process::exit(1);
    }
    println!("✅ Step 2 complete:  Klines saved\n");

    // Step 3: Calculate cumulative price changes
    println!("📈 Step 3: Calculating cumulative price changes...");
    if let Err(e) = cumulative_price_change::run() {
        eprintln!("❌ Failed at Step 3: {}", e);
        process::exit(1);
    }
    println!("✅ Step 3 complete: Results calculated\n");

    // Step 4: Display results in formatted table
    println!("📊 Step 4: Displaying results..\n");
    if let Err(e) = comfy_table::run() {
        eprintln!("❌ Failed at Step 4: {}", e);
        process::exit(1);
    }

    Ok(())
}
