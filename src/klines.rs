use crate::storage_utils::AsyncStorageManager;
use anyhow::Result;
use configparser::ini::Ini;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

#[derive(Deserialize)]
struct ExchangeInfo {
    symbols: Vec<SymbolEntry>,
    #[serde(rename = "rateLimits")]
    rate_limits: Vec<RateLimit>,
}

#[derive(Deserialize)]
struct RateLimit {
    #[serde(rename = "rateLimitType")]
    limit_type: String,
    interval: String,
    limit: u32,
}

#[derive(Deserialize, Clone)]
struct SymbolEntry {
    symbol: String,
    status: String,
    #[serde(rename = "contractType")]
    contract_type: String,
    #[serde(rename = "quoteAsset")]
    quote_asset: String,
    #[serde(rename = "underlyingSubType", default)]
    underlying_sub_type: Vec<String>,
}

#[derive(Serialize)]
struct KlineResult {
    symbol: String,
    #[serde(rename = "underlyingSubType")]
    underlying_sub_type: Vec<String>,
    klines: Vec<serde_json::Map<String, Value>>,
}

fn calculate_request_weight(limit: u32) -> u32 {
    match limit {
        0..=99 => 1,
        100..=499 => 2,
        500..=1000 => 5,
        _ => 10,
    }
}

async fn fetch_kline(
    client: &Client,
    symbol_data: SymbolEntry,
    params: &[(&str, String)],
) -> Option<KlineResult> {
    let url = "https://fapi.binance.com/fapi/v1/klines";

    let mut query = params.to_vec();
    query.push(("symbol", symbol_data.symbol.clone()));

    let resp = client.get(url).query(&query).send().await;

    match resp {
        Ok(response) => {
            let status = response.status();

            if status == 418 || status == 429 {
                if let Ok(text) = response.text().await {
                    if text.contains("-1003") {
                        let re = Regex::new(r"until\s+(\d+)").unwrap();
                        if let Some(caps) = re.captures(&text) {
                            if let Some(ts_match) = caps.get(1) {
                                let ban_until: u64 = ts_match.as_str().parse().unwrap_or(0);
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64;

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

            if !status.is_success() {
                println!("HTTP_ERROR: {} -> Status {}", symbol_data.symbol, status);
                return None;
            }

            match response.json::<Vec<Vec<Value>>>().await {
                Ok(raw_klines) => {
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

pub async fn run() -> Result<()> {
    // 1. Initialize Storage Manager (creates ../storage automatically)
    let storage = AsyncStorageManager::new_relative("storage").await?;

    // 2. Load Config (INI is not JSON, so we handle it manually, but use storage path as anchor)
    // storage.base_dir is ".../storage", so parent is the binary folder.
    let config_path = storage
        .base_dir
        .parent()
        .ok_or(anyhow::anyhow!("Invalid storage path"))?
        .join("config.ini");

    let mut config = Ini::new();
    let config_str = config_path
        .to_str()
        .ok_or(anyhow::anyhow!("Invalid config path"))?;

    if config.load(config_str).is_err() {
        return Err(anyhow::anyhow!("config.ini not found at {:?}", config_path));
    }

    let limit_val: u32 = config
        .get("klines", "limit")
        .unwrap_or("500".to_string())
        .parse()?;
    let interval = config.get("klines", "interval").unwrap_or("1m".to_string());

    let kline_params = vec![("interval", interval), ("limit", limit_val.to_string())];

    let weight_per_req = calculate_request_weight(limit_val);
    println!(
        "Configured Limit: {} -> Cost: {} weight/req",
        limit_val, weight_per_req
    );

    // 3. Load Exchange Info using Generic Manager (Strictly Typed)
    // This replaces manual fs::read and serde_json::from_slice
    let exchange_info: ExchangeInfo = storage.load("exchange_info").await?;

    let api_limit_total = exchange_info
        .rate_limits
        .iter()
        .find(|r| r.limit_type == "REQUEST_WEIGHT" && r.interval == "MINUTE")
        .map(|r| r.limit)
        .unwrap_or(2400);

    let safe_capacity = (api_limit_total as f64 * 0.90) as u32;
    let batch_size = std::cmp::max(1, safe_capacity / weight_per_req) as usize;

    println!(
        "Batch Calculation: {} / {} = {} reqs/min",
        safe_capacity, weight_per_req, batch_size
    );

    let symbols_to_fetch: Vec<SymbolEntry> = exchange_info
        .symbols
        .into_iter()
        .filter(|s| {
            s.status == "TRADING" && s.contract_type == "PERPETUAL" && s.quote_asset == "USDT"
        })
        .collect();

    println!("Symbols to fetch: {}", symbols_to_fetch.len());

    let client = Client::builder().pool_max_idle_per_host(50).build()?;
    let mut all_results = Vec::new();

    for (i, batch) in symbols_to_fetch.chunks(batch_size).enumerate() {
        let start_time = Instant::now();
        let current_index = i * batch_size;

        println!(
            "Processing batch {} to {}...",
            current_index,
            current_index + batch.len()
        );

        let tasks: Vec<_> = batch
            .iter()
            .map(|s| fetch_kline(&client, s.clone(), &kline_params))
            .collect();

        let results = futures::future::join_all(tasks).await;
        for res in results.into_iter().flatten() {
            all_results.push(res);
        }

        if current_index + batch_size < symbols_to_fetch.len() {
            let elapsed = start_time.elapsed();
            if elapsed.as_secs() < 60 {
                let wait_time = Duration::from_secs(62) - elapsed; // 60s + 2s buffer
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

    // 4. Save Results using Generic Manager (Atomic Write)
    println!("Saving {} klines to storage...", all_results.len());
    storage.save("klines", &all_results).await?;

    Ok(())
}
