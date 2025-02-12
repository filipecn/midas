use crate::finance::SymbolHistory;

use super::finance::Quote;
use super::oracles::{Oracle, Signal};

pub struct Method {
    strategies: Vec<Box<dyn Oracle>>,
}

impl Method {
    pub fn run(&self, quote: &Quote, history: &SymbolHistory) -> Result<Signal, &str> {
        let mut buy_signal_count = 0;
        let mut _sell_signal_count = 0;
        let mut _none_signal_count = 0;
        for strategy in &self.strategies {
            match strategy.run(quote, history) {
                Ok(Signal::Buy) => buy_signal_count += 1,
                Ok(Signal::Sell) => _sell_signal_count += 1,
                Ok(Signal::None) => _none_signal_count += 1,
                Err(_s) => continue,
            }
        }
        if buy_signal_count > 0 {
            return Ok(Signal::Buy);
        }
        Err("asd")
    }
}
