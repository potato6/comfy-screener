use crate::storage_utils::AsyncStorageManager;
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
                let matches = if v.is_number() {
                    required_value
                        .parse::<serde_json::Number>()
                        .is_ok_and(|n| v == &serde_json::Value::Number(n))
                } else if v.is_boolean() {
                    required_value
                        .parse::<bool>()
                        .is_ok_and(|b| v == &serde_json::Value::Bool(b))
                } else if v.is_null() {
                    required_value == "null"
                } else {
                    // Fallback for other types (objects, arrays) or if required_value isn't a simple literal
                    serde_json::from_str::<serde_json::Value>(required_value)
                        .is_ok_and(|req_val| v == &req_val)
                };

                if !matches {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

// MAIN

pub async fn fetch_exchange_info(filters: &HashMap<String, String>) -> Result<()> {
    let storage = AsyncStorageManager::new_relative("storage").await?;

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
        .filter(|s| matches_filters(s, filters))
        .count();

    storage.save("exchange_info", &exchange_info).await?;

    Ok(())
}
