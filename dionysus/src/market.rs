use crate::binance::BinanceMarket;
use crate::finance::DiError;

#[derive(Default)]
pub struct PairPrice {
    pub symbol: String,
    pub currency: String,
    pub price: f64,
}

#[derive(Default)]
pub struct PairPriceStats {
    pub symbol: String,
    pub currency: String,
    pub price_change_percent: f64,
    pub last_price: f64,
    pub volume: f64,
}

pub trait Market {
    fn get_all_prices(&self, currency: &str) -> Result<Vec<PairPrice>, DiError>;
    fn get_all_24h_price_stats(&self, currency: &str) -> Result<Vec<PairPriceStats>, DiError>;
}

impl Market for BinanceMarket {
    fn get_all_prices(&self, currency: &str) -> Result<Vec<PairPrice>, DiError> {
        match self.market.get_all_prices() {
            Ok(answer) => {
                let binance::model::Prices::AllPrices(prices) = answer;
                return Ok(prices
                    .iter()
                    .filter(|price| price.symbol.contains(currency))
                    .map(|price| PairPrice {
                        symbol: String::from(
                            &price.symbol[..(price.symbol.len() - currency.len())],
                        ),
                        currency: String::from(&price.symbol[currency.len()..]),
                        price: price.price,
                    })
                    .collect());
            }
            Err(e) => Err(DiError::Message(format!("{:?}", e))),
        }
    }

    fn get_all_24h_price_stats(&self, currency: &str) -> Result<Vec<PairPriceStats>, DiError> {
        match self.market.get_all_24h_price_stats() {
            Ok(stats) => {
                return Ok(stats
                    .iter()
                    .filter(|stat| stat.symbol.contains(currency))
                    .map(|stat| PairPriceStats {
                        symbol: String::from(&stat.symbol[..(stat.symbol.len() - currency.len())]),
                        currency: String::from(&stat.symbol[currency.len()..]),
                        last_price: stat.last_price,
                        volume: stat.volume,
                        price_change_percent: stat
                            .price_change_percent
                            .parse::<f64>()
                            .unwrap_or(0.0),
                    })
                    .collect());
            }
            Err(e) => Err(DiError::Message(format!("{:?}", e))),
        }
    }
}
