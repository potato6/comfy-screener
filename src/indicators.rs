use crate::cumulative_price_change::InputKline;
use ta::Next;
use ta::indicators::RelativeStrengthIndex;

pub fn calculate_rsi(klines: &[InputKline], period: u32) -> Option<f64> {
    let mut rsi_indicator = RelativeStrengthIndex::new(period as usize).ok()?;

    let close_prices: Vec<f64> = klines.iter().filter_map(|kline| kline.close).collect();

    if close_prices.len() < period as usize {
        return None;
    }

    let mut last_rsi: Option<f64> = None;
    for price in close_prices {
        last_rsi = Some(rsi_indicator.next(price));
    }
    last_rsi
}
