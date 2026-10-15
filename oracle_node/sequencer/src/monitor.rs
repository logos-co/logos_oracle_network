use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use dashmap::DashMap;
use tracing::{info, error, warn};
// external deps
use common::PartialPriceObservation;

const MAX_VEC_SIZE_PER_FEED_ID: usize = 1024;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct FeedKey {
    pub feed_id: String,
    pub provider: String,
}

pub struct PriceMonitor {
    map: Arc<DashMap<FeedKey, VecDeque<PartialPriceObservation>>>,
}

impl PriceMonitor {

    pub fn new(map: Arc<DashMap<FeedKey, VecDeque<PartialPriceObservation>>>) -> Self {
        Self { map }
    }

    pub async fn run(&self,
                     price_update_queue: &mut UnboundedReceiver<PartialPriceObservation>,
    ) -> anyhow::Result<()> {

        info!("Starting price monitor...");

        loop {
            let price_update = price_update_queue.recv().await;
            let Some(price_update) = price_update else { error!("Channel closed"); break; };
            let feed_key = FeedKey {
                feed_id: price_update.feed_id.clone(),
                provider: price_update.provider.clone(),
            };
            
            self
                .map
                .entry(feed_key)
                .and_modify(|v| {
                    if v.len() > MAX_VEC_SIZE_PER_FEED_ID {
                        warn!("Price feed {} entries too large...", price_update.feed_id);
                        // Remove oldest entry
                        v.pop_front();
                    }
                    v.push_back(price_update.clone());
                    // warn!("Price feed {}, entries count: {}", price_update.feed_id, v.len());
                })
                .or_insert(VecDeque::from([price_update]));
        }

        Ok(())
    }
}