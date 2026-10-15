use std::time::Duration;
use anyhow::Context;
use chrono::DateTime;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};
use url::Url;
use common::{CoinbaseData, CoinbaseMessage, PartialPriceObservation};
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
        "product_ids": [price_feed],
        "channel": "ticker"
    });

    // debug!("subscribe_msg: {:?}", &subscribe_msg);

    let msg = Message::text(subscribe_msg.to_string());
    write.send(msg).await?;
    info!("Subscribed to Coinbase ETH-USD ticker");

    while let Some(msg) = read.next().await {
        // debug!("msg: {:?}", msg);
        if let Ok(Message::Text(text)) = msg {
            // debug!("text: {:?}", text);
            if let Ok(parsed_msg) = serde_json::from_str::<CoinbaseMessage>(&text) {
                // debug!("ticker: {:?}", parsed_msg);

                if parsed_msg.channel == "ticker" {

                    let timestamp = match DateTime::parse_from_rfc3339(&parsed_msg.timestamp) {
                        Ok(dt) => dt.timestamp_millis(),
                        Err(e) => {
                            println!("Failed to parse timestamp: {}", e);
                            continue; // Skip this message if the timestamp is completely broken
                        }
                    };

                    for event in parsed_msg.events {
                        // Coinbase sends "snapshot" or "update" event types
                        for ticker in event.tickers {

                            info!("Coinbase - got ticker: {:?}", ticker);
                            if let Ok(price_f64) = ticker.price.parse::<f64>() {
                                let price = (price_f64 * 1_000_000.0) as i64;

                                let data = CoinbaseData {
                                    price,
                                    timestamp,
                                    feed_id: ticker.product_id
                                };

                                match PartialPriceObservation::try_from(
                                    (price_feed_normalized.to_string(), data)) {
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
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::EnvFilter;
    use common::time_info_poll;
    use super::*;

    #[tokio::test]
    async fn test_price_fetch() {

        setup_test_tracing();

        let _ = rustls::crypto::ring::default_provider().install_default();

        let url = Url::parse("wss://advanced-trade-ws.coinbase.com").unwrap();
        let price_feed = "ETH-USDC";
        let price_feed_normalized = "ETH/USDC";
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let _handle = tokio::spawn(async move {
            fetch_price(url, price_feed, price_feed_normalized, tx).await
        });

        let expected_count = 3;
        let f = tokio::time::timeout(Duration::from_secs(20), async move {
            let mut counter = 0;
            loop {
                let price = rx.recv().await;
                if price.is_none() {
                    break;
                }
                info!("Received: {:?}", price);
                counter += 1;
                if counter >= expected_count {
                    break;
                }
            }
            counter
        });

        let res = f.await.unwrap();
        // println!("res: {:?}", res);
        assert_eq!(res, expected_count);
    }

    pub fn setup_test_tracing() {
        // Note: tracing-test crate is another dev dependency and only listen to main thread
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            // ignore error if another // unit test already init. it
            .try_init();
    }
}

