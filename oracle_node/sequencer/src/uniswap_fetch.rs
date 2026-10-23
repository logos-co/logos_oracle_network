use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use url::Url;
use common::{PartialPriceObservation, UniswapData};
use alloy::{
    primitives::{Address, U160},
    providers::ProviderBuilder,
    sol
};
use alloy::primitives::aliases::{I24, U24};
use alloy::primitives::{keccak256, B256};
use alloy::sol_types::SolValue;
use anyhow::anyhow;
use chrono::Utc;
use tracing::{debug, error, info};
use crate::config::UniswapConfig;

sol! {
    #[sol(rpc)]
    contract StateView {
        function getSlot0(bytes32 poolId) external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint24 protocolFee,
            uint24 lpFee
        );
    }

    // to encode and hash the PoolId
    struct PoolKey {
        address currency0;
        address currency1;
        uint24 fee;
        int24 tickSpacing;
        address hooks;
    }

    #[sol(rpc)]
    contract IERC20 {
        function decimals() external view returns (uint8);
    }
}

pub async fn fetch_price(
    url: Url,
    provider_index: i32,
    price_feed: &str,
    price_feed_normalized: &str,
    price_update_queue: UnboundedSender<PartialPriceObservation>,
    cfg: UniswapConfig,
) -> anyhow::Result<()> {

    let provider = ProviderBuilder::new().connect_http(url);
    let state_view = StateView::new(cfg.contracts.state_view, provider.clone());

    let (token_0, token_1) = price_feed.split_once("/")
        .ok_or(anyhow!("Price feed should be TOKEN_0/TOKEN_1"))?;
    debug!("tokens: {} / {}", token_0, token_1);

    let token_0_addr = cfg.tokens.get(token_0).ok_or(anyhow!("Token 0 not found in Uniswap config"))?;
    let token_1_addr = cfg.tokens.get(token_1).ok_or(anyhow!("Token 1 not found in Uniswap config"))?;

    let (currency0, currency1) = if token_0_addr < token_1_addr {
        (token_0_addr, token_1_addr)
    } else {
        (token_1_addr, token_0_addr)
    };

    info!("currency 0: {}", currency0.to_string());
    info!("currency 1: {}", currency1.to_string());

    // Fetch the decimals onchain
    let erc20_0 = IERC20::new(*currency0, provider.clone());
    let erc20_1 = IERC20::new(*currency1, provider.clone());

    let decimals_0 = erc20_0.decimals().call().await?;
    let decimals_1 = erc20_1.decimals().call().await?;

    info!("Token 0 Decimals: {}", decimals_0);
    info!("Token 1 Decimals: {}", decimals_1);

    let pool_key = PoolKey {
        currency0: *currency0,
        currency1: *currency1,
        fee: U24::from(500), // 0.05%
        tickSpacing: I24::try_from(10)?,
        hooks: Address::ZERO, // Assuming no hooks for this standard pool
    };

    let pool_id: B256 = keccak256(pool_key.abi_encode());

    loop {
        match state_view.getSlot0(pool_id).call().await {
            Ok(slot0) => {
                // `getSlot0` is a view function (does not return Ethereum block timestamp)
                // so we assign a local Unix ts the moment the call resolve
                let timestamp_call = Utc::now().timestamp();
                let sqrt_price_x96: U160 = slot0.sqrtPriceX96;
                info!("sqrt_price_x96: {}", sqrt_price_x96);
                let sqrt_price_f64 = sqrt_price_x96.to_string().parse::<f64>()?;

                let data = UniswapData {
                    price: sqrt_price_f64,
                    timestamp: timestamp_call,
                    feed_id: price_feed.to_string(),
                    token_0_decimals: decimals_0,
                    token_1_decimals: decimals_1,
                    is_base_token_0: (currency0 == token_0_addr),
                    feed_id_normalized: price_feed_normalized.to_string(),
                    provider_index,
                };

                match PartialPriceObservation::try_from(data) {
                    Ok(price_obs) => {
                        if let Err(e) = price_update_queue.send(price_obs) {
                            error!("Error while sending price update event: {}", e);
                        }
                    },
                    Err(e) => {
                        error!("Error while converting event to price update event: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch Uniswap V4 price: {}", e);
            },
        }
        tokio::time::sleep(Duration::from_secs(12)).await;
    }

    Ok(())

}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use serial_test::serial;
    use crate::test_utils::setup_test_tracing;

    #[tokio::test]
    #[serial]
    async fn test_price_fetch() -> anyhow::Result<()> {

        const TEST_TIMEOUT: Duration = Duration::from_secs(45);
        const EXPECTED_PRICE_EVENT_COUNT: i32 = 3;
        setup_test_tracing();
        let _ = rustls::crypto::ring::default_provider().install_default();

        let _ = dotenvy::from_filename_override(".env.test")
            .context("Failed to load .env.test file")?;
        let test_rpc_url = Url::parse(std::env::var("ETHEREUM_RPC_URL")
            .context("ETHEREUM_RPC_URL must be set in .env.test")?.as_str())?;

        let price_feed = "weth/usdc";
        let price_feed_normalized = "ETH/USDC";

        // uniswap config
        let uniswap_cfg = {
            let cfg_content = std::fs::read("../resources/uniswap_config.toml")?;
            let cfg: UniswapConfig = toml::from_slice(cfg_content.as_slice())?;
            cfg
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let _handle = tokio::spawn(async move {
            let res = fetch_price(test_rpc_url, 0, price_feed, price_feed_normalized, tx, uniswap_cfg).await;
            debug!("fetch_price result: {:?}", res);
            res
        });

        let f = tokio::time::timeout(TEST_TIMEOUT, async move {
            let mut counter = 0;
            loop {
                let price = rx.recv().await;
                if price.is_none() {
                    break;
                }
                info!("Received: {:?}", price);
                counter += 1;
                if counter >= EXPECTED_PRICE_EVENT_COUNT {
                    break;
                }
            }
            counter
        });

        let received = f.await.context("While waiting/receiving price observation")?;
        debug!("received: {:?}", received);
        assert_eq!(received, EXPECTED_PRICE_EVENT_COUNT);

        Ok(())
    }

}