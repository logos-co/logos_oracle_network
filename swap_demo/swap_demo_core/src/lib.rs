/*
use serde::{Deserialize, Serialize};

/// Example state struct — customize for your program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramState {
    pub initialized: bool,
    pub owner: [u8; 32],
}
*/
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct PriceState {
    pub feed_id: [u8; 32], // asset pair identifier, e.g. hash("BTC/USDT")
    pub price: u64,        // attested median, real value = price * 10^(-decimals)
    pub decimals: u32,     // number of decimal places in `price`
    pub valid_count: u32,  // number of observations aggregated in this round
    pub round: u64,        // round identifier, in Bedrock block-height terms
    pub confidence: u64,   // OPTIONAL: dispersion of observations, scaled like `price`
}