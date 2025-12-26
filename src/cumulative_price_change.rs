use crate::storage_utils::AsyncStorageManager;
use anyhow::Result;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

// --- Data Structures & Custom Deserialization (Unchanged) ---

#[derive(Deserialize, Debug)]
struct InputKline {
    #[serde(deserialize_with = "deserialize_f64_lenient")]
    open: Option<f64>,
    #[serde(deserialize_with = "deserialize_f64_lenient")]
    close: Option<f64>,
    #[serde(rename = "closeTime")]
    close_time: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct SymbolData {
    symbol: String,
    #[serde(default)]
    klines: Vec<InputKline>,
    #[serde(rename = "underlyingSubType", default)]
    underlying_sub_type: Vec<String>,
}

#[derive(Serialize, Debug)]
struct ResultItem {
    symbol: String,
    movement_pct: f64,
    #[serde(rename = "subType")]
    sub_type: Vec<String>,
}

#[derive(Serialize, Debug)]
struct OutputData {
    last_updated_timestamp: i64,
    results: Vec<ResultItem>,
}

struct LenientF64Visitor;

impl<'de> Visitor<'de> for LenientF64Visitor {
    type Value = Option<f64>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a float, an integer, or a string representing a number")
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(Some(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Some(v as f64))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Some(v as f64))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if v.trim().is_empty() {
            Ok(None)
        } else {
            v.parse::<f64>().map(Some).map_err(E::custom)
        }
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

fn deserialize_f64_lenient<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(LenientF64Visitor)
}

// --- Domain Logic (Unchanged) ---

fn analyze_klines_data(klines: &[InputKline]) -> Option<(f64, i64)> {
    if klines.is_empty() {
        return None;
    }

    let is_valid = |k: &&InputKline| k.open.is_some() && k.close.is_some() && k.close_time.is_some();
    let first_kline = klines.iter().find(is_valid)?;
    let last_kline = klines.iter().rfind(is_valid)?;

    let first_close = first_kline.close?;
    let last_close = last_kline.close?;
    let last_close_time = last_kline.close_time?;

    if first_close == 0.0 {
        return None;
    }

    let cumulative_return = ((last_close / first_close) - 1.0) * 100.0;

    Some((cumulative_return, last_close_time))
}

// --- Main Execution (Refactored) ---

pub async fn run() -> Result<()> {
    let storage = AsyncStorageManager::new_relative("storage").await?;

    let all_symbols_data: Vec<SymbolData> = match storage.load("klines").await {
        Ok(data) => data,
        Err(_) => {
            // Silently return if file doesn't exist, as the TUI will show empty state.
            return Ok(());
        }
    };

    let mut results = Vec::with_capacity(all_symbols_data.len());
    let mut max_close_time = 0;

    for symbol_data in all_symbols_data {
        if let Some((movement_pct, last_close_time)) = analyze_klines_data(&symbol_data.klines) {
            results.push(ResultItem {
                symbol: symbol_data.symbol,
                movement_pct,
                sub_type: symbol_data.underlying_sub_type,
            });

            if last_close_time > max_close_time {
                max_close_time = last_close_time;
            }
        }
    }

    results.sort_unstable_by(|a, b| {
        b.movement_pct
            .partial_cmp(&a.movement_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if results.is_empty() {
        return Ok(());
    }

    let output_data = OutputData {
        last_updated_timestamp: max_close_time,
        results,
    };

    storage.save("results", &output_data).await?;

    Ok(())
}