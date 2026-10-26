use std::str::FromStr;
// third-party
use rust_decimal::Decimal;
use anyhow::anyhow;
use pyth_sdk::Price;
use rust_decimal::prelude::FromPrimitive;
use chrono::DateTime;
use num_traits::ToPrimitive;
// internal
use crate::{RedstonePriceEvent, HermesPriceEvent, CoinbaseMessage, CoinbaseData, UniswapData, WrappedBinancePriceEvent};

pub const PRICE_OBSERVATION_DECIMAL: u32 = 6;
const PRICE_OBSERVATION_DECIMAL_I32: i32 = - (PRICE_OBSERVATION_DECIMAL as i32);

// ~= const TWO_POW_96: f64 = 2.0_f64.powi(96);
const TWO_POW_96: f64 = 79_228_162_514_264_337_593_543_950_336.0;
// ~= const PRICE_OBSERVATION_10_POWI_DECIMAL = 10_f64.powi(PRICE_OBSERVATION_DECIMAL_I32);
const PRICE_OBSERVATION_10_POWI_DECIMAL: f64 = 1_000_000.0;

pub const PROVIDER_BINANCE: &str = "binance";
pub const PROVIDER_REDSTONE: &str = "redstone";
pub const PROVIDER_PYTH: &str = "pyth";
pub const PROVIDER_COINBASE: &str = "coinbase";
pub const PROVIDER_UNISWAP: &str = "uniswap";

#[derive(Debug, Clone)]
pub struct PartialPriceObservation {
    pub feed_id: String,
    pub provider: String,
    pub provider_index: usize,
    pub feed_id_provider: String,
    pub price: i64,
    pub decimals: u32,
    pub timestamp: i64,
}

impl TryFrom<WrappedBinancePriceEvent> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from(wevent: WrappedBinancePriceEvent) -> Result<Self, Self::Error> {

        let mut decimal = Decimal::from_str(wevent.event.last_price.as_str())?;
        decimal.rescale(PRICE_OBSERVATION_DECIMAL);
        let scaled_i64 = decimal.mantissa() as i64;
        Ok(Self {
            feed_id: wevent.feed_id_normalized,
            provider: PROVIDER_BINANCE.to_string(),
            provider_index: wevent.provider_index,
            feed_id_provider: wevent.event.symbol,
            price: scaled_i64,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: wevent.event.event_time.try_into()?,
        })
    }
}

/*
impl TryFrom<(String, RedstonePriceEvent)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, event): (String, RedstonePriceEvent)) -> Result<Self, Self::Error> {

        let mut decimal = Decimal::from_f64(event.value)
            .ok_or(anyhow!("Failed to parse Redstone price value"))?;

        decimal.rescale(PRICE_OBSERVATION_DECIMAL);
        let scaled_i64 = decimal.mantissa() as i64;
        Ok(Self {
            feed_id: feed_id_normalized,
            provider: PROVIDER_REDSTONE.to_string(),
            feed_id_provider: event.symbol,
            price: scaled_i64,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: event.timestamp.try_into()?,
        })
    }
}
*/

/*
impl TryFrom<(String, HermesPriceEvent)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, event): (String, HermesPriceEvent)) -> Result<Self, Self::Error> {

        let parsed_update = event.parsed.ok_or(anyhow!("No updates in Hermes price event"))?;

        if parsed_update.is_empty() {
            return Err(anyhow!("Empty updates in Hermes price event"));
        }

        let price_latest = parsed_update[0].clone();

        let scaled_pyth_price = Price {
            price: price_latest.price.price.parse::<i64>()?,
            conf: price_latest.price.conf.parse::<u64>()?,
            expo: price_latest.price.expo,
            publish_time: price_latest.price.publish_time,
        }
            .scale_to_exponent(PRICE_OBSERVATION_DECIMAL_I32)
            .ok_or(anyhow!("Failed to scale_to_exponent for price"))?;

        Ok(Self {
            feed_id: feed_id_normalized,
            provider: PROVIDER_PYTH.to_string(),
            feed_id_provider: price_latest.id,
            price: scaled_pyth_price.price,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: price_latest.price.publish_time,
        })
    }
}
*/

impl TryFrom<CoinbaseData> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from(data: CoinbaseData) -> Result<Self, Self::Error> {

        // On overflows, == f64::INFINITY (or -f64::INFINITY)
        let scaled = data.price * 1_000_000.0;
        // 2^53 - maximum exact integer representation in f64
        const MAX_EXACT_F64_INT: f64 = 9_007_199_254_740_992.0;
        if scaled.abs() > MAX_EXACT_F64_INT {
            return Err(anyhow!("Price exceeds exact float representation bounds"));
        }

        let price_final = scaled.to_i64().ok_or(anyhow!("Price conversion failed (NaN or Infinity)"))?;

        Ok(Self {
            feed_id: data.feed_id_normalized,
            provider: PROVIDER_COINBASE.to_string(),
            provider_index: data.provider_index,
            feed_id_provider: data.feed_id,
            price: price_final,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: data.timestamp,
        })
    }
}

impl TryFrom<UniswapData> for PartialPriceObservation {
    type Error = anyhow::Error;

    fn try_from(data: UniswapData) -> Result<Self, Self::Error> {

        // Uniswap Math
        let price_ratio = data.price / TWO_POW_96;
        let raw_price = price_ratio * price_ratio;

        // Decimal Adjustment
        let decimal_diff = (data.token_1_decimals as i32) - (data.token_0_decimals as i32);
        let decimal_adjustment = 10_f64.powi(decimal_diff);
        let price_token0_in_token1 = raw_price / decimal_adjustment;

        // Invert if necessary
        let final_price_f64 = if data.is_base_token_0 {
            price_token0_in_token1
        } else {
            1.0 / price_token0_in_token1
        };

        // Scale to target decimals and round half-up
        let price_final = (final_price_f64 * PRICE_OBSERVATION_10_POWI_DECIMAL).round() as i64;

        Ok(Self {
            feed_id: data.feed_id_normalized,
            provider: PROVIDER_UNISWAP.to_string(),
            provider_index: data.provider_index,
            feed_id_provider: data.feed_id,
            price: price_final,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: data.timestamp,
        })
    }
}