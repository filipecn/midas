use crate::cache::Cache;
use crate::finance::Quote;
use crate::time::{Date, TimeWindow};
use rand::thread_rng;
use rand_distr::{Distribution, Normal};

pub struct BrownianMotionMarket {
    // - Drift (mu): 0.2
    pub mu: f64,
    // - Volatility (sigma): 0.4
    pub sigma: f64,
    // - Time horizon: 1.0
    pub time_horizon: f64,
    pub cache: Cache,
}

impl Default for BrownianMotionMarket {
    fn default() -> Self {
        BrownianMotionMarket {
            mu: 0.2,
            sigma: 0.4,
            time_horizon: 1.0,
            cache: Cache::default(),
        }
    }
}

pub fn generate_brownian_data(mu: f64, sigma: f64, duration: &TimeWindow) -> Vec<Quote> {
    // generate data in minute resolution, then sample
    let total_minutes = duration.num_minutes() as usize;
    let time_increment = TimeWindow::minutes(1);

    let mut quotes = Vec::with_capacity(total_minutes);
    let normal = Normal::new(0.0, 1.0).unwrap();

    // from https://github.com/nzengi/stochastic-gbm/blob/master/src/gbm.rs
    let dt = 1.0 / total_minutes as f64;
    let drift = (mu - 0.5 * sigma.powi(2)) * dt;
    let vol_sqrt_dt = sigma * dt.sqrt();
    let mut old_price = 500.0;
    let mut rng = thread_rng();
    let mut quote_date = Date::now();
    for _ in 0..total_minutes {
        let z = normal.sample(&mut rng);
        let price = old_price * (drift + vol_sqrt_dt * z).exp();
        old_price = price;
        let quote = Quote {
            symbol: "brownian".to_string(),
            biddate: quote_date.clone(),
            askdate: quote_date.clone(),
            bid: price,
            ask: price,
        };
        quotes.push(quote);
        quote_date += time_increment;
    }

    quotes
}
