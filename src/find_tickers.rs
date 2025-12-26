use crate::storage_utils::{AppConfig, AsyncStorageManager};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

// DATA

#[derive(Serialize, Deserialize)]
pub struct ExchangeInfo {
    pub symbols: Vec<Map<String, Value>>,
    #[serde(rename = "rateLimits")]
    pub rate_limits: Vec<RateLimit>,
}

#[derive(Serialize, Deserialize)]
pub struct RateLimit {
    #[serde(rename = "rateLimitType")]
    pub limit_type: String,
    pub interval: String,
    pub limit: u32,
}

// FILTER

fn matches_filters(symbol: &Map<String, Value>, filters: &HashMap<String, String>) -> bool {
    for (key, required_value) in filters {
        match symbol.get(key) {
            Some(Value::String(s)) => {
                if s != required_value {
                    return false;
                }
            }
            Some(Value::Array(arr)) => {
                if !arr.iter().any(|v| v.as_str() == Some(required_value)) {
                    return false;
                }
            }
            Some(v) => {
                if v.to_string() != *required_value {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

// MAIN

pub async fn fetch_exchange_info() -> Result<()> {
    let storage = AsyncStorageManager::new_relative("storage").await?;

    let config: AppConfig = storage.load("config").await?;

    let client = Client::new();
    let response = client
        .get("https://fapi.binance.com/fapi/v1/exchangeInfo")
        .send()
        .await?
        .error_for_status()?;

    let exchange_info: ExchangeInfo = response.json().await?;

    // We keep the logic, just remove the print statements.
    let _matching_count = exchange_info
        .symbols
        .iter()
        .filter(|s| matches_filters(s, &config.filters))
        .count();

    storage.save("exchange_info", &exchange_info).await?;

    Ok(())
}