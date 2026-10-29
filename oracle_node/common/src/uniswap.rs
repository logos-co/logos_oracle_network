pub struct UniswapData {
    pub price: f64,
    /// Unix timestamp in milliseconds
    pub timestamp_millis: i64,
    pub feed_id: String,
    pub token_0_decimals: u8,
    pub token_1_decimals: u8,
    pub is_base_token_0: bool,
    pub feed_id_normalized: String,
    pub provider_index: usize,
}