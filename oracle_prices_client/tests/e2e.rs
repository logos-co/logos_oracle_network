use anyhow::anyhow;
use clock_core::CLOCK_01_PROGRAM_ACCOUNT_ID;
use e2e_utils::{deploy_contract, init_wallet, last_block_id, setup_test_tracing, wait_for_block, wait_for_tx};
use oracle_prices_client::{compute_feed_price_pda, compute_oracle_prices_account_pda,
                           InitializeAccounts, InitializeFeedAccounts, OraclePricesClient,
                           SubmitAttestationAccounts, FinalizeAttestationAccounts};
use oracle_prices_core::{AttestationStatus, FeedState, DISPUTE_WINDOW_BLOCKS};
use rand::Rng;
use serial_test::serial;
use tempfile::tempdir;
use tracing::info;

const ORACLE_PRICES_BIN: &str = "../oracle_prices/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin";

/// Price decimals (same as oracle_node common::PRICE_OBSERVATION_DECIMAL)
const PRICE_DECIMALS: u32 = 6;

#[tokio::test]
#[serial]
#[cfg_attr(not(feature = "e2e_lbc"), ignore = "requires a running Logos blockchain (enable feature e2e_lbc)")]
async fn test_e2e_oracle_prices() -> anyhow::Result<()> {
    setup_test_tracing();

    let wallet_storage_folder = tempdir()?;
    let wallet_core = init_wallet(wallet_storage_folder.path()).await?;

    info!("Reading oracle_prices bin: {}", ORACLE_PRICES_BIN);
    let oracle_prices_program = deploy_contract(&wallet_core, ORACLE_PRICES_BIN, "oracle prices").await?;
    info!("oracle prices program id: {:?}", oracle_prices_program.id());

    let client = OraclePricesClient::new(&wallet_core, oracle_prices_program.id());
    let oracle_prices_account = compute_oracle_prices_account_pda(&oracle_prices_program.id());

    // Initialize (only once, contract state is kept across test runs)
    // Note: OraclePricesState is { feeds: Vec<[u8; 32]> } (same borsh layout as Vec<[u8; 32]>)
    if client.fetch_oracle_prices_account::<Vec<[u8; 32]>>().await.is_err() {
        info!("Oracle prices not initialized, doing it now...");
        let tx_hash = client.initialize(InitializeAccounts { oracle_prices_account }).await
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
    let decimals = PRICE_DECIMALS;
    let block_before = last_block_id(&wallet_core).await?;
    let tx_hash = client.submit_attestation(
        SubmitAttestationAccounts { feed_price, clock: CLOCK_01_PROGRAM_ACCOUNT_ID },
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
    let block_after = last_block_id(&wallet_core).await?;

    let feed_state: FeedState = client.fetch_feed_price(&feed_id).await
        .map_err(|e| anyhow!(e))?;
    info!("feed state: {:?}", feed_state);
    assert!(feed_state.last_final.is_none());
    let attested_price = feed_state.pending.ok_or(anyhow!("No pending attestation"))?;
    assert_eq!(attested_price.feed_id, feed_id);
    assert_eq!(attested_price.price, price);
    assert_eq!(attested_price.decimals, decimals);
    assert_eq!(attested_price.round, round);
    assert_eq!(attested_price.confidence, confidence);
    assert_eq!(attested_price.status, AttestationStatus::Pending);
    assert_eq!(attested_price.set_hash, set_hash);
    assert_eq!(attested_price.proposer_id, proposer_oracle_id);
    // written_at is the block id read by the contract from the clock account
    assert!(
        (block_before..=block_after).contains(&attested_price.written_at),
        "written_at {} not in [{}, {}]", attested_price.written_at, block_before, block_after
    );

    // Finalize once the dispute window is over
    // Note: an early finalize is rejected by the contract (tested in oracle_prices_core unit tests)
    info!("Waiting for dispute window to end...");
    wait_for_block(&wallet_core, attested_price.written_at + DISPUTE_WINDOW_BLOCKS).await?;
    let tx_hash = client.finalize_attestation(
        FinalizeAttestationAccounts { feed_price, clock: CLOCK_01_PROGRAM_ACCOUNT_ID },
        feed_id,
        round,
    ).await
        .map_err(|e| anyhow!(e))?;
    info!("Waiting for transaction (finalize attestation) to end...");
    wait_for_tx(&wallet_core, &tx_hash, 3).await?;

    let feed_state: FeedState = client.fetch_feed_price(&feed_id).await
        .map_err(|e| anyhow!(e))?;
    info!("feed state: {:?}", feed_state);
    assert!(feed_state.pending.is_none());
    let final_price = feed_state.last_final.ok_or(anyhow!("No final attestation"))?;
    assert_eq!(final_price.status, AttestationStatus::Final);
    assert_eq!(final_price.round, round);
    assert_eq!(final_price.price, price);
    assert_eq!(final_price.written_at, attested_price.written_at);

    Ok(())
}
