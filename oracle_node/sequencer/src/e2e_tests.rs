use std::path::PathBuf;
use std::str::FromStr;
use lee::{AccountId, PublicTransaction};
use lee::program::Program;
use lee::public_transaction::{Message, WitnessSet};
use lez_common::transaction::LeeTransaction;
use oracle_register_client::{OracleRegisterClient, RegisterState, InitializeAccounts};
use sequencer_service_rpc::RpcClient;
use super::*;
use serial_test::serial;
use spel_framework::prelude::BorshDeserialize;
use crate::test_utils::{init_wallet, send_deploy_tx, setup_test_tracing};
use tempfile::tempdir;
use wallet::WalletCore;
use crate::register_contract::{compute_pda_from_literal, compute_vault_pda};
use sequencer_service_rpc::RpcClient as _;
use token_core::{TokenDefinition, TokenHolding};
use tracing::warn;
use token_client::TokenClient;
use crate::token_client::{NewFungibleDefinitionAccounts, TransferAccounts};

const TOKEN_BIN: &str = "../../../../lez-programs/programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin";

const ORACLE_REGISTER_BIN: &str = "../../oracle_register/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin";
const ORACLE_PRICES_BIN: &str = "../oracle_prices/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin";

#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "e2e_lbc"), ignore = "requires a running Logos blockchain (enable feature e2e_lbc)")]
async fn test_e2e_1() -> anyhow::Result<()> {
    setup_test_tracing();

    let wallet_storage_folder = tempdir()?;
    let wallet_storage_file_path = wallet_storage_folder.path().join("storage.json");
    std::fs::File::create(wallet_storage_file_path.as_path())?;
    let mut wallet_core = init_wallet(wallet_storage_file_path)?;

    info!("Wallet core initialized...");

    let (account_id, _chain_index) = wallet_core.create_new_account_public(None);
    wallet_core
        .store_persistent_data()?;
    info!("account id: {}", account_id);
    info!("chain_index: {}", _chain_index);

    let (lon_def_account_id, _chain_index) = wallet_core.create_new_account_public(None);
    wallet_core
        .store_persistent_data()?;
    info!("lon def account id: {}", lon_def_account_id);

    let (lon_holder_account_id, _chain_index) = wallet_core.create_new_account_public(None);
    wallet_core
        .store_persistent_data()?;
    info!("Lon holder account id: {}", lon_holder_account_id);

    let (lon_sequencer_account_id, _chain_index) = wallet_core.create_new_account_public(None);
    wallet_core
        .store_persistent_data()?;
    info!("Sequencer account id (for staking): {}", lon_sequencer_account_id);

    info!("Reading token bin: {}", TOKEN_BIN);

    let token_program = deploy_contract(&wallet_core, TOKEN_BIN, "token").await?;

    let token_client = TokenClient::new(&wallet_core, token_program.id());
    init_token_program_for_lon(&token_client, &token_program, &lon_def_account_id, &lon_holder_account_id).await?;
    transfer_lon_token_to(&token_client, &lon_def_account_id, &lon_holder_account_id, &lon_sequencer_account_id, 1_000).await?;

    info!("token program id: {:?}", token_program.id());

    info!("Reading oracle_register bin: {}", ORACLE_REGISTER_BIN);
    let oracle_register_program = deploy_contract(&wallet_core, ORACLE_REGISTER_BIN, "oracle register").await?;
    info!("oracle register program id: {:?}", oracle_register_program.id());

    let oracle_register_client = OracleRegisterClient::new(&wallet_core, oracle_register_program.id());

    init_register_program(
        &oracle_register_client,
        &token_program,
        &oracle_register_program,
        lon_holder_account_id,
    ).await?;

    Ok(())
}

async fn deploy_contract(wallet_core: &WalletCore, bin_path: &str, program_name: &str) -> anyhow::Result<Program> {

    let bytecode =
        fs::read(bin_path).context(format!("While reading {}", bin_path))?;
    let program =
        Program::new(bytecode.into()).context("Failed to parse program")?;

    let tx_hash = send_deploy_tx(&wallet_core, &program, program_name, program.elf().to_vec()).await?;
    info!("tx hash: {}", tx_hash);

    info!("Waiting for transaction (deploy) to end...");
    tokio::time::sleep(Duration::from_secs(12)).await;

    Ok(program)
}

