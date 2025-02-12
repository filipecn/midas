use super::time::*;
use std::{cmp, collections::HashMap};

/// A financial quote is the price at which an asset was last traded, or the
/// price at which it can be bought or sold. It can also refer to the bid
/// or ask price of a security.
#[derive(Debug, Clone)]
pub struct Quote {
    /// The highest price a buyer is willing to pay.
    pub bid: f64,
    /// The lowest price a seller is willing to accept.
    pub ask: f64,
    pub biddate: Date,
    pub askdate: Date,
}

/// Summary of price movements of an asset over a time period.
#[derive(Debug, Clone)]
pub struct Sample {
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

#[derive(Debug, Clone)]
pub struct SymbolHistory<'a> {
    rolling_mean_data: HashMap<u32, f64>,
    std_dev_data: HashMap<u32, f64>,
    data: &'a [Sample],
}

impl Quote {
    pub fn from_sample(sample: &Sample) -> Quote {
        Quote {
            bid: (sample.open + sample.close) / 2.0,
            ask: (sample.open + sample.close) / 2.0,
            biddate: Date::from_timestamp(sample.timestamp as i64),
            askdate: Date::from_timestamp(sample.timestamp as i64),
        }
    }
}

impl<'a> SymbolHistory<'a> {
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
