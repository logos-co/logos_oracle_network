use std::process::ExitCode;
// third-party
use clap::Parser as _;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
// internal
use indexer::args::IndexerArgs;
use indexer::{run, IndexerConfig};

#[tokio::main]
async fn main() -> ExitCode {
    let args = IndexerArgs::parse();

    setup_tracing();

    let cfg = match IndexerConfig::try_from(&args) {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("[Main] config error: {:#?}", err);
            return ExitCode::FAILURE;
        }
    };

    let shutdown = CancellationToken::new();
    let shutdown_ = shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("Ctrl-C received, shutting down...");
            shutdown_.cancel();
        }
    });

    if let Err(err) = run(cfg, shutdown, None).await {
        error!("[Main] error: {:#?}", err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn setup_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();
}
