use std::str::FromStr;
// third-party
use rust_decimal::Decimal;
use anyhow::anyhow;
use pyth_sdk::Price;
use rust_decimal::prelude::FromPrimitive;
use chrono::DateTime;
// internal
use crate::{BinancePriceEvent, RedstonePriceEvent, HermesPriceEvent, CoinbaseMessage, CoinbaseData};

pub const PRICE_OBSERVATION_DECIMAL: u32 = 6;
const PRICE_OBSERVATION_DECIMAL_PYTH: i32 = - (PRICE_OBSERVATION_DECIMAL as i32);

pub const PROVIDER_BINANCE: &str = "binance";
pub const PROVIDER_REDSTONE: &str = "redstone";
pub const PROVIDER_PYTH: &str = "pyth";
pub const PROVIDER_COINBASE: &str = "coinbase";

#[derive(Debug, Clone)]
pub struct PartialPriceObservation {
    pub feed_id: String,
    pub provider: String,
    pub feed_id_provider: String,
    pub price: i64,
    pub decimals: u32,
    pub timestamp: i64,
}

impl TryFrom<(String, BinancePriceEvent)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, event): (String, BinancePriceEvent)) -> Result<Self, Self::Error> {

        let mut decimal = Decimal::from_str(event.last_price.as_str())?;
        decimal.rescale(PRICE_OBSERVATION_DECIMAL);
        let scaled_i64 = decimal.mantissa() as i64;
        Ok(Self {
            feed_id: feed_id_normalized,
            provider: PROVIDER_BINANCE.to_string(),
            feed_id_provider: event.symbol,
            price: scaled_i64,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: event.event_time.try_into()?,
        })
    }
}

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
            .scale_to_exponent(PRICE_OBSERVATION_DECIMAL_PYTH)
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

impl TryFrom<(String, CoinbaseData)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, data): (String, CoinbaseData)) -> Result<Self, Self::Error> {

        Ok(Self {
            feed_id: feed_id_normalized,
            provider: PROVIDER_COINBASE.to_string(),
            feed_id_provider: data.feed_id,
            price: data.price,
            decimals: PRICE_OBSERVATION_DECIMAL,
            timestamp: data.timestamp,
        })
    }
}