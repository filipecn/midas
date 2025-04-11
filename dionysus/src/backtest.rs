use crate::{
    finance::{Book, BookLine, DiError, Sample, Token},
    historical_data::HistoricalData,
    strategy::Chrysus,
    time::TimeWindow,
    ERROR, INFO,
};

use slog::slog_error;

struct BacktestData<'a> {
    samples: &'a [Sample],
    pub sample_index: usize,
}

impl<'a> BacktestData<'a> {
    pub fn new(samples: &'a [Sample]) -> BacktestData<'a> {
        BacktestData {
            samples,
            sample_index: 0,
        }
    }
}

impl<'a> HistoricalData for BacktestData<'a> {
    fn append(&mut self, _: &Token, _: &Sample) -> Result<(), DiError> {
        Err(DiError::NotImplemented)
    }

    fn fetch_last(&mut self, _: &Token, _: &TimeWindow) -> Result<&[Sample], DiError> {
        Err(DiError::NotImplemented)
    }

    fn get_last(&self, _: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        let first_index = self.sample_index.saturating_sub(duration.count as usize);
        Ok(&self.samples[first_index..self.sample_index])
    }
}

pub fn backtest(chrysus: &Chrysus, history: &[Sample]) {
    let mut c: Chrysus = chrysus.clone();
    c.capital = 1000.0;
    let mut backtest_data = BacktestData::new(history);
    for i in 1..history.len() {
        backtest_data.sample_index = i;
        let book = Book {
            token: chrysus.token.clone(),
            bids: vec![BookLine {
                price: history[i].close,
                quantity: 1.0,
            }],
            asks: vec![BookLine {
                price: history[i].close,
                quantity: 1.0,
            }],
        };
        let orders = c.decide(book, &backtest_data);
        for order in orders {
            c.realize(&order);
        }
    }
}
