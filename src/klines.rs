use crate::storage_utils::{AppConfig, AsyncStorageManager};
use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// CONSTANTS & DATA STRUCTURES

// Keys used to map the raw array response from Binance into a labeled JSON object.
// Binance returns arrays like [12345, "500.00", ...], so we need these keys to make it readable.
const KLINE_KEYS: &[&str] = &[
    "openTime",
    "open",
    "high",
    "low",
    "close",
    "volume",
    "closeTime",
    "quoteAssetVolume",
    "numberOfTrades",
    "takerBuyBaseAssetVolume",
    "takerBuyQuoteAssetVolume",
    "ignore",
];

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub klines: KlineConfig,
    // Change 'trading' to a generic map
    // We use String values for simple equality checks
    #[serde(default)]
    pub filters: HashMap<String, String>,
}

// Output Structures

// This is the clean data we will save to 'klines.json' for analysis.

#[derive(Serialize)]
struct KlineResult {
    symbol: String,
    #[serde(rename = "underlyingSubType")]
    underlying_sub_type: Vec<String>,
    // We use a Map<String, Value> because we converted the raw array into a Key-Value object
    klines: Vec<serde_json::Map<String, Value>>,
}

// HELPER FUNCTIONS

/// Calculates how much "weight" a request consumes based on the number of candles (limit).
/// Binance has strict weight limits per minute; heavier requests cost more.
fn calculate_request_weight(limit: u32) -> u32 {
    match limit {
        0..=99 => 1,
        100..=499 => 2,
        500..=1000 => 5,
        _ => 10,
    }
}

/// Fetches candle (kline) data for a single symbol.
/// Returns None if the request fails or is rate-limited, allowing the batch to continue.
async fn fetch_kline(
    client: &Client,
    symbol_data: SymbolEntry,
    params: &[(&str, String)],
) -> Option<KlineResult> {
    let url = "https://fapi.binance.com/fapi/v1/klines";

    // Append the specific symbol to the shared query parameters (limit, interval)
    let mut query = params.to_vec();
    query.push(("symbol", symbol_data.symbol.clone()));

    // Execute the HTTP GET request
    let resp = client.get(url).query(&query).send().await;

    match resp {
        Ok(response) => {
            let status = response.status();

            // -- Rate Limit Handling --
            // HTTP 418 (IP Ban) or 429 (Too Many Requests)
            if status == 418 || status == 429 {
                if let Ok(text) = response.text().await {
                    // Check for specific Binance error code -1003 (IP Ban)
                    if text.contains("-1003") {
                        let re = Regex::new(r"until\s+(\d+)").unwrap();
                        // Parse the "ban until" timestamp from the error message
                        if let Some(caps) = re.captures(&text) {
                            if let Some(ts_match) = caps.get(1) {
                                let ban_until: u64 = ts_match.as_str().parse().unwrap_or(0);
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64;

                                // If banned, sleep exactly until the ban lifts + 5 seconds buffer
                                if ban_until > now {
                                    let wait_ms = ban_until - now;
                                    let wait_sec = (wait_ms as f64 / 1000.0) + 5.0;
                                    println!(
                                        "CRITICAL: IP BANNED until {}. Sleeping {:.2}s...",
                                        ban_until, wait_sec
                                    );
                                    tokio::time::sleep(Duration::from_secs_f64(wait_sec)).await;
                                    return None;
                                }
                            }
                        }
                    }
                    println!(
                        "RATE_LIMIT_HIT ({}): {}. Cooling down...",
                        status, symbol_data.symbol
                    );
                }
                return None;
            }

            // Handle generic HTTP errors (404, 500, etc.)
            if !status.is_success() {
                println!("HTTP_ERROR: {} -> Status {}", symbol_data.symbol, status);
                return None;
            }

            // -- Data Parsing --
            // 1. Parse raw body into a vector of vectors (List of candles, where a candle is a list of values)
            match response.json::<Vec<Vec<Value>>>().await {
                Ok(raw_klines) => {
                    // 2. Map the raw values to our KLINE_KEYS (e.g., index 0 -> "openTime")
                    let klines_as_dicts: Vec<serde_json::Map<String, Value>> = raw_klines
                        .into_iter()
                        .map(|k| {
                            KLINE_KEYS
                                .iter()
                                .zip(k.into_iter())
                                .map(|(key, val)| (key.to_string(), val))
                                .collect()
                        })
                        .collect();

                    Some(KlineResult {
                        symbol: symbol_data.symbol,
                        underlying_sub_type: symbol_data.underlying_sub_type,
                        klines: klines_as_dicts,
                    })
                }
                Err(e) => {
                    println!("JSON_DECODE_ERROR: {} -> {}", symbol_data.symbol, e);
                    None
                }
            }
        }
        Err(e) => {
            println!("NETWORK_ERROR: {} -> {}", symbol_data.symbol, e);
            None
        }
    }
}

