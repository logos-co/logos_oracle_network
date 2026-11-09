use std::process::ExitCode;
// third-party
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing::{
    info,
    debug,
    error,
    level_filters::LevelFilter
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
// internal
use sequencer::args::SequencerArgs;
use sequencer::{run, SequencerConfig};

#[tokio::main]
async fn main() -> ExitCode {
    let args = SequencerArgs::parse();

    setup_tracing();
    debug!("args: {:?}", &args);

    let cfg = match SequencerConfig::try_from(&args) {
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

    if let Err(err) = run(cfg, shutdown).await {
        error!("[Main] error: {:#?}", err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
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