async fn init_token_program_for_lon(client: &TokenClient<'_>, token_program: &Program, lon_def: &AccountId, lon_holder: &AccountId) -> anyhow::Result<()> {

    let accounts = NewFungibleDefinitionAccounts {
        definition_target_account: *lon_def,
        holding_target_account: *lon_holder,
    };
    client.new_fungible_definition(accounts, "LON token".to_string(), 21_000, None).await
        .map_err(|e| anyhow!(e))?;

    info!("Waiting for transaction (new fungible def) to end...");
    tokio::time::sleep(Duration::from_secs(12 * 3)).await;

    {
        let account = client.wallet.sequencer_client
            .get_account(*lon_def).await
            .map_err(|e| anyhow!("get account: {}", e))?;
        // println!("account for {:?}: {:?}", register, account);
        let r = TokenDefinition::try_from_slice(&account.data);
            // .map_err(|e| anyhow!("deserialize register: {}", e))?;
        info!("Lon def: {:?}", r);
    };

    // Use get_token_holding
    {
        let account = client.wallet.sequencer_client
            .get_account(*lon_holder).await
            .map_err(|e| anyhow!("get account: {}", e))?;
        // println!("account for {:?}: {:?}", register, account);
        let r = TokenHolding::try_from_slice(&account.data);
        // .map_err(|e| anyhow!("deserialize register: {}", e))?;
        info!("Lon holder: {:?}", r);
    };

    Ok(())
}

async fn get_token_holding(client: &TokenClient<'_>, account: &AccountId) -> anyhow::Result<TokenHolding> {
    let account = client.wallet.sequencer_client
        .get_account(*account)
        .await
        .map_err(|e| anyhow!("get account: {}", e))?;
    TokenHolding::try_from_slice(&account.data)
        .map_err(|e| anyhow!("deserialize token holding: {}", e))
}

async fn transfer_lon_token_to(client: &TokenClient<'_>, lon_def: &AccountId, lon_holder: &AccountId, to: &AccountId, amount: u128) -> anyhow::Result<()> {

    if get_token_holding(&client, &to).await.is_err() {
        let acc_0 = token_client::InitializeAccountAccounts {
            definition_account: *lon_def,
            account_to_initialize: *to,
        };

        if let Err(e) = client.initialize_account(acc_0).await {
            warn!("Failed to initialize account: {}", e);
        }

        info!("Waiting for transaction (init account) to end...");
        tokio::time::sleep(Duration::from_secs(15)).await;
    }

    let acc = TransferAccounts {
        sender: *lon_holder,
        recipient: *to,
    };
    client.transfer(acc, amount).await
        .map_err(|e| anyhow!("transfer error: {}", e))?;

    info!("Waiting for transaction (transfer) to end...");
    tokio::time::sleep(Duration::from_secs(15)).await;

    Ok(())
}

async fn init_register_program(client: &OracleRegisterClient<'_>, token_program: &Program, oracle_register_program: &Program, owner: AccountId) -> anyhow::Result<()> {
    if let Err(e) = client.fetch_register::<RegisterState>().await {
        info!("Oracle register not initialized, doing it now...");

        // TODO: export literal in oracle_register_client?
        let register: AccountId = compute_pda_from_literal(&oracle_register_program.id(), "register");
        info!("register: {} - {:?}", register, register);
        let init_accounts = InitializeAccounts {
            register,
            owner,
        };

        let r = client.initialize(init_accounts, token_program.id()).await
            .map_err(|e| anyhow!(e))?;
        info!("init reg result: {:?}", r);

        info!("Waiting for transaction (initialize) to end...");
        tokio::time::sleep(Duration::from_secs(12 + 6)).await;
        client.fetch_register::<RegisterState>().await
            .map_err(|e| anyhow!(e))?;

    } else {
        info!("Oracle register contract already initialized...");
    }

    Ok(())
}
