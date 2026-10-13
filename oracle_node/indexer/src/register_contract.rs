use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{anyhow, Context};
use tracing::{debug, info};
// third-party: lez
use spel_framework::prelude::ProgramId;
use wallet::WalletCore;
// internal
use oracle_register_client::{OracleRegisterClient, RegisterState as OracleRegisterState, RegisterState};
use tokio::sync::RwLock;
use common::RegisterContractInfo;
use crate::mtree::TreeSnapshot;
/*
pub async fn fetch_registered(rc_info: &RegisterContractInfo) -> anyhow::Result<Vec<[u8; 32]>> {

    let wallet_core = WalletCore::from_env().context("Getting wallet accounts from env")?;
    let oracle_register_program_id = ProgramId::from(rc_info.oracle_register_program_id);
    let client = OracleRegisterClient::new(&wallet_core, oracle_register_program_id);

    // debug!("Fetching oracle register state...");
    // debug!("client program id: {:?}", client.program_id);
    let register_state = client
        .fetch_register::<OracleRegisterState>()
        .await
        .map_err(|err| anyhow!("{}", err))?;

    // debug!("register store fetched...");

    Ok(register_state.registered.to_vec())
}
*/

pub async fn fetch_oracle_register_state(rc_info: &RegisterContractInfo) -> anyhow::Result<RegisterState> {

    let wallet_core = WalletCore::from_env().context("Getting wallet accounts from env")?;
    let oracle_register_program_id = ProgramId::from(rc_info.oracle_register_program_id);
    let client = OracleRegisterClient::new(&wallet_core, oracle_register_program_id);

    // debug!("Fetching oracle register state...");
    // debug!("client program id: {:?}", client.program_id);
    let register_state = client
        .fetch_register::<OracleRegisterState>()
        .await
        .map_err(|err| anyhow!("{}", err))?;

    // debug!("register store fetched...");

    Ok(register_state)
}

pub async fn merkle_tree_roots_poll(rc_info: &RegisterContractInfo, merkle_tree_roots: Arc<RwLock<VecDeque<TreeSnapshot>>>, max_capacity: usize, poll_interval: Duration) -> anyhow::Result<()> {

    info!("Starting merkle-tree poll...");
    loop {
        let register_state = fetch_oracle_register_state(rc_info).await?;

        let last_root = {
            let lock = merkle_tree_roots.read().await;
            lock.back().map(|snapshot| snapshot.clone())
        };

        if last_root.is_none() || last_root.unwrap().root != register_state.mtree.current_root {

            let mut write_lock = merkle_tree_roots.write().await;

            if write_lock.len() == max_capacity {
                // SPEC/FIXME: need to specify how many roots to keep (or when to reset roots)
                write_lock.pop_front();
                debug!("Removing Merkle tree root from root cache...");
            }

            write_lock.push_back(TreeSnapshot {
                root: register_state.mtree.current_root.clone(),
                total_leaves: register_state.mtree.len(),
            });
            info!("Added new Merkle tree root");
        }

        tokio::time::sleep(poll_interval).await;
    }
    Ok(())
}

