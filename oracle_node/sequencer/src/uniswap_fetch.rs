use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, Duration};
use url::Url;
use common::PartialPriceObservation;
use alloy::primitives::{Address, U160};
use alloy::providers::ProviderBuilder;
use alloy::sol;

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

    todo!()


}
