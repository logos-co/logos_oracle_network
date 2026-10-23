use serde::Deserialize;

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
    pub price: String,
}

pub struct CoinbaseData {
    pub price: i64,
    pub timestamp: i64,
    pub feed_id: String,
    pub feed_id_normalized: String,
    pub provider_index: i32,
}