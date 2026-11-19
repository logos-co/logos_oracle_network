use anyhow::{anyhow, Context};
// third-party - LEZ
use wallet::WalletCore;
use spel_framework::prelude::{seed_from_str, AccountId, ProgramId, compute_pda_multi, ToSeed};
// internal
use common::PricesContractInfo;
use oracle_prices_client::{compute_feed_price_pda, OraclePricesClient, SubmitAttestationAccounts};
use crate::indexer::lon::AttestationSubmission;

pub async fn submit_attestation(pc_info: &PricesContractInfo, submission: &AttestationSubmission) -> anyhow::Result<()> {

    let wallet_core = WalletCore::from_env()
        .context("Getting wallet accounts from env")?;
    let client = OraclePricesClient::new(&wallet_core, pc_info.oracle_prices_program_id);

    let feed_id: [u8; 32] = submission.feed_id.as_slice().try_into()?;
    let accounts = SubmitAttestationAccounts {
        feed_price: compute_feed_price_pda(&pc_info.oracle_prices_program_id, &feed_id),
    };

    // TODO: pass the clock account once oracle_prices contract reads it
    client.submit_attestation(accounts,
                              feed_id,
                              submission.round.try_into()?,
                              submission.price.try_into()?,
                              submission.confidence.try_into()?,
                              submission.set_hash.as_slice().try_into()?,
                              submission.proposer_oracle_id.as_slice().try_into()?,
                              submission.proposer_channel_id.as_slice().try_into()?,
                              submission.proposer_leaf_index,
                              submission.proposer_membership_proof.clone(),
                              submission.decimals,
    )
        .await
        .map_err(|err| anyhow!(err))?;

    Ok(())
}
