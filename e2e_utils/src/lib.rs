//! Shared helpers for contract e2e tests (require a running LEZ sequencer)

use std::path::Path;
use std::str::FromStr;
use std::sync::Once;
use std::time::Duration;

use anyhow::Context;
use common::HashType;
use common::transaction::LeeTransaction;
use lee::ProgramDeploymentTransaction;
use lee::program::Program;
use sequencer_service_rpc::RpcClient as _;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use wallet::WalletCore;

static TRACING_INIT: Once = Once::new();

pub fn setup_test_tracing() {
    TRACING_INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    });
}

/// Init a wallet with a fresh storage (stored in `storage_dir`)
///
/// Note: wallet config is read from the usual location (see `wallet::helperfunctions::fetch_config_path`)
pub async fn init_wallet(storage_dir: &Path) -> anyhow::Result<WalletCore> {
    let config_path = wallet::helperfunctions::fetch_config_path()?;
    let storage_path = storage_dir.join("storage.json");
    let statistics_path = storage_dir.join("statistics.json");

    info!("config path: {:?}", config_path);
    info!("storage path: {:?}", storage_path);

    Ok(
        WalletCore::new_init_storage(config_path, storage_path, statistics_path, None, "").await?.0
    )
}

pub async fn send_deploy_tx(wallet_core: &WalletCore, bytecode: Vec<u8>) -> anyhow::Result<HashType> {
    let message = lee::program_deployment_transaction::Message::new(bytecode);
    let transaction = ProgramDeploymentTransaction::new(message);
    // Note: response is Ok(Tx hash) even in case of: `Transaction with hash [...] failed execution check with error: ProgramAlreadyExists, skipping it`
    let response = wallet_core
        .helm_owned()
        .send_transaction(LeeTransaction::ProgramDeployment(transaction))
        .await?;
    Ok(response)
}

/// Deploy the program read from `bin_path` (a no-op if the program is already deployed)
pub async fn deploy_contract(wallet_core: &WalletCore, bin_path: &str, program_name: &str) -> anyhow::Result<Program> {
    let bytecode = std::fs::read(bin_path).context(format!("While reading {}", bin_path))?;
    let program = Program::new(bytecode.into()).context("Failed to parse program")?;

    let tx_hash = send_deploy_tx(wallet_core, program.elf().to_vec()).await?;
    info!("{} deploy tx hash: {}", program_name, tx_hash);

    info!("Waiting for transaction (deploy) to end...");
    tokio::time::sleep(Duration::from_secs(12)).await;

    Ok(program)
}

/// Wait for a transaction (hex encoded hash) to be included in a block
///
/// Note: a transaction failing execution is skipped by the sequencer (never included in a block),
///       so give up after `max_blocks` blocks
pub async fn wait_for_tx(wallet_core: &WalletCore, tx_hash: &str, max_blocks: u64) -> anyhow::Result<()> {
    let hash = HashType::from_str(tx_hash)?;
    let client = wallet_core.helm_owned();
    let start_block = client.get_last_block_id().await?;

    loop {
        if client.get_transaction(hash).await?.is_some() {
            debug!("Transaction {} included in a block", tx_hash);
            return Ok(());
        }

        let current_block = client.get_last_block_id().await?;
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
pub async fn wait_for_block(wallet_core: &WalletCore, block_id: u64) -> anyhow::Result<()> {
    while wallet_core.helm_owned().get_last_block_id().await? < block_id {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}

/// Last block id known by the sequencer
pub async fn last_block_id(wallet_core: &WalletCore) -> anyhow::Result<u64> {
    Ok(wallet_core.helm_owned().get_last_block_id().await?)
}
