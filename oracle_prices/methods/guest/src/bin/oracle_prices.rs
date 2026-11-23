#![no_main]

use spel_framework::prelude::*;
use oracle_prices_core::{FeedState, FeedStateError};

risc0_zkvm::guest::entry!(main);

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct OraclePricesState {
    // TODO: for now, everybody can initialize a feed
    //       idea: restrict initialize_feed to registered oracle node
    // owner: [u8; 32],
    feeds: Vec<[u8; 32]>,
}

/// Read the current block id from the LEZ clock account (must be CLOCK_01_PROGRAM_ACCOUNT_ID)
fn read_clock_block_id(clock: &AccountWithMetadata, account_index: usize) -> Result<u64, SpelError> {
    if clock.account_id != clock_core::CLOCK_01_PROGRAM_ACCOUNT_ID {
        return Err(SpelError::Custom {
            code: 0,
            message: format!("Invalid clock account: {}", clock.account_id),
        });
    }
    let data: Vec<u8> = clock.account.data.clone().into();
    let clock_data: clock_core::ClockAccountData = borsh::from_slice(&data).map_err(|e| {
        SpelError::DeserializationError {
            account_index,
            message: e.to_string(),
        }
    })?;
    Ok(clock_data.block_id)
}

fn read_feed_state(feed_price: &AccountWithMetadata, account_index: usize) -> Result<FeedState, SpelError> {
    let data: Vec<u8> = feed_price.account.data.clone().into();
    borsh::from_slice(&data).map_err(|e| SpelError::DeserializationError {
        account_index,
        message: e.to_string(),
    })
}

fn write_feed_state(feed_price: &mut AccountWithMetadata, state: &FeedState) -> Result<(), SpelError> {
    let bytes = borsh::to_vec(state).map_err(|e| SpelError::SerializationError {
        message: e.to_string(),
    })?;
    feed_price.account.data = bytes.try_into().unwrap();
    Ok(())
}

fn to_spel_error(e: FeedStateError) -> SpelError {
    SpelError::Custom {
        code: e.code(),
        message: e.to_string(),
    }
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
        write_feed_state(&mut feed_price, &FeedState::default())?;

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
    /// and store it as a PENDING AttestedPrice
    /// Note: the previous pending attestation (if any) is finalized if its dispute window is over
    #[instruction]
    #[allow(clippy::too_many_arguments)]
    pub fn submit_attestation(
        #[account(mut, pda = [literal("oracle_prices__"), arg("feed_id")])]
        mut feed_price: AccountWithMetadata,
        // LEZ clock account (CLOCK_01_PROGRAM_ACCOUNT_ID), read-only
        clock: AccountWithMetadata,
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
        // pb: how to avoid an attacker to set a round too high?
        // TODO: proposer must authorize the TX + check membership proof
        //       (using proposer_channel_id, proposer_leaf_index & proposer_membership_proof)
        let _ = (proposer_channel_id, proposer_leaf_index, proposer_membership_proof);

        let now = read_clock_block_id(&clock, 1)?;
        let mut state = read_feed_state(&feed_price, 0)?;

        let attestation = AttestedPrice {
            feed_id,
            price,
            decimals,
            round,
            confidence,
            status: AttestationStatus::Pending, // set by FeedState::submit
            set_hash,
            proposer_id: proposer_oracle_id,
            written_at: now, // set by FeedState::submit
        };
        state.submit(attestation, now).map_err(to_spel_error)?;
        write_feed_state(&mut feed_price, &state)?;

        // Clock account is read-only (but must be returned)
        Ok(SpelOutput::execute(vec![feed_price, clock], vec![]))
    }

    /// Finalize the pending attestation of a price feed (once its dispute window is over)
    /// Note: anyone can call this
    #[instruction]
    pub fn finalize_attestation(
        #[account(mut, pda = [literal("oracle_prices__"), arg("feed_id")])]
        mut feed_price: AccountWithMetadata,
        // LEZ clock account (CLOCK_01_PROGRAM_ACCOUNT_ID), read-only
        clock: AccountWithMetadata,
        feed_id: [u8; 32],
        round: u64,
    ) -> SpelResult {
        let now = read_clock_block_id(&clock, 1)?;
        let mut state = read_feed_state(&feed_price, 0)?;

        state.finalize(round, now).map_err(to_spel_error)?;
        write_feed_state(&mut feed_price, &state)?;

        // Clock account is read-only (but must be returned)
        Ok(SpelOutput::execute(vec![feed_price, clock], vec![]))
    }
}
