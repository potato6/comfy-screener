use crate::storage_utils::AsyncStorageManager;
use anyhow::Result;
use chrono::DateTime; // Make sure 'chrono' is in Cargo.toml
use comfy_table::{
    Attribute, Cell, CellAlignment, Color, ContentArrangement, Table,
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_BORDERS_ONLY,
};
use serde::Deserialize;

// --- Data Structures (Matching results.json) ---

#[derive(Deserialize, Debug)]
struct OutputData {
    last_updated_timestamp: i64,
    results: Vec<AssetResult>,
}

#[derive(Deserialize, Debug)]
struct AssetResult {
    symbol: String,
    #[serde(rename = "subType")]
    sub_type: Vec<String>,
    movement_pct: f64,
}

// --- Visual Helpers (Restored) ---

/// Calculates a "brightness ratio" based on how close the value is to the top mover.
/// Top mover = 1.0 brightness. Smaller movers fade out to 0.4 brightness.
fn get_visibility_ratio(current_pct: f64, top_pct: f64) -> f64 {
    if top_pct == 0.0 {
        return 1.0;
    }

    let mut ratio = 0.4 + 0.6 * (current_pct / top_pct);
    if ratio < 0.4 {
        ratio = 0.4;
    }
    ratio
}

/// Converts Unix Milliseconds to "DD-MM-YYYY HH:MM:SS"
fn format_timestamp(ts_ms: i64) -> String {
    let seconds = ts_ms / 1000;
    let nanoseconds = ((ts_ms % 1000) * 1_000_000) as u32;

    if let Some(dt) = DateTime::from_timestamp(seconds, nanoseconds) {
        return dt.format("%d-%m-%Y %H:%M:%S").to_string();
    }
    "Unknown Time".to_string()
}

// --- Main Execution ---

pub async fn run() -> Result<()> {
    // 1. Initialize Storage Manager (Generic)
    let storage = AsyncStorageManager::new_relative("storage").await?;

    // 2. Load Data (Typed)
    let data: OutputData = match storage.load("results").await {
        Ok(d) => d,
        Err(_) => {
            println!("No results found. Please run the analysis first.");
            return Ok(());
        }
    };

    if data.results.is_empty() {
        println!("No data found in results.json");
        return Ok(());
    }

    // 3. Prepare Header Info
    let time_str = format_timestamp(data.last_updated_timestamp);
    let title = format!("(Data taken at {} UTC)", time_str);

    // 4. Configure Table (Visual Styles)
    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Rank").add_attribute(Attribute::Bold),
            Cell::new("Asset").add_attribute(Attribute::Bold),
            Cell::new("Type").add_attribute(Attribute::Bold),
            Cell::new("Total Movement (%)")
                .add_attribute(Attribute::Bold)
                .set_alignment(CellAlignment::Right),
        ]);

    // 5. Calculate Color Logic
    // We grab the highest % to use as our "100% brightness" baseline
    let top_mover_pct = data.results.first().map(|r| r.movement_pct).unwrap_or(1.0);
    let safe_top_pct = if top_mover_pct == 0.0 {
        1.0
    } else {
        top_mover_pct
    };

    // 6. Populate Rows (Top 15 Only)
    let mut rank = 1;
    for asset in data.results.iter().take(15) {
        let ratio = get_visibility_ratio(asset.movement_pct, safe_top_pct);

        // Calculate Faded Colors
        let cyan_val = (255.0 * ratio) as u8;
        let green_val = (255.0 * ratio) as u8;
        let gray_val = (150.0 * ratio) as u8;

        // Format Subtypes
        let subtype_str = if asset.sub_type.is_empty() {
            "N/A".to_string()
        } else {
            format!("({})", asset.sub_type.join(", "))
        };

        // Create Cells
        let rank_cell = Cell::new(rank).fg(Color::DarkGrey);

        let asset_cell = Cell::new(&asset.symbol).fg(Color::Rgb {
            r: 0,
            g: cyan_val,
            b: cyan_val,
        });

        let type_cell = Cell::new(subtype_str).fg(Color::Rgb {
            r: gray_val,
            g: gray_val,
            b: gray_val,
        });

        let pct_cell = Cell::new(format!("{:.2}%", asset.movement_pct))
            .fg(Color::Rgb {
                r: 0,
                g: green_val,
                b: 0,
            })
            .set_alignment(CellAlignment::Right);

        table.add_row(vec![rank_cell, asset_cell, type_cell, pct_cell]);

        rank += 1;
    }

    println!("\n{}\n{}", title, table);

    Ok(())
}
