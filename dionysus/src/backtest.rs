use crate::{
    finance::{Book, BookLine, DiError, Order, Sample, Token},
    historical_data::HistoricalData,
    strategy::Chrysus,
    time::{Date, TimeWindow},
    utils::compute_change_pct,
    ERROR, INFO,
};

use slog::slog_error;

#[derive(Default, Clone)]
pub struct Backtest {
    pub orders: Vec<Order>,
    pub pct: f64,
    pub period: TimeWindow,
}

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

pub fn backtest(chrysus: &Chrysus, history: &[Sample]) -> Backtest {
    let capital = 1000.0;
    let mut c: Chrysus = chrysus.clone();
    c.capital = capital;
    let mut backtest_result = Backtest::default();
    backtest_result.period = TimeWindow {
        resolution: history[0].resolution,
        count: history.len() as i64,
    };
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
        let mut orders = c.decide(book, &backtest_data);
        for order in &mut orders {
            order.date = Date::from_timestamp(history[i].timestamp);
            c.realize(&order);
            backtest_result.orders.push(order.clone());
        }
    }
    backtest_result.pct = compute_change_pct(capital, c.capital);
    backtest_result
}
