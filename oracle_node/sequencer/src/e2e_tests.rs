use std::path::PathBuf;
use std::str::FromStr;
use lee::{AccountId, PublicTransaction};
use lee::program::Program;
use lee::public_transaction::{Message, WitnessSet};
use lez_common::transaction::LeeTransaction;
use oracle_register_client::{OracleRegisterClient, RegisterState, InitializeAccounts};
use oracle_prices_client::{compute_feed_price_pda, compute_oracle_prices_account_pda,
                           InitializeAccounts as OraclePricesInitializeAccounts,
                           InitializeFeedAccounts, OraclePricesClient, SubmitAttestationAccounts};
use oracle_prices_core::{AttestationStatus, AttestedPrice};
use sequencer_service_rpc::RpcClient;
use super::*;
use serial_test::serial;
use spel_framework::prelude::BorshDeserialize;
use crate::test_utils::{init_wallet, send_deploy_tx, setup_test_tracing, wait_for_tx};
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
const ORACLE_PRICES_BIN: &str = "../../oracle_prices/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin";

#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "e2e_lbc"), ignore = "requires a running Logos blockchain (enable feature e2e_lbc)")]
async fn test_e2e_oracle_prices() -> anyhow::Result<()> {
    setup_test_tracing();

    let wallet_storage_folder = tempdir()?;
    let wallet_storage_file_path = wallet_storage_folder.path().join("storage.json");
    std::fs::File::create(wallet_storage_file_path.as_path())?;
    let wallet_core = init_wallet(wallet_storage_file_path)?;

    info!("Reading oracle_prices bin: {}", ORACLE_PRICES_BIN);
    let oracle_prices_program = deploy_contract(&wallet_core, ORACLE_PRICES_BIN, "oracle prices").await?;
    info!("oracle prices program id: {:?}", oracle_prices_program.id());

    let client = OraclePricesClient::new(&wallet_core, oracle_prices_program.id());
    let oracle_prices_account = compute_oracle_prices_account_pda(&oracle_prices_program.id());

    // Initialize (only once, contract state is kept across test runs)
    // Note: OraclePricesState is { feeds: Vec<[u8; 32]> } (same borsh layout as Vec<[u8; 32]>)
    if client.fetch_oracle_prices_account::<Vec<[u8; 32]>>().await.is_err() {
        info!("Oracle prices not initialized, doing it now...");
        let tx_hash = client.initialize(OraclePricesInitializeAccounts { oracle_prices_account }).await
            .map_err(|e| anyhow!(e))?;
        info!("Waiting for transaction (initialize) to end...");
        wait_for_tx(&wallet_core, &tx_hash, 3).await?;
    } else {
        info!("Oracle prices contract already initialized...");
    }

    // Random feed_id: the contract keeps every initialized feed (test can be run multiple times)
    let feed_id: [u8; 32] = rand::thread_rng().r#gen();
    info!("feed_id: {}", hex::encode(feed_id));
    let feed_price = compute_feed_price_pda(&oracle_prices_program.id(), &feed_id);

    let tx_hash = client.initialize_feed(InitializeFeedAccounts { oracle_prices_account, feed_price }, feed_id).await
        .map_err(|e| anyhow!(e))?;
    info!("Waiting for transaction (initialize feed) to end...");
    wait_for_tx(&wallet_core, &tx_hash, 3).await?;

    let round: u64 = 42;
    let price: u64 = 3_000_000_000;
    let confidence: u64 = 1_500;
    let set_hash: [u8; 32] = [3; 32];
    let proposer_oracle_id: [u8; 32] = [1; 32];
    let decimals = common::PRICE_OBSERVATION_DECIMAL;
    let tx_hash = client.submit_attestation(
        SubmitAttestationAccounts { feed_price },
        feed_id,
        round,
        price,
        confidence,
        set_hash,
        proposer_oracle_id,
        [2; 32], // proposer_channel_id
        0, // proposer_leaf_index
        vec![], // proposer_membership_proof
        decimals,
    ).await
        .map_err(|e| anyhow!(e))?;
    info!("Waiting for transaction (submit attestation) to end...");
    wait_for_tx(&wallet_core, &tx_hash, 3).await?;

    let attested_price: AttestedPrice = client.fetch_feed_price(&feed_id).await
        .map_err(|e| anyhow!(e))?;
    info!("attested price: {:?}", attested_price);
    assert_eq!(attested_price.feed_id, feed_id);
    assert_eq!(attested_price.price, price);
    assert_eq!(attested_price.decimals, decimals);
    assert_eq!(attested_price.round, round);
    assert_eq!(attested_price.confidence, confidence);
    assert_eq!(attested_price.status, AttestationStatus::Pending);
    assert_eq!(attested_price.set_hash, set_hash);
    assert_eq!(attested_price.proposer_id, proposer_oracle_id);
    assert_eq!(attested_price.written_at, 0); // TODO: block id once the contract reads the clock account

    Ok(())
}

#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "e2e_lbc"), ignore = "requires a running Logos blockchain (enable feature e2e_lbc)")]
async fn test_e2e_oracle_register() -> anyhow::Result<()> {
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
    let tx_hash = client.new_fungible_definition(accounts, "LON token".to_string(), 21_000, None).await
        .map_err(|e| anyhow!(e))?;

    info!("Waiting for transaction (new fungible def) to end...");
    wait_for_tx(client.wallet, &tx_hash, 3).await?;

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
        wait_for_tx(client.wallet, &r, 3).await?;
        client.fetch_register::<RegisterState>().await
            .map_err(|e| anyhow!(e))?;

    } else {
        info!("Oracle register contract already initialized...");
    }

    Ok(())
}
