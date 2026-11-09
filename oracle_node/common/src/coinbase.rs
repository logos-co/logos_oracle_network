use serde::Deserialize;
use rust_decimal::Decimal;

#[derive(Deserialize, Debug)]
pub struct CoinbaseMessage {
    pub channel: String,
    pub timestamp: String,
    pub events: Vec<CoinbaseEvent>,
}

#[derive(Deserialize, Debug)]
pub struct CoinbaseEvent {
    #[serde(rename = "type")]
    event_type: String,
    pub tickers: Vec<CoinbaseTicker>,
}

#[derive(Deserialize, Debug)]
pub struct CoinbaseTicker {
    pub product_id: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
}

pub struct CoinbaseData {
    pub price: Decimal,
    /// Unix timestamp in milliseconds
    pub timestamp_millis: i64,
    pub feed_id: String,
    pub feed_id_normalized: String,
    pub provider_index: usize,
}