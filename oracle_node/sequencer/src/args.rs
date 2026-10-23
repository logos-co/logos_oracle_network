use std::path::PathBuf;
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(about = "LON zone sequencer")]
pub struct SequencerArgs {
    /// Logos blockchain node HTTP endpoint
    #[arg(
        long,
        default_value = "http://localhost:8080",
        env = "SEQUENCER_NODE_ENDPOINT"
    )]
    pub node_url: String,

    /// Logos blockchain node REST API endpoint
    #[arg(
        long,
        default_value = "http://localhost:18080",
        env = "SEQUENCER_REST_ENDPOINT"
    )]
    pub node_rest_url: String,

    /// Path to the signing key file (created if it doesn't exist)
    #[arg(
        long,
        default_value = "./data",
        env = "SEQUENCER_DATA_FOLDER"
    )]
    pub data_folder: PathBuf,

    /// Path to the signing key file (created if it doesn't exist)
    #[arg(
        long,
        default_value = "sequencer.key",
        env = "SEQUENCER_SIGNING_KEY_PATH"
    )]
    pub key_path: PathBuf,

    // /// Path to the signing key file (created if it doesn't exist)
    // #[arg(
    //     long,
    //     default_value = "oracle.key",
    //     env = "SEQUENCER_ORACLE_SIGNING_KEY_PATH"
    // )]
    // pub oracle_key_path: PathBuf,

    /// Basic auth username for node endpoint
    #[arg(long, env = "SEQUENCER_NODE_AUTH_USERNAME")]
    pub node_auth_username: Option<String>,

    /// Basic auth password for node endpoint
    #[arg(long, env = "SEQUENCER_NODE_AUTH_PASSWORD")]
    pub node_auth_password: Option<String>,

    /// Path to the checkpoint file for crash recovery
    #[arg(
        long,
        default_value = "sequencer.checkpoint",
        env = "CHECKPOINT_PATH"
    )]
    pub checkpoint_path: PathBuf,

    /// Path to the channel ID file
    #[arg(long, default_value = "channel.txt", env = "CHANNEL_PATH")]
    pub channel_path: PathBuf,

    #[arg(long, help = "Pyth network bearer - optional")]
    pub(crate) pyth_bearer: Option<String>,

    #[arg(long, default_value = "resources/provider_config.json")]
    pub provider_config: PathBuf,
    
    #[arg(long, default_value = "resources/register_contract_config.json")]
    pub register_contract_config: PathBuf,

    #[arg(long, default_value = "resources/uniswap_config.toml")]
    pub uniswap_config: Option<PathBuf>,

    /// The ETHEREUM RPC URL (Alchemy?) for Uniswap data (can be passed via CLI or ENV)
    #[arg(long, env = "ETHEREUM_RPC_URL", hide_env_values = true)]
    pub ethereum_rpc_url: Option<String>,

    /// A list of providers (for price fetching)
    #[arg(
        short,
        long,
        value_enum,
        action = clap::ArgAction::Append,
        required = true // Need at least one provider
    )]
    pub provider: Vec<Provider>,

    #[arg(
        long,
        default_value = "ETH/USDC",
        help = "Price feed - normalized between all price providers (check provider_config.json)"
    )]
    pub price_feed_normalized: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ValueEnum)]
pub enum Provider {
    Binance,
    Coinbase,
    Uniswap,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Binance => "binance",
            Provider::Coinbase => "coinbase",
            Provider::Uniswap => "uniswap",
        }
    }
}