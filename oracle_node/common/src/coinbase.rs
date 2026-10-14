use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct CoinbaseTickerMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub product_id: Option<String>,
    pub price: Option<String>,
    // Other fields like time, volume, etc. are available
}