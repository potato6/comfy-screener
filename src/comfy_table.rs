use anyhow::Result;
use chrono::{DateTime};
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_BORDERS_ONLY, Attribute, Cell, CellAlignment,
    Color, ContentArrangement, Table,
};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct ProcessedData {
    last_updated_timestamp: i64,
    results: Vec<AssetResult>,
}

#[derive(Deserialize)]
struct AssetResult {
    symbol: String,
    #[serde(rename = "subType")]
    sub_type: Vec<String>,
    movement_pct: f64,
}

fn get_visibility_ratio(current_pct: f64, top_pct: f64) -> f64 {
    let mut ratio = 0.4 + 0.6 * (current_pct / top_pct);
    if ratio < 0.4 {
        ratio = 0.4;
    }
    ratio
}

fn format_timestamp(ts_ms: i64) -> String {
    let seconds = ts_ms / 1000;
    let nanoseconds = ((ts_ms % 1000) * 1_000_000) as u32;

    // FIX: Use DateTime::from_timestamp directly, which handles the conversion safely
    if let Some(dt) = DateTime::from_timestamp(seconds, nanoseconds) {
        return dt.format("%d-%m-%Y %H:%M:%S").to_string();
    }
    "Unknown Time".to_string()
}

pub fn run() -> Result<()> {
    let exe_path = std::env::current_exe()?;
    let base_dir = exe_path.parent().unwrap();
    let results_file = base_dir.join("storage").join("results.json");

    if !results_file.exists() {
        println!("File not found: {:?}", results_file);
        return Ok(());
    }

    let content = fs::read_to_string(results_file)?;
    let data: ProcessedData = serde_json::from_str(&content)?;

    if data.results.is_empty() {
        println!("No data found.");
        return Ok(());
    }

    let time_str = format_timestamp(data.last_updated_timestamp);
    let title = format!("(Data taken at {} UTC)", time_str);

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

    let top_mover_pct = data.results[0].movement_pct;
    let safe_top_pct = if top_mover_pct == 0.0 { 1.0 } else { top_mover_pct };

    let mut rank = 1;
    for asset in data.results.iter().take(15) {
        let ratio = get_visibility_ratio(asset.movement_pct, safe_top_pct);

        let cyan_val = (255.0 * ratio) as u8;
        let green_val = (255.0 * ratio) as u8;
        let gray_val = (150.0 * ratio) as u8;

        let subtype_str = if asset.sub_type.is_empty() {
            "N/A".to_string()
        } else {
            format!("({})", asset.sub_type.join(", "))
        };

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
