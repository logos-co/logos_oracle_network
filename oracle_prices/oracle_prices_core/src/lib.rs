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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum AttestationStatus {
    #[default]
    Pending,   // written by the proposer, dispute window open
    Final,     // window passed with no successful dispute, or the winning set confirmed the written value
    Corrected, // replaced by the median of the set that won a dispute
    Voided,    // the proposer's committed set did not hold N valid observations; the round carries no price
}

#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct AttestedPrice {
    pub feed_id: [u8; 32],         // asset pair identifier, e.g. hash("BTC/USDT")
    pub price: u64,                // attested median, real value = price * 10^(-decimals), replaced on CORRECTED
    pub decimals: u32,             // number of decimal places in `price`
    pub round: u64,                // round identifier, in Bedrock block-height terms
    pub confidence: u64,           // dispersion of the N observations, scaled like `price`
    pub status: AttestationStatus, // set only by the contract; a new write is always PENDING
    pub set_hash: [u8; 32],        // from the submission
    pub proposer_id: [u8; 32],     // oracle_id of the writer, set by the contract
    pub written_at: u64,           // LEZ block that included the submission, set by the contract
}