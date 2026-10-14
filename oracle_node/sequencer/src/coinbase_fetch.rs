use std::time::Duration;
use anyhow::Context;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info};
use url::Url;
use common::{CoinbaseTickerMsg, PartialPriceObservation};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

pub async fn fetch_price(
    url: Url,
    price_feed: &str,
    price_feed_normalized: &str,
    price_update_queue: UnboundedSender<PartialPriceObservation>) -> anyhow::Result<()> {

    // TODO: reconn loop (or reco is already handled?)

    let price_feed_normalized = price_feed_normalized.to_string();
    let mut retry_delay = Duration::from_secs(1);

    info!("Connecting to Coinbase WebSocket stream ({}) for price feed: {}...", url, price_feed);
    let (ws_stream, _) = connect_async(url.as_str())
        .await
        .context("While connecting to Coinbase WS")?;
    let (mut write, mut read) = ws_stream.split();

    // Subscribe to the ticker channel for ETH-USD
    let subscribe_msg = json!({
        "type": "subscribe",
        "product_ids": ["ETH-USD"],
        "channel": "ticker"
    });

    let msg = Message::text(subscribe_msg.to_string());
    write.send(msg).await?;
    info!("Subscribed to Coinbase ETH-USD ticker");

    while let Some(msg) = read.next().await {
        if let Ok(Message::Text(text)) = msg {
            if let Ok(ticker) = serde_json::from_str::<CoinbaseTickerMsg>(&text) {
                // Ignore initial subscription confirmation messages
                if ticker.msg_type == "ticker" {
                    match PartialPriceObservation::try_from(
                        (price_feed_normalized.to_string(), ticker)) {
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
            }
        }
    }

    Ok(())
}