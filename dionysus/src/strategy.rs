use std::collections::HashMap;

use crate::data::HistoricalData;
use crate::finance::{DiError, Quote};
use crate::oracles::{Oracle, Signal};
use crate::time::TimeUnit;

type Wallet = HashMap<String, f64>;

pub struct Strategy {
    pub oracles: Vec<Box<dyn Oracle>>,

    time_resolution: TimeUnit,
    _wallet: Wallet,
}

impl Strategy {
    pub fn new() -> Self {
        Self {
            time_resolution: TimeUnit::Day(1),
            oracles: Vec::new(),
            _wallet: Wallet::new(),
        }
    }

    pub fn run(&self, quote: &Quote, history: &mut impl HistoricalData) -> Result<Signal, DiError> {
        let quote_time = quote.biddate.clone();
        let mut buy_signal_count = 0;
        let mut _sell_signal_count = 0;
        let mut _none_signal_count = 0;
        for strategy in &self.oracles {
            let required_samples = strategy.required_samples();
            //let period_data;
            //match history.get_previous_samples(
            //    &quote_time,
            //    &quote.symbol[..],
            //    &self.time_resolution,
            //    required_samples as u64,
            //) {
            //    Ok(data) => period_data = data,
            //    Err(e) => return Err(e),
            //}
            //match strategy.run(&quote, &period_data[..]) {
            //    Ok(Signal::Buy) => buy_signal_count += 1,
            //    Ok(Signal::Sell) => _sell_signal_count += 1,
            //    Ok(Signal::None) => _none_signal_count += 1,
            //    Err(_s) => continue,
            //}
        }
        if buy_signal_count > 0 {
            return Ok(Signal::Buy);
        }
        Ok(Signal::None)
    }
}
