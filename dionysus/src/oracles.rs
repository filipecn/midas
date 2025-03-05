use slog::slog_info;
use slog_scope;

use crate::{
    finance::{DiError, Quote, Sample},
    INFO,
};
use std::{cmp, collections::HashMap};

#[derive(Debug, Clone)]
pub struct SymbolTA<'a> {
    rolling_mean_data: HashMap<u32, f64>,
    std_dev_data: HashMap<u32, f64>,
    data: &'a [Sample],
}

impl<'a> SymbolTA<'a> {
    pub fn new(samples: &'a [Sample]) -> Self {
        Self {
            rolling_mean_data: HashMap::new(),
            std_dev_data: HashMap::new(),
            data: samples,
        }
    }
    pub fn size(&self) -> usize {
        self.data.len()
    }
    pub fn rolling_mean(&mut self, n: u32) -> f64 {
        match self.rolling_mean_data.get(&n) {
            Some(value) => value * 1.0,
            None => {
                let first_index = cmp::max(0, self.data.len() as i32 - n as i32) as usize;
                let mean = self.data[first_index..]
                    .iter()
                    .map(|sample| sample.close)
                    .sum::<f64>()
                    / self.data.len() as f64;
                self.rolling_mean_data.insert(n, mean);
                mean
            }
        }
    }
    pub fn std_dev(&mut self, n: u32) -> f64 {
        match self.std_dev_data.get(&n) {
            Some(value) => value * 1.0,
            None => {
                let first_index = cmp::max(0, self.data.len() as i32 - n as i32) as usize;
                let mean = self.rolling_mean(n);
                let variance = (self.data[first_index..]
                    .iter()
                    .map(|sample| (sample.close - mean).powi(2))
                    .sum::<f64>()
                    / self.data.len() as f64)
                    .sqrt();
                self.std_dev_data.insert(n, variance);
                variance
            }
        }
    }
}

/// A signal represents the sentiment of an strategy.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Signal {
    Buy,
    Sell,
    None,
}

/// Bot strategies emit signals for each given new quote.
pub trait Oracle {
    fn required_samples(&self) -> u32 {
        0
    }
    fn run(&self, quote: &Quote, history: &[Sample]) -> Result<Signal, DiError>;
}

pub struct TraceOracle;
pub struct MeanReversionOracle {
    pub n: u32,
}

impl Oracle for TraceOracle {
    fn run(&self, quote: &Quote, _history: &[Sample]) -> Result<Signal, DiError> {
        println!("{:?}", quote);
        Ok(Signal::None)
    }
}

impl Oracle for MeanReversionOracle {
    fn required_samples(&self) -> u32 {
        self.n
    }
    fn run(&self, quote: &Quote, history: &[Sample]) -> Result<Signal, DiError> {
        if history.len() < self.n as usize {
            return Err(DiError::Error);
        }
        let mut ta = SymbolTA::new(history);

        let std_dev = ta.std_dev(history.len() as u32);
        let mean = ta.rolling_mean(history.len() as u32);

        INFO!("test {:?}", quote);

        slog_info!(
            slog_scope::logger(),
            "MeanReversionStrategy handling quote: {:?}",
            quote
        );
        slog_info!(
            slog_scope::logger(),
            "Px: {}; Mean: {}; Std Dev: {}",
            quote.ask,
            mean,
            std_dev
        );
        slog_info!(
            slog_scope::logger(),
            "quote.ask: {}; (mean - 2.0 * std_dev): {}",
            quote.ask,
            mean - 2.0 * std_dev
        );

        let buy = quote.ask < mean - 2.0 * std_dev;
        let sell = quote.ask > mean + 2.0 * std_dev;

        if buy {
            slog_info!(
                slog_scope::logger(),
                "***Buy signal for {}***",
                quote.symbol
            );
            Ok(Signal::Buy)
        } else if sell {
            slog_info!(
                slog_scope::logger(),
                "***Sell signal for {}***",
                quote.symbol
            );
            Ok(Signal::Sell)
        } else {
            slog_info!(slog_scope::logger(), "No signal for {}", quote.symbol);
            Ok(Signal::None)
        }
    }
}
