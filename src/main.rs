mod analysis;
mod cumulative_price_change;
mod find_tickers;
mod klines;
mod storage_utils;
mod tui;
mod indicators;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Directly launch the TUI.
    if let Err(e) = tui::run_tui().await {
        // If the TUI exits with an error, print it to stderr.
        // Benign "Quit" errors are suppressed.
        if !e.to_string().contains("Quit") {
            eprintln!("TUI Error: {}", e);
        }
    }
    Ok(())
}
