#[path = "common/token_client.rs"]
#[allow(dead_code, unused)]
mod token_client;

use anyhow::anyhow;
use e2e_utils::{deploy_contract, init_wallet, setup_test_tracing, wait_for_tx};
use nssa::AccountId;
use nssa::program::Program;
use oracle_register_client::{compute_register_pda, InitializeAccounts, OracleRegisterClient, RegisterState};
use serial_test::serial;
use spel_framework::prelude::BorshDeserialize;
use tempfile::tempdir;
use token_client::{InitializeAccountAccounts, NewFungibleDefinitionAccounts, TokenClient, TransferAccounts};
use token_core::{TokenDefinition, TokenHolding};
use tracing::{info, warn};

const TOKEN_BIN: &str = "../../../lez-programs_2/programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin";
const ORACLE_REGISTER_BIN: &str = "../oracle_register/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin";

#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "e2e_lbc"), ignore = "requires a running Logos blockchain (enable feature e2e_lbc)")]
async fn test_e2e_oracle_register() -> anyhow::Result<()> {
    setup_test_tracing();

    let wallet_storage_folder = tempdir()?;
    let mut wallet_core = init_wallet(wallet_storage_folder.path()).await?;

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
    init_token_program_for_lon(&token_client, &lon_def_account_id, &lon_holder_account_id).await?;
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

async fn init_token_program_for_lon(client: &TokenClient<'_>, lon_def: &AccountId, lon_holder: &AccountId) -> anyhow::Result<()> {

    let accounts = NewFungibleDefinitionAccounts {
        definition_target_account: *lon_def,
        holding_target_account: *lon_holder,
    };
    let tx_hash = client.new_fungible_definition(accounts, "LON token".to_string(), 21_000, None).await
        .map_err(|e| anyhow!(e))?;

    info!("Waiting for transaction (new fungible def) to end...");
    wait_for_tx(client.wallet, &tx_hash, 3).await?;

    let account = client.wallet.get_account_public(*lon_def).await
        .map_err(|e| anyhow!("get account: {}", e))?;
    info!("Lon def: {:?}", TokenDefinition::try_from_slice(&account.data));

    info!("Lon holder: {:?}", get_token_holding(client, lon_holder).await);

    Ok(())
}

async fn get_token_holding(client: &TokenClient<'_>, account: &AccountId) -> anyhow::Result<TokenHolding> {
    let account = client.wallet.get_account_public(*account).await
        .map_err(|e| anyhow!("get account: {}", e))?;
    TokenHolding::try_from_slice(&account.data)
        .map_err(|e| anyhow!("deserialize token holding: {}", e))
}

async fn transfer_lon_token_to(client: &TokenClient<'_>, lon_def: &AccountId, lon_holder: &AccountId, to: &AccountId, amount: u128) -> anyhow::Result<()> {

    if get_token_holding(client, to).await.is_err() {
        let acc_0 = InitializeAccountAccounts {
            definition_account: *lon_def,
            account_to_initialize: *to,
        };

        match client.initialize_account(acc_0).await {
            Ok(tx_hash) => {
                info!("Waiting for transaction (init account) to end...");
                if let Err(e) = wait_for_tx(client.wallet, &tx_hash, 3).await {
                    warn!("Failed to initialize account: {}", e);
                }
            }
            Err(e) => warn!("Failed to initialize account: {}", e),
        }
    }

    let acc = TransferAccounts {
        sender: *lon_holder,
        recipient: *to,
    };
    let tx_hash = client.transfer(acc, amount).await
        .map_err(|e| anyhow!("transfer error: {}", e))?;

    info!("Waiting for transaction (transfer) to end...");
    wait_for_tx(client.wallet, &tx_hash, 3).await?;

    Ok(())
}

async fn init_register_program(client: &OracleRegisterClient<'_>, token_program: &Program, oracle_register_program: &Program, owner: AccountId) -> anyhow::Result<()> {
    if client.fetch_register::<RegisterState>().await.is_err() {
        info!("Oracle register not initialized, doing it now...");

        let register = compute_register_pda(&oracle_register_program.id());
        info!("register: {} - {:?}", register, register);
        let init_accounts = InitializeAccounts {
            register,
            owner,
        };

        let r = client.initialize(init_accounts, token_program.id()).await
            .map_err(|e| anyhow!(e))?;
        info!("init reg result: {:?}", r);

        info!("Waiting for transaction (initialize) to end...");
        wait_for_tx(client.wallet, &r, 3).await?;
        client.fetch_register::<RegisterState>().await
            .map_err(|e| anyhow!(e))?;

    } else {
        info!("Oracle register contract already initialized...");
    }

    Ok(())
}
