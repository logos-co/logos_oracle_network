use std::path::PathBuf;
use std::time::Duration;
use anyhow::Context;
use tempfile::tempdir;
// third-party - LB
use wallet::{WalletCore, program_facades::token::Token};
use lee::{
    AccountId, ProgramDeploymentTransaction, PublicTransaction,
    program::Program,
    program_deployment_transaction,
    public_transaction::{Message, WitnessSet},
};
use common::transaction::LeeTransaction;
use sequencer_service_rpc::RpcClient as _;

const TOKEN_BINARY: &str = "../../../lez-programs/programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin";

pub fn init_wallet(storage_path: PathBuf) -> anyhow::Result<WalletCore> {

    let config_path = wallet::helperfunctions::fetch_config_path().unwrap();
    // FIXME: rm this and use provided storage_path 
    let storage_path = wallet::helperfunctions::fetch_persistent_storage_path().unwrap();

    println!("config path: {:?}", config_path);
    println!("storage path: {:?}", storage_path);

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
) {
    let deploy_msg = program_deployment_transaction::Message::new(bytecode);
    let deploy_tx = ProgramDeploymentTransaction::new(deploy_msg);

    match wallet_core
        .sequencer_client
        .send_transaction(LeeTransaction::ProgramDeployment(deploy_tx))
        .await
    {
        Ok(_) => println!(
            "  {} deployed (program ID: {:?})",
            program_name,
            program.id()
        ),
        Err(e) => {

            println!("Error sending deployment transaction: {:?}", e);

            let err_str = format!("{:?}", e);
            if err_str.contains("already")
                || err_str.contains("exists")
                || err_str.contains("duplicate")
            {
                println!(
                    "  {} already deployed (program ID: {:?})",
                    program_name,
                    program.id()
                );
            } else {
                panic!("Failed to deploy {}: {:?}", program_name, e);
            }
        }
    }
}

pub async fn wait_for_block_seal() {
    let secs = std::env::var("LEZ_RLN_BLOCK_SEAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
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
    println!("Hello, world!");

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

    println!("account id: {}", account_id);
    println!("chain_index: {}", _chain_index);

    // End wallet create account

    // Deploy token contract

    let token_bytecode =
        std::fs::read(TOKEN_BINARY).context(format!("While reading {}", TOKEN_BINARY))?;
    let token_program =
        Program::new(token_bytecode.into()).context("Failed to parse token program")?;

    send_deploy_tx(&wallet_core, &token_program, "Token program", token_program.elf().to_vec()).await;
    wait_for_block_seal().await;

    let is_deployed = is_program_deployed(&wallet_core, &token_program, &account_id).await;
    println!("is program deployed: {}", is_deployed);

    // End deploy token contract

    Ok(())
}
