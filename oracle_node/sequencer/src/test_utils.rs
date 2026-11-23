use std::path::PathBuf;
#[cfg(test)]
use std::str::FromStr;
use std::sync::Once;
#[cfg(test)]
use std::time::Duration;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use wallet::WalletCore;
use lee::{program::Program, program_deployment_transaction, public_transaction::{Message, WitnessSet}, AccountId, ProgramDeploymentTransaction, ProgramId, PublicTransaction};
use lez_common::HashType;
use lez_common::transaction::LeeTransaction;
use sequencer_service_rpc::RpcClient as _;


static TRACING_INIT: Once = Once::new();

pub fn setup_test_tracing() {
    TRACING_INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    });
}

pub fn init_wallet(storage_path: PathBuf) -> anyhow::Result<WalletCore> {

    let config_path = wallet::helperfunctions::fetch_config_path()?;
    // FIXME: rm this and use provided storage_path
    // let storage_path = wallet::helperfunctions::fetch_persistent_storage_path()?;

    info!("config path: {:?}", config_path);
    info!("storage path: {:?}", storage_path);

    Ok(
        WalletCore::new_init_storage(config_path, storage_path, None, "")?.0
    )
}

#[cfg(test)]
pub(crate) async fn send_deploy_tx(
    wallet_core: &WalletCore,
    program: &Program,
    program_name: &str,
    bytecode: Vec<u8>,
) -> anyhow::Result<HashType> {

    let message = lee::program_deployment_transaction::Message::new(bytecode);
    let transaction = ProgramDeploymentTransaction::new(message);
    let response = wallet_core
        .sequencer_client
        .send_transaction(LeeTransaction::ProgramDeployment(transaction))
        .await?;
    // .map_err()
    // .context("Transaction submission error")?;

    /*
    // Note: response is Ok(Tx hash) even in case of: `Transaction with hash [...] failed execution check with error: ProgramAlreadyExists, skipping it`
    info!("response: {:?}", response);

    if let Err(e) = response {
        let err_str = format!("{:?}", e);
        info!("err_str: {}", err_str);
        if err_str.contains("already")
            || err_str.contains("exists")
            || err_str.contains("duplicate")
        {
            debug!("Program already deployed");
        } else {
            return Err(e.into());
        }
    }
    */

    Ok(response)

}

/// Wait for a transaction (hex encoded hash) to be included in a block
///
/// Note: a transaction failing execution is skipped by the sequencer (never included in a block),
///       so give up after `max_blocks` blocks
#[cfg(test)]
pub(crate) async fn wait_for_tx(wallet_core: &WalletCore, tx_hash: &str, max_blocks: u64) -> anyhow::Result<()> {

    let hash = HashType::from_str(tx_hash)?;
    let start_block = wallet_core.sequencer_client.get_last_block_id().await?;

    loop {
        if wallet_core.sequencer_client.get_transaction(hash).await?.is_some() {
            debug!("Transaction {} included in a block", tx_hash);
            return Ok(());
        }

        let current_block = wallet_core.sequencer_client.get_last_block_id().await?;
        if current_block > start_block + max_blocks {
            anyhow::bail!(
                "Transaction {} not included after {} blocks (likely failed execution, check sequencer logs)",
                tx_hash, max_blocks
            );
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Wait for the sequencer to reach block id `block_id`
#[cfg(test)]
pub(crate) async fn wait_for_block(wallet_core: &WalletCore, block_id: u64) -> anyhow::Result<()> {
    while wallet_core.sequencer_client.get_last_block_id().await? < block_id {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}
