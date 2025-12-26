use crate::find_tickers::ExchangeInfo;
use crate::storage_utils::{AppConfig, AsyncStorageManager};
use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
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

#[derive(Serialize)]
struct KlineResult {
    symbol: String,
    #[serde(rename = "underlyingSubType")]
    underlying_sub_type: Vec<String>,
    klines: Vec<Map<String, Value>>,
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
    symbol_map: &Map<String, Value>,
    params: &[(&str, String)],
) -> Option<KlineResult> {
    let symbol = match symbol_map.get("symbol").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return None,
    };

    let sub_types: Vec<String> = match symbol_map.get("underlyingSubType").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => Vec::new(),
    };

    let url = "https://fapi.binance.com/fapi/v1/klines";
    let mut query = params.to_vec();
    query.push(("symbol", symbol.clone()));

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
                                if let Ok(ban_until) = ts_match.as_str().parse::<u64>() {
                                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                                    if ban_until > now {
                                        let wait_ms = ban_until - now;
                                        let wait_sec = (wait_ms as f64 / 1000.0) + 5.0;
                                        tokio::time::sleep(Duration::from_secs_f64(wait_sec)).await;
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                }
                return None;
            }

            if !status.is_success() {
                return None;
            }

            match response.json::<Vec<Vec<Value>>>().await {
                Ok(raw_klines) => {
                    let klines_as_dicts = raw_klines.into_iter().map(|k| {
                        KLINE_KEYS.iter().zip(k.into_iter()).map(|(&key, val)| (key.to_string(), val)).collect()
                    }).collect();

                    Some(KlineResult {
                        symbol,
                        underlying_sub_type: sub_types,
                        klines: klines_as_dicts,
                    })
                }
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

fn matches_filters(symbol: &Map<String, Value>, filters: &HashMap<String, String>) -> bool {
    filters.iter().all(|(key, required_value)| {
        match symbol.get(key) {
            Some(Value::String(s)) => s == required_value,
            Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(required_value)),
            Some(v) => &v.to_string() == required_value,
            None => false,
        }
    })
}

pub async fn run() -> Result<()> {
    let storage = AsyncStorageManager::new_relative("storage").await?;
    let config: AppConfig = storage.load("config").await?;
    let exchange_info: ExchangeInfo = storage.load("exchange_info").await?;

    let symbols_to_fetch: Vec<Map<String, Value>> = exchange_info
        .symbols
        .into_iter()
        .filter(|s| matches_filters(s, &config.filters))
        .collect();

    let client = Client::builder().pool_max_idle_per_host(50).build()?;
    let limit_str = config.klines.limit.to_string();
    let kline_params = vec![
        ("interval", config.klines.interval.clone()),
        ("limit", limit_str),
    ];
    let weight_per_req = calculate_request_weight(config.klines.limit);

    let api_limit_total = exchange_info
        .rate_limits
        .iter()
        .find(|r| r.limit_type == "REQUEST_WEIGHT" && r.interval == "MINUTE")
        .map(|r| r.limit)
        .unwrap_or(2400);

    let safe_capacity = (api_limit_total as f64 * 0.90) as u32;
    let batch_size = std::cmp::max(1, safe_capacity / weight_per_req) as usize;

    let mut all_results = Vec::new();

    for (i, batch) in symbols_to_fetch.chunks(batch_size).enumerate() {
        let start_time = Instant::now();
        
        let tasks: Vec<_> = batch.iter().map(|s| fetch_kline(&client, s, &kline_params)).collect();
        let results = futures::future::join_all(tasks).await;
        all_results.extend(results.into_iter().flatten());

        if i * batch_size + batch.len() < symbols_to_fetch.len() {
            let elapsed = start_time.elapsed();
            if elapsed.as_secs() < 60 {
                let wait = Duration::from_secs(62) - elapsed;
                tokio::time::sleep(wait).await;
            }
        }
    }

    storage.save("klines", &all_results).await?;
    Ok(())
}