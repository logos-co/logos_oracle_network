use std::path::PathBuf;
use std::sync::Once;
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
