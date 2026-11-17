use anyhow::{anyhow, Context};
// third-party - LEZ
use wallet::WalletCore;
use spel_framework::prelude::{seed_from_str, AccountId, ProgramId, compute_pda_multi, ToSeed};
// internal
use common::PricesContractInfo;
use oracle_prices_client::{OraclePricesClient, PublishPriceAccounts};
use crate::indexer::lon::AttestationSubmission;

pub async fn submit_attestation(pc_info: &PricesContractInfo, submission: &AttestationSubmission) -> anyhow::Result<()> {

    let wallet_core = WalletCore::from_env()
        .context("Getting wallet accounts from env")?;
    let client = OraclePricesClient::new(&wallet_core, pc_info.oracle_prices_program_id);

    let accounts = PublishPriceAccounts {
        feed_price: AccountId::default(),
    };

    // TODO: call submit_attestation (+ clock account) once oracle_prices contract is updated
    client.publish_price(accounts,
                         submission.feed_id.as_slice().try_into()?,
                         submission.price.try_into()?,
                         submission.decimals,
                         0, // valid_count: no longer part of the attestation
                         submission.round.try_into()?,
                         submission.confidence.try_into()?,
    )
        .await
        .map_err(|err| anyhow!(err))?;

    Ok(())
}
