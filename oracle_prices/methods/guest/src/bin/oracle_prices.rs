#![no_main]

use spel_framework::prelude::*;

risc0_zkvm::guest::entry!(main);

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct OraclePricesState {
    // TODO: for now, everybody can initialize a feed
    //       idea: restrict initialize_feed to registered oracle node
    // owner: [u8; 32],
    feeds: Vec<[u8; 32]>,
}

#[lez_program]
mod oracle_prices {

    #[allow(unused_imports)]
    use super::*;
    use oracle_prices_core::{AttestationStatus, AttestedPrice};

    /// Initialize the contract
    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("oracle_prices"))]
        mut oracle_prices_account: AccountWithMetadata,
    ) -> SpelResult {

        // println!("initialize...");

        let state = OraclePricesState {
            feeds: vec![],
        };
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        oracle_prices_account.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![oracle_prices_account], vec![]))
    }

    /// Initialize a price feed (to receive AttestedPrice from indexers)
    #[instruction]
    pub fn initialize_feed(
        #[account(mut, pda = [literal("oracle_prices")])]
        mut oracle_prices_account: AccountWithMetadata,
        #[account(init, pda = [literal("oracle_prices__"), arg("feed_id")])]
        mut feed_price: AccountWithMetadata,
        feed_id: [u8; 32],
    ) -> SpelResult {
        let price = AttestedPrice::default();
        let bytes = borsh::to_vec(&price).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        feed_price.account.data = bytes.try_into().unwrap();

        // Add feed to oracle prices state
        let data: Vec<u8> = oracle_prices_account.account.data.clone().into();
        let mut state: OraclePricesState = borsh::from_slice(&data).map_err(|e| {
            SpelError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            }
        })?;
        state.feeds.push(feed_id);
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        oracle_prices_account.account.data = bytes.try_into().unwrap();

        println!("AA feed price: {:?}", feed_price);

        Ok(SpelOutput::execute(vec![oracle_prices_account, feed_price], vec![]))
    }

    /// Submit an AttestationSubmission to a price feed (require an initialized price feed)
    /// and store it as an AttestedPrice
    #[instruction]
    #[allow(clippy::too_many_arguments)]
    pub fn submit_attestation(
        #[account(mut, pda = [literal("oracle_prices__"), arg("feed_id")])]
        mut feed_price: AccountWithMetadata,
        feed_id: [u8; 32],
        round: u64,
        price: u64,
        confidence: u64,
        set_hash: [u8; 32],
        proposer_oracle_id: [u8; 32],
        proposer_channel_id: [u8; 32],
        proposer_leaf_index: u64,
        proposer_membership_proof: Vec<u8>,
        decimals: u32,
    ) -> SpelResult {

        // TODO:
        // check "round" (only allow increasing round)?
        // pb: how to avoid an attacker to set a round too high?
        // TODO: proposer must authorize the TX + check membership proof
        //       (using proposer_channel_id, proposer_leaf_index & proposer_membership_proof)
        let _ = (proposer_channel_id, proposer_leaf_index, proposer_membership_proof);

        let price = AttestedPrice {
            feed_id,
            price,
            decimals,
            round,
            confidence,
            status: AttestationStatus::Pending,
            set_hash,
            proposer_id: proposer_oracle_id,
            written_at: 0, // TODO: read block id from the clock account
        };

        // println!("publish price: {:?}", price);

        let bytes = borsh::to_vec(&price).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        feed_price.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![feed_price], vec![]))
    }
}