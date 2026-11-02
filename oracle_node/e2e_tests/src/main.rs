use std::path::PathBuf;
use std::time::Duration;
// third-party
use anyhow::{anyhow, Context};
use tempfile::tempdir;
use tracing::{
    info,
    debug,
    // warn,
    error,
    level_filters::LevelFilter
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
// third-party - LB
use wallet::{WalletCore, program_facades::token::Token};
use lee::{program::Program, program_deployment_transaction, public_transaction::{Message, WitnessSet}, AccountId, ProgramDeploymentTransaction, ProgramId, PublicTransaction};
use common::transaction::LeeTransaction;
use oracle_prices_client::{InitializeAccounts, OraclePricesClient, OraclePricesState};
use sequencer_service_rpc::RpcClient as _;

const TOKEN_BINARY: &str = "../../../lez-programs/programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin";

const ORACLE_PRICES_BINARY: &str = "../oracle_prices/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin";

pub fn init_wallet(storage_path: PathBuf) -> anyhow::Result<WalletCore> {

    let config_path = wallet::helperfunctions::fetch_config_path().unwrap();
    // FIXME: rm this and use provided storage_path 
    let storage_path = wallet::helperfunctions::fetch_persistent_storage_path().unwrap();

    info!("config path: {:?}", config_path);
    info!("storage path: {:?}", storage_path);

    /*
    if storage_path.exists() {
        WalletCore::new_update_chain(config_path, storage_path, None).unwrap()
    } else {
        println!("First run: initializing wallet storage at {storage_path:?}");
        WalletCore::new_init_storage(config_path, storage_path, None, "")
            .unwrap()
            .0
    }
    */

    Ok(
        WalletCore::new_init_storage(config_path, storage_path, None, "")?.0
    )
}

async fn send_deploy_tx(
    wallet_core: &WalletCore,
    program: &Program,
    program_name: &str,
    bytecode: Vec<u8>,
) -> anyhow::Result<()> {

    /*
    let deploy_msg = program_deployment_transaction::Message::new(bytecode);
    // info!("deploy_msg: {:?}", deploy_msg);
    let deploy_tx = ProgramDeploymentTransaction::new(deploy_msg);
    // info!("deploy_tx: {:?}", deploy_tx);

    match wallet_core
        .sequencer_client
        .send_transaction(LeeTransaction::ProgramDeployment(deploy_tx))
        .await
    {
        Ok(_) => info!(
            "  {} deployed (program ID: {:?})",
            program_name,
            program.id()
        ),
        Err(e) => {

            debug!("Error sending deployment transaction: {:?}", e);

            let err_str = format!("{:?}", e);
            if err_str.contains("already")
                || err_str.contains("exists")
                || err_str.contains("duplicate")
            {
                info!(
                    "  {} already deployed (program ID: {:?})",
                    program_name,
                    program.id()
                );
            } else {
                panic!("Failed to deploy {}: {:?}", program_name, e);
            }
        }
    }
    */

    let message = lee::program_deployment_transaction::Message::new(bytecode);
    let transaction = ProgramDeploymentTransaction::new(message);
    let response = wallet_core
        .sequencer_client
        .send_transaction(LeeTransaction::ProgramDeployment(transaction))
        .await;
        // .map_err()
        // .context("Transaction submission error")?;

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

    Ok(())

}

pub async fn wait_for_block_seal() {
    let secs = std::env::var("BLOCK_SEAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12); // Default in wallet (see TIME_TO_WAIT_FOR_BLOCK_SECONDS)
    tokio::time::sleep(Duration::from_secs(secs)).await;
}

async fn is_program_deployed(
    wallet_core: &WalletCore,
    program: &Program,
    account_id: &AccountId,
) -> bool {
    match wallet_core.get_account_public(account_id.clone()).await {
        Ok(account) => account.program_owner == program.id(),
        Err(_) => false,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    setup_tracing();

    // Wallet account new

    let wallet_storage_folder = tempdir()?;
    let wallet_storage_file_path = wallet_storage_folder.path().join("storage.json");
    std::fs::File::create(wallet_storage_file_path.as_path())?;
    /*
    let wallet_storage = tempfile::Builder::new()
        .prefix("storage")
        .suffix(".json")
        .rand_bytes(5)
        .tempfile()?;
    */

    let mut wallet_core = init_wallet(wallet_storage_file_path)?;

    let (account_id, _chain_index) = wallet_core.create_new_account_public(None);
    wallet_core
        .store_persistent_data()?;

    info!("account id: {}", account_id);
    info!("chain_index: {}", _chain_index);

    // End wallet create account

    // Deploy token contract

    info!("Reading token bin: {}", TOKEN_BINARY);
    let token_bytecode =
        std::fs::read(TOKEN_BINARY).context(format!("While reading {}", TOKEN_BINARY))?;
    let token_program =
        Program::new(token_bytecode.into()).context("Failed to parse token program")?;

    send_deploy_tx(&wallet_core, &token_program, "Token program", token_program.elf().to_vec()).await?;
    info!("waiting for block seal...");
    wait_for_block_seal().await;

    info!("token program id: {:?}", token_program.id());

    // let is_deployed = is_program_deployed(&wallet_core, &token_program, &account_id).await;
    // info!("is program deployed: {}", is_deployed);

    let ac_id = {
        let mut account_id_bytes = [0u8; 32];

        // Iterate through each u32 and convert it to 4 bytes
        for (i, &word) in token_program.id().iter().enumerate() {
            // to_le_bytes() ensures the correct byte order for RISC Zero/Solana
            let bytes = word.to_be_bytes();

            // Copy the 4 bytes into the correct chunk of the 32-byte array
            account_id_bytes[i * 4..(i * 4) + 4].copy_from_slice(&bytes);
        }

        AccountId::new(account_id_bytes)
    };
    let token_program_account = wallet_core.get_account_public(ac_id).await;
    info!("token program account: {:?}", token_program_account);

    // End deploy token contract

    // Oracle prices contract

    info!("Reading oracle prices bin: {}", ORACLE_PRICES_BINARY);
    let oracle_prices_bytecode =
        std::fs::read(ORACLE_PRICES_BINARY).context(format!("While reading {}", ORACLE_PRICES_BINARY))?;
    let oracle_prices_program =
        Program::new(oracle_prices_bytecode.into()).context("Failed to parse token program")?;

    send_deploy_tx(&wallet_core, &oracle_prices_program, "Oracle prices program", oracle_prices_program.elf().to_vec()).await?;
    info!("waiting for block seal...");
    wait_for_block_seal().await;

    let client = OraclePricesClient::new(&wallet_core, oracle_prices_program.id());
    info!("client program id: {:?}", client.program_id);
    // println!("client wallet: {:?}", client.wallet);

    let initAccounts = InitializeAccounts {
        oracle_prices_account: compute_pda_1(&oracle_prices_program.id(), "oracle_prices"),
    };
    info!("initAccounts acc: {:?}", initAccounts.oracle_prices_account);
    let result = client.initialize(initAccounts)
        .await
        .map_err(|err| anyhow!(err))
        .context("While calling `initialize` on oracle prices contract")?;

    info!("init result: {}", result);

    let data = client.fetch_oracle_prices_account::<OraclePricesState>()
        .await
        .map_err(|err| anyhow!(err))
        .context("While fetching oracle prices account data")?;
    info!("oracle price state feeds: {:?}", data.feeds);

    // End oracle prices contract


    Ok(())
}

fn setup_tracing() {

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let console_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(console_layer)
        .with(filter)
        .init();
}

use spel_framework::prelude::{seed_from_str, compute_pda_multi, ToSeed};

pub fn compute_pda_1(program_id: &ProgramId, literal: &str) -> AccountId {
    let tag = seed_from_str(literal);
    compute_pda_multi(program_id, &[&tag as &dyn ToSeed])
}