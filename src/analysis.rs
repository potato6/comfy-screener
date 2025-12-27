//! This module contains the core analysis pipeline logic.

use crate::{cumulative_price_change, find_tickers, klines, storage_utils::{AsyncStorageManager, AppConfig}};
use anyhow::Result;

/// Runs the full analysis pipeline:
/// 1. Fetches exchange info to get tradable symbols.
/// 2. Fetches the kline (candlestick) data for each symbol.
/// 3. Analyzes the klines to calculate cumulative price changes.
pub async fn run_analysis_pipeline() -> Result<()> {
    // Load application configuration
    let storage = AsyncStorageManager::new_relative("storage").await?;
    let app_config: AppConfig = storage.load("config").await?;

    // Step 1: Fetch Metadata
    find_tickers::fetch_exchange_info(&app_config.filters).await?;

    // Step 2: Download Candles
    klines::run(&app_config.klines, &app_config.filters).await?;

    // Step 3: Analyze Data
    cumulative_price_change::run(app_config.rsi_period).await?;

    Ok(())
}