// ================================================================
// MAIN LOGIC
// ================================================================

pub async fn run() -> Result<()> {
    // 1. Initialize the Storage Manager
    // This creates the 'storage' directory next to the running executable if it doesn't exist.
    let storage = AsyncStorageManager::new_relative("storage").await?;

    // 2. Load Configuration
    // We expect 'storage/config.json' to exist. This generic load method creates a strongly-typed
    // 'AppConfig' struct, so we get compiler errors if our code expects fields that don't exist.
    println!("Loading configuration...");
    let config: AppConfig = storage.load("config").await?;

    // Extract settings from the config struct
    let limit_val = config.klines.limit;
    let interval = config.klines.interval;
    let quote_asset = config.trading.quote_asset;
    let contract_type = config.trading.contract_type;

    // Prepare shared query parameters for all API calls
    let kline_params = vec![("interval", interval), ("limit", limit_val.to_string())];

    // Calculate how expensive each request is (Binance weight)
    let weight_per_req = calculate_request_weight(limit_val);
    println!(
        "Configured Limit: {} -> Cost: {} weight/req",
        limit_val, weight_per_req
    );

    // 3. Load Exchange Information
    // This data should have been fetched by a previous step (fetch_exchange_info).
    let exchange_info: ExchangeInfo = storage.load("exchange_info").await?;

    // -- Batch Size Calculation --
    // We calculate how many requests we can safely send per minute without getting banned.
    // We aim for 90% usage of the max limit to be safe.
    let api_limit_total = exchange_info
        .rate_limits
        .iter()
        .find(|r| r.limit_type == "REQUEST_WEIGHT" && r.interval == "MINUTE")
        .map(|r| r.limit)
        .unwrap_or(2400); // Default to 2400 if not found

    let safe_capacity = (api_limit_total as f64 * 0.90) as u32;
    let batch_size = std::cmp::max(1, safe_capacity / weight_per_req) as usize;

    println!(
        "Batch Calculation: {} / {} = {} reqs/min",
        safe_capacity, weight_per_req, batch_size
    );

    // 4. Filter Symbols
    // Only keep symbols that match our config (e.g., USDT perps that are currently trading)
    let symbols_to_fetch: Vec<SymbolEntry> = exchange_info
        .symbols
        .into_iter()
        .filter(|s| {
            s.status == "TRADING"
                && s.contract_type == contract_type
                && s.quote_asset == quote_asset
        })
        .collect();

    println!("Symbols to fetch: {}", symbols_to_fetch.len());

    // 5. Data Fetching Loop
    let client = Client::builder().pool_max_idle_per_host(50).build()?;
    let mut all_results = Vec::new();

    // Iterate through symbols in chunks (batches) to respect rate limits
    for (i, batch) in symbols_to_fetch.chunks(batch_size).enumerate() {
        let start_time = Instant::now();
        let current_index = i * batch_size;

        println!(
            "Processing batch {} to {}...",
            current_index,
            current_index + batch.len()
        );

        // Create async tasks for every symbol in the current batch
        let tasks: Vec<_> = batch
            .iter()
            .map(|s| fetch_kline(&client, s.clone(), &kline_params))
            .collect();

        // Run all tasks in the batch concurrently and wait for them to finish
        let results = futures::future::join_all(tasks).await;

        // Flatten the results (remove Nones) and add to our master list
        for res in results.into_iter().flatten() {
            all_results.push(res);
        }

        // -- Rate Limit Sleep --
        // If there are more batches left, we must wait until the minute is over
        // to reset our API weight usage.
        if current_index + batch_size < symbols_to_fetch.len() {
            let elapsed = start_time.elapsed();
            if elapsed.as_secs() < 60 {
                let wait_time = Duration::from_secs(62) - elapsed; // 60s + 2s safety buffer
                println!(
                    "Batch done in {:.2?}. Waiting {:.2?}...",
                    elapsed, wait_time
                );
                tokio::time::sleep(wait_time).await;
            } else {
                println!("Batch took {:.2?}. Continuing...", elapsed);
            }
        }
    }

    // 6. Save Data
    // We use generic saving (which uses atomic writes via .tmp files).
    // Note: We pass &all_results (reference) because our storage manager handles serialization.
    println!("Saving {} klines to storage...", all_results.len());
    storage.save("klines", &all_results).await?;

    Ok(())
}
