use std::time::Duration;
use anyhow::{anyhow, Context};
use chrono::DateTime;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, timeout};
use tracing::{error, info, warn};
use url::Url;
use common::{CoinbaseData, CoinbaseMessage, PartialPriceObservation};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
// Coinbase sends a heartbeat every second (heartbeats channel), so no message during this
// period means the connection is dead (e.g. half-open TCP connection)
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Outcome of a websocket session (when not an error)
enum SessionEnd {
    /// Connection was lost or closed by the server - should reconnect
    Disconnected { received_data: bool },
    /// The price update queue receiver has been dropped - no need to continue
    QueueClosed,
}


pub async fn fetch_price(
    url: Url,
    provider_index: usize,
    price_feed: &str,
    price_feed_normalized: &str,
    price_update_queue: UnboundedSender<PartialPriceObservation>) -> anyhow::Result<()> {

    let mut retry_delay = INITIAL_RETRY_DELAY;

    loop {
        match run_session(&url, provider_index, price_feed, price_feed_normalized, &price_update_queue).await {
            Ok(SessionEnd::QueueClosed) => {
                return Err(anyhow!("Coinbase - price update queue closed, stopping price fetch"));
            }
            Ok(SessionEnd::Disconnected { received_data }) => {
                warn!("Coinbase connection lost for price feed: {}", price_feed);
                if received_data {
                    retry_delay = INITIAL_RETRY_DELAY; // Reset backoff, connection was working
                }
            }
            Err(e) => {
                error!("Coinbase connection error for price feed {}: {:#}", price_feed, e);
            }
        }

        info!("Reconnecting to Coinbase in {} seconds...", retry_delay.as_secs());
        sleep(retry_delay).await;
        retry_delay = std::cmp::min(retry_delay * 2, MAX_RETRY_DELAY);
    }
}

/// Connect, subscribe & process messages until the connection is lost
async fn run_session(
    url: &Url,
    provider_index: usize,
    price_feed: &str,
    price_feed_normalized: &str,
    price_update_queue: &UnboundedSender<PartialPriceObservation>) -> anyhow::Result<SessionEnd> {

    info!("Connecting to Coinbase WebSocket stream ({}) for price feed: {}...", url, price_feed);
    let (ws_stream, _) = timeout(READ_TIMEOUT, connect_async(url.as_str()))
        .await
        .context("Timeout while connecting to Coinbase WS")?
        .context("While connecting to Coinbase WS")?;
    let (mut write, mut read) = ws_stream.split();

    // Subscribe to the ticker channel + heartbeats channel (keep the connection open when
    // updates are sparse and allow to detect a dead connection)
    for channel in ["ticker", "heartbeats"] {
        let subscribe_msg = json!({
            "type": "subscribe",
            "product_ids": [price_feed],
            "channel": channel
        });
        timeout(READ_TIMEOUT, write.send(Message::text(subscribe_msg.to_string())))
            .await
            .context("Timeout while subscribing to Coinbase")?
            .with_context(|| format!("While subscribing to Coinbase {} channel", channel))?;
    }
    info!("Subscribed to Coinbase {} ticker", price_feed);

    let mut received_data = false;

    loop {
        let msg = match timeout(READ_TIMEOUT, read.next()).await {
            Ok(Some(Ok(msg))) => msg,
            Ok(Some(Err(e))) => {
                error!("Coinbase stream error: {}", e);
                return Ok(SessionEnd::Disconnected { received_data });
            }
            Ok(None) => {
                info!("Coinbase stream ended");
                return Ok(SessionEnd::Disconnected { received_data });
            }
            Err(_) => {
                warn!("No message from Coinbase in {} seconds", READ_TIMEOUT.as_secs());
                return Ok(SessionEnd::Disconnected { received_data });
            }
        };

        let text = match msg {
            Message::Text(text) => text,
            Message::Close(frame) => {
                info!("Coinbase closed the connection: {:?}", frame);
                return Ok(SessionEnd::Disconnected { received_data });
            }
            // Ping / Pong are handled by tungstenite
            _ => continue,
        };

        received_data = true;

        // Note: heartbeats & subscriptions messages do not parse as CoinbaseMessage
        let Ok(parsed_msg) = serde_json::from_str::<CoinbaseMessage>(&text) else {
            continue;
        };

        if parsed_msg.channel != "ticker" {
            continue;
        }

        let timestamp = match DateTime::parse_from_rfc3339(&parsed_msg.timestamp) {
            Ok(dt) => dt.timestamp_millis(),
            Err(e) => {
                error!("Failed to parse Coinbase timestamp: {}", e);
                continue; // Skip this message if the timestamp is completely broken
            }
        };

        for event in parsed_msg.events {
            // Coinbase sends "snapshot" or "update" event types
            for ticker in event.tickers {

                info!("Coinbase - got ticker: {:?}", ticker);

                let data = CoinbaseData {
                    price: ticker.price,
                    timestamp,
                    feed_id: ticker.product_id,
                    feed_id_normalized: price_feed_normalized.to_string(),
                    provider_index,
                };

                match PartialPriceObservation::try_from(data) {
                    Ok(price_obs) => {
                        if let Err(e) = price_update_queue.send(price_obs) {
                            error!("Error while sending price update event: {}", e);
                            return Ok(SessionEnd::QueueClosed);
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use crate::test_utils::setup_test_tracing;

    #[tokio::test]
    #[serial]
    async fn test_price_fetch() {

        const TEST_TIMEOUT: Duration = Duration::from_secs(20);
        const EXPECTED_PRICE_EVENT_COUNT: i32 = 3;
        setup_test_tracing();
        let _ = rustls::crypto::ring::default_provider().install_default();

        let url = Url::parse("wss://advanced-trade-ws.coinbase.com").unwrap();
        let price_feed = "ETH-USDC";
        let price_feed_normalized = "ETH/USDC";
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let _handle = tokio::spawn(async move {
            fetch_price(url, 0, price_feed, price_feed_normalized, tx).await
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

        let received = f.await.unwrap();
        // debug!("res: {:?}", res);
        assert_eq!(received, EXPECTED_PRICE_EVENT_COUNT);
    }
}

