use crate::cache::Cache;
use binance;

pub struct BinanceMarket {
    pub market: binance::market::Market,
    pub cache: Cache,
}

impl Default for BinanceMarket {
    fn default() -> Self {
        Self {
            market: binance::api::Binance::new(None, None),
            cache: Cache::default(),
        }
    }
}
