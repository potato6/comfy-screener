//! This module contains the core analysis pipeline logic.

use crate::{find_tickers, klines, cumulative_price_change};
use anyhow::Result;

/// Runs the full analysis pipeline:
/// 1. Fetches exchange info to get tradable symbols.
/// 2. Fetches the kline (candlestick) data for each symbol.
/// 3. Analyzes the klines to calculate cumulative price changes.
pub async fn run_analysis_pipeline() -> Result<()> {
    // Step 1: Fetch Metadata
    find_tickers::fetch_exchange_info().await?;

    // Step 2: Download Candles
    klines::run().await?;

    // Step 3: Analyze Data
    cumulative_price_change::run().await?;

    Ok(())
}
