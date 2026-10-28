use std::time::Duration;
// third-party
use tracing::debug;
// internal
use common::PartialPriceObservation;

pub struct PreAggregator {
    pub max_deviation_bips: u64, // Basis points (e.g., 300 = 3.00% max deviation)
    pub max_staleness: Duration,
}

impl PreAggregator {
    pub fn new(max_deviation_bips: u64, max_staleness: Duration) -> Self {
        Self {
            max_deviation_bips,
            max_staleness,
        }
    }

    pub fn compute_local_median(
        &self,
        observations: &[PartialPriceObservation],
        current_timestamp_millis: i64,
    ) -> Option<i64> {

        let max_staleness_millis = i64::try_from(self.max_staleness.as_millis()).unwrap_or(i64::MAX);

        // Sanity Check: Filter non-positive and stale prices
        let mut valid: Vec<i64> = observations
            .iter()
            .filter(|obs| {
                obs.price > 0
                    && current_timestamp_millis.saturating_sub(obs.timestamp_millis) <= max_staleness_millis
            })
            .map(|obs| obs.price)
            .collect();

        if valid.is_empty() {
            debug!("No valid observations found (too old observations or negative prices)");
            return None;
        }

        // Sort to find provisional median
        valid.sort_unstable();
        let prov_median = get_spec_median(valid.as_slice());

        // Gross Outlier Filtering (> max_deviation_bips from provisional median)
        let filtered: Vec<i64> = valid
            .into_iter()
            .filter(|&price| {
                let diff = if price > prov_median {
                    price - prov_median
                } else {
                    prov_median - price
                };

                // Calculate percentage difference in Basis Points: (diff * 10000) / prov_median
                let deviation_bips = (diff as u128 * 10_000) / (prov_median as u128);

                deviation_bips <= (self.max_deviation_bips as u128)
            })
            .collect();

        if filtered.is_empty() {
            debug!("No valid observations (after gross outlier filtering)");
            return None;
        }

        // Re compute final median according to spec
        let mut final_sorted = filtered;
        final_sorted.sort_unstable();

        Some(get_spec_median(&final_sorted))
    }
}

/// Helper function, from specification:
/// `If the number of values is even, it takes the lower of the two middle values,
/// so the result is always one of the observed prices and never an average.`
fn get_spec_median(sorted_prices: &[i64]) -> i64 {
    let len = sorted_prices.len();
    if len % 2 == 1 {
        // Odd number of items: exact middle
        sorted_prices[len / 2]
    } else {
        // Even number of items: lower of the two middle values
        sorted_prices[(len / 2) - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gross_outlier_filtering_and_even_median() {
        let agg = PreAggregator::new(300, Duration::from_secs(30)); // 3% max deviation, 30s max staleness
        let now = Duration::from_secs(1_700_000_000).as_millis() as i64;

        let obs = vec![
            PartialPriceObservation {
                feed_id: "ETH/USDC".to_string(),
                provider: "binance".to_string(),
                provider_index: 0,
                feed_id_provider: "ethusdc".to_string(),
                price: 2400_000_000,
                decimals: 6,
                timestamp_millis: now,
            },
            PartialPriceObservation {
                feed_id: "ETH/USDC".to_string(),
                provider: "coinbase".to_string(),
                provider_index: 0,
                feed_id_provider: "ETH-USDC".to_string(),
                price: 2402_000_000,
                decimals: 6,
                timestamp_millis: now,
            },
            PartialPriceObservation {
                feed_id: "ETH/USDC".to_string(),
                provider: "uniswap".to_string(), // manipulated
                provider_index: 0,
                feed_id_provider: "ETH-USDC".to_string(),
                price: 1800_000_000, // -25% outlier
                decimals: 6,
                timestamp_millis: now,
            },
        ];

        let median = agg.compute_local_median(&obs, now).unwrap();

        // Uniswap dropped -> Remaining: [2400_000_000, 2402_000_000]
        // Length = 2 (Even) -> Lower middle = 2400_000_000
        assert_eq!(median, 2400_000_000);
    }

    #[test]
    fn test_staleness_in_milliseconds() {
        let agg = PreAggregator::new(300, Duration::from_secs(30)); // 30s max staleness
        let now = Duration::from_secs(1_700_000_000).as_millis() as i64;
        let age = |d: Duration| now - d.as_millis() as i64;

        let make = |price: i64, timestamp_millis: i64| PartialPriceObservation {
            feed_id: "ETH/USDC".to_string(),
            provider: "binance".to_string(),
            provider_index: 0,
            feed_id_provider: "ethusdc".to_string(),
            price,
            decimals: 6,
            timestamp_millis,
        };

        // 31s old -> rejected
        assert_eq!(agg.compute_local_median(&[make(2400_000_000, age(Duration::from_secs(31)))], now), None);
        // 29s old -> accepted
        assert_eq!(agg.compute_local_median(&[make(2400_000_000, age(Duration::from_secs(29)))], now), Some(2400_000_000));
    }
}
