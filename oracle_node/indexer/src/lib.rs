pub mod args;
mod indexer;
mod register_contract;
mod prices_contract;
mod mtree;

pub use crate::indexer::lon;

use std::collections::HashSet;
use std::time::Duration;
use anyhow::Context;
// third-party
use spel_framework::serde_json;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
// internal
// use common::time_poller;
use common::{time_info_poll, RegisterContractInfo, PricesContractInfo};
use crate::args::IndexerArgs;
use crate::indexer::{channel_discover, Indexer};
use crate::lon::AttestedPrice;
use crate::mtree::RootsCache;
use crate::register_contract::merkle_tree_roots_poll;

const MAX_ROOTS_TO_KEEP: usize = 10;

/// Everything needed to run an indexer (built from CLI args or directly, e.g. in tests)
pub struct IndexerConfig {
    /// Logos blockchain node REST API endpoint
    pub node_rest_url: String,
    pub node_auth_username: Option<String>,
    pub node_auth_password: Option<String>,
    pub register_contract: RegisterContractInfo,
    pub prices_contract: PricesContractInfo,
}

/// Build a config from CLI args (reading the files the args point to)
impl TryFrom<&IndexerArgs> for IndexerConfig {
    type Error = anyhow::Error;

    fn try_from(args: &IndexerArgs) -> anyhow::Result<Self> {

        // oracle_register contract info
        let register_contract = {
            let cfg_content = std::fs::read(args.register_contract_config.as_path())
                .context(format!("Reading {}", args.register_contract_config.as_path().display()))?;
            let cfg: RegisterContractInfo = toml::from_slice(cfg_content.as_slice())?;
            debug!("oracle register contract cfg: {:?}", cfg);
            cfg
        };

        // oracle_prices contract info
        let prices_contract = {
            let file = std::fs::File::open(args.prices_contract_config.as_path())
                .context(format!("Reading {}", args.prices_contract_config.as_path().display()))?;
            let reader = std::io::BufReader::new(file);
            let cfg = serde_json::from_reader::<_, PricesContractInfo>(reader)?;
            debug!("oracle prices contract cfg: {:?}", cfg);
            cfg
        };

        Ok(Self {
            node_rest_url: args.node_rest_url.clone(),
            node_auth_username: args.node_auth_username.clone(),
            node_auth_password: args.node_auth_password.clone(),
            register_contract,
            prices_contract,
        })
    }
}

/// Run the indexer until a task fails or `shutdown` is cancelled
///
/// Every attested price successfully published to the oracle_prices contract is also sent to
/// `attested_tx` (if provided).
///
/// Note: does not install any tracing subscriber (this is up to the caller)
pub async fn run(
    cfg: IndexerConfig,
    shutdown: CancellationToken,
    attested_tx: Option<mpsc::UnboundedSender<AttestedPrice>>,
) -> anyhow::Result<()> {

    info!("Oracle Indexer starting up...");
    info!("  Logos blockchain Node: {}", cfg.node_rest_url);

    let oracle_register_cfg = cfg.register_contract;
    let oracle_prices_cfg = cfg.prices_contract;

    // merlke tree
    let roots_cache = {
        let mut r = RootsCache::default();
        r.max_capacity = MAX_ROOTS_TO_KEEP;
        r
    };
    // end merkle tree

    /*
    let (watch_channel_ids_tx, watch_channel_ids_rx) = watch::channel(HashSet::new());
    let _channel_discover_handle = tokio::spawn(async move {
        channel_discover(Duration::from_millis(100), watch_channel_ids_tx, oracle_register_cfg).await
    });
    let (watch_time_info_tx, watch_time_info_rx) = watch::channel(None);
    let node_rest_url = args.node_rest_url.clone();
    let _watch_time_info_handle = tokio::spawn(async move  {
        time_info_poll(node_rest_url, Duration::from_millis(100), watch_time_info_tx).await
    });

    // Wait for some initial channel_ids & time_info
    let mut chi = watch_channel_ids_rx.clone();
    let mut ti = watch_time_info_rx.clone();
    let (_first, _second) = tokio::join!(
        chi.wait_for(|state| !state.is_empty()),
        ti.wait_for(|state| state.is_some()),
    );

    info!("Channel ids & time_info working, starting indexer now...");

    // TODO: pass current_slot so indexer can wait 1 / 2 slots to start its work?
    let indexer = match Indexer::new(
        &args.node_rest_url,
        args.node_auth_username,
        args.node_auth_password,
        watch_time_info_rx.clone(),
        watch_channel_ids_rx.clone(),
        oracle_prices_cfg,
    ) {
        Ok(i) => i,
        Err(e) => {
            error!("Indexer initialization failed: {e}");
            std::process::exit(1);
        }
    };

    info!("Launching indexer...");
    // indexer.run().await;
    tokio::time::sleep(Duration::from_secs(30)).await;
    info!("Exiting indexer main...");
    */

    let mut set = JoinSet::new();
    let (watch_time_info_tx, watch_time_info_rx) = watch::channel(None);
    let node_rest_url = cfg.node_rest_url.clone();
    set.spawn(async move {
        time_info_poll(node_rest_url, Duration::from_millis(100), watch_time_info_tx).await
    });
    let (watch_channel_ids_tx, watch_channel_ids_rx) = watch::channel(HashSet::new());
    let oracle_register_cfg_2 = oracle_register_cfg.clone();
    set.spawn(async move {
        channel_discover(Duration::from_millis(100), watch_channel_ids_tx, &oracle_register_cfg).await
    });

    let merkle_tree_roots = roots_cache.history.clone();
    set.spawn(async move {
        merkle_tree_roots_poll(&oracle_register_cfg_2, merkle_tree_roots, roots_cache.max_capacity, Duration::from_millis(100)).await
    });

    let indexer = Indexer::new(
        &cfg.node_rest_url,
        cfg.node_auth_username,
        cfg.node_auth_password,
        watch_time_info_rx.clone(),
        watch_channel_ids_rx.clone(),
        oracle_prices_cfg,
        roots_cache.history.clone(),
        attested_tx,
    )
        .context("Indexer initialization failed")?;

    set.spawn(async move {
        indexer.run().await
    });

    let result = loop {
        tokio::select! {
            () = shutdown.cancelled() => {
                info!("Shutdown requested");
                break Ok(());
            }
            res = set.join_next() => match res {
                Some(Ok(Err(e))) => {
                    error!("Task error: {:#?}", e);
                    error!("Aborting...");
                    break Err(e);
                },
                Some(Ok(_)) => {
                    info!("Tasks finished");
                    break Ok(());
                },
                Some(Err(e)) => {
                    error!("Join error: {:#?}", e);
                    break Err(e.into());
                },
                None => break Ok(()),
            }
        }
    };

    set.shutdown().await;
    info!("Indexer stopped.");

    result
}
