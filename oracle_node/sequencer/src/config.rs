use std::collections::HashMap;
use serde::Deserialize;
use alloy::primitives::Address;
// use toml::from_slice;

#[derive(Deserialize, Debug)]
pub struct UniswapConfig {
    pub contracts: ContractsConfig,
    pub tokens: HashMap<String, Address>,
    pub pool: PoolConfig,
}

/// Uniswap v4 pool parameters (part of the PoolKey used to compute the pool id)
#[derive(Deserialize, Debug)]
pub struct PoolConfig {
    /// Pool fee in hundredths of a bip (e.g. 500 = 0.05%)
    pub fee: u32,
    pub tick_spacing: i32,
}

#[derive(Deserialize, Debug)]
pub struct ContractsConfig {
    pub state_view: Address,
}

#[derive(Deserialize, Debug)]
pub struct TokensConfig {
    pub weth: Address,
    pub usdc: Address,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tracing::info;
    use crate::setup_tracing;
    use super::*;

    #[test]
    fn test_reading_uniswap_config() -> anyhow::Result<()> {

        setup_tracing();

        let cfg_ = "../resources/uniswap_config.toml";
        let cfg_content = fs::read(cfg_)?;
        let cfg: UniswapConfig = toml::from_slice(cfg_content.as_slice())?;
        info!("cfg: {:?}", cfg);

        Ok(())
    }
}