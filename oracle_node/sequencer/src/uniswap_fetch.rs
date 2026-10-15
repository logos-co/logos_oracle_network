use tokio::sync::mpsc::UnboundedSender;
use url::Url;
use common::PartialPriceObservation;
use alloy::primitives::{Address, U160};
use alloy::providers::ProviderBuilder;
use alloy::sol;
use clap::builder::TypedValueParser;

sol! {
    #[sol(rpc)]
    contract UniswapV3Pool {
        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );
    }
}

pub async fn fetch_price(
    url: Url,
    price_feed: &str,
    price_feed_normalized: &str,
    price_update_queue: UnboundedSender<PartialPriceObservation>) -> anyhow::Result<()> {

    /*
    let provider = ProviderBuilder::new().on_http(url);

    // The Ethereum mainnet address for the ETH/USDC 0.05% pool
    let pool_address: Address = "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"
        .parse()?;

    let pool = UniswapV3Pool::new(pool_address, provider);
    */
    /*
    loop {
        // 3. Call slot0() on the contract
        match pool.slot0().call().await {
            Ok(slot0) => {
                // alloy strictly types this as U160 based on our sol! macro
                let sqrt_price_x96: U160 = slot0.sqrtPriceX96;

                // --- Uniswap V3 Math ---
                // The safest way to convert large alloy Uint types to f64
                // is to parse them via their string representation.
                let sqrt_price_f64: f64 = sqrt_price_x96.to_string().parse().unwrap();

                // Divide by 2^96
                let two_pow_96 = 2.0_f64.powi(96);
                let price_ratio = sqrt_price_f64 / two_pow_96;

                // Square it
                let raw_price = price_ratio * price_ratio;

                // Adjust for Token Decimals (USDC = 6, WETH = 18)
                let decimal_adjustment = 10_f64.powi(18 - 6);
                let actual_price = raw_price * decimal_adjustment;

                // Invert because the pool is USDC/WETH (Token0/Token1)
                let eth_usdc_price = 1.0 / actual_price;

                println!("Uniswap V3 (Alloy) ETH/USDC Price: {}", eth_usdc_price);

                let price_u64 = (eth_usdc_price * 1_000_000.0) as u64;

                // monitor.add_price(FeedKey { feed_id: "ETH/USDC".into(), provider: "uniswap_v3".into() }, price_u64);
            }
            Err(e) => {
                println!("Failed to fetch Uniswap price: {}", e);
            }
        }

        // Poll every 12 seconds
        sleep(Duration::from_secs(12)).await;
    }
    */

    Ok(())
}
