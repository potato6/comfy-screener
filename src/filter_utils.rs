use serde_json::Value;
use std::collections::HashMap;

pub fn matches_filters(
    symbol: &serde_json::Map<String, Value>,
    filters: &HashMap<String, String>,
) -> bool {
    for (key, required_value) in filters {
        // 1. Get the value from the symbol using the config key
        match symbol.get(key) {
            Some(symbol_value) => {
                // 2. Logic: Handle different data types in the API response
                match symbol_value {
                    // Case A: The API field is a String (e.g., "status": "TRADING")
                    Value::String(s) => {
                        if s != required_value {
                            return false;
                        }
                    }
                    // Case B: The API field is an Array (e.g., "underlyingSubType": ["PoW"])
                    Value::Array(arr) => {
                        // Check if the array contains our required string
                        let found = arr.iter().any(|v| v.as_str() == Some(required_value));
                        if !found {
                            return false;
                        }
                    }
                    // Case C: Numbers/Booleans (Simple equality check via string)
                    val => {
                        if val.to_string() != *required_value {
                            return false;
                        }
                    }
                }
            }
            // Case D: The symbol doesn't even have this key -> Filter fails
            None => return false,
        }
    }
    true // All filters passed
}
