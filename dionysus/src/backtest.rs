use crate::{
    finance::{Book, BookLine, DiError, Order, Sample, Token},
    historical_data::HistoricalData,
    strategy::Chrysus,
    time::{Date, TimeWindow},
    utils::compute_change_pct,
};

#[derive(Default, Clone)]
pub struct Backtest {
    pub initial_capital: f64,
    pub orders: Vec<Order>,
    pub period: TimeWindow,
    pub currency_balance: f64,
    pub symbol_balance: f64,
}

impl Backtest {
    pub fn compute_profit(&self, tick: f64) -> f64 {
        compute_change_pct(
            self.initial_capital,
            self.currency_balance + tick * self.symbol_balance,
        )
    }
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
    backtest_result.initial_capital = capital;
    backtest_result.period = TimeWindow {
        resolution: history[0].resolution,
        count: history.len() as i64,
    };
    let mut backtest_data = BacktestData::new(history);
    let offset = chrysus.strategy.required_history_size();
    for i in offset..history.len() {
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
    backtest_result.currency_balance = c.capital;
    backtest_result.symbol_balance = c.balance;
    backtest_result
}
