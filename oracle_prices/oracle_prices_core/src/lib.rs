//use serde::{Deserialize, Serialize};

/*
/// Example state struct — customize for your program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramState {
    pub initialized: bool,
    pub owner: [u8; 32],
}
*/

use spel_framework::prelude::*;
// use borsh::{BorshDeserialize, BorshSerialize};

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct OraclePricesState {
    // TODO: for now, everybody can initialize a feed
    //       idea: restrict initialize_feed to registered oracle node
    // owner: [u8; 32],
    pub feeds: Vec<[u8; 32]>,
}

#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct PriceState {
    pub feed_id: [u8; 32], // asset pair identifier, e.g. hash("BTC/USDT")
    pub price: u64,        // attested median, real value = price * 10^(-decimals)
    pub decimals: u32,     // number of decimal places in `price`
    pub valid_count: u32,  // number of observations aggregated in this round
    pub round: u64,        // round identifier, in Bedrock block-height terms
    pub confidence: u64,   // OPTIONAL: dispersion of observations, scaled like `price`
}