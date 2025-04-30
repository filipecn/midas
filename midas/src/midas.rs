use slog::slog_error;
use std::collections::HashMap;
use std::fs::File;

use dionysus::{
    backtest::{backtest, Backtest},
    binance::BinanceMarket,
    counselor::Counselor,
    finance::{Book, MarketEvent, MarketTick, Order, Sample, Token},
    historical_data::HistoricalData,
    strategy::{Chrysus, Strategy},
    time::TimeWindow,
    wallet::{BinanceWallet, DigitalWallet},
    ERROR,
};

pub enum MidasEvent {
    BookUpdate(Token),
    KLineUpdate(usize),
}

pub struct Midas {
    pub wallet: BinanceWallet,
    pub market: BinanceMarket,
    pub hesperides: Vec<Chrysus>,
    pub ticks: HashMap<Token, MarketTick>,
    pub books: HashMap<Token, Book>,
    balance: HashMap<Token, f64>,
}

impl Midas {
    pub fn new(keys_file: &str, use_test_api: bool) -> Midas {
        Self {
            wallet: BinanceWallet::new(&keys_file, use_test_api),
            market: BinanceMarket::new(use_test_api),
            hesperides: Vec::new(),
            ticks: HashMap::new(),
            books: HashMap::new(),
            balance: HashMap::new(),
        }
    }

    pub fn init(&mut self, state_file: &String) {
        self.load_state(state_file);
        self.market.day_ticker_all_service("USDT");
        self.balance = HashMap::new();
        match self.wallet.get_balance() {
            Ok(balance) => {
                for (token, asset) in balance {
                    self.balance.insert(token.clone(), asset.free);
                }
            }
            Err(e) => ERROR!("{:?}", e),
        };
    }

    pub fn save_state(&self, filename: &String) {
        let file = File::create(filename.as_str()).unwrap();
        if let Err(e) = serde_json::to_writer_pretty(file, &self.hesperides) {
            ERROR!("{:?}", e);
        }
    }

    pub fn load_state(&mut self, filename: &String) {
        let data = std::fs::read_to_string(filename).expect("Unable to read file");
        self.hesperides = serde_json::from_str(&data).expect("Unable to parse");
        for i in 0..self.hesperides.len() {
            self.init_token(i);
        }
    }

    fn init_token(&mut self, index: usize) {
        let chrysus = &self.hesperides[index];
        if chrysus.token.is_pair() {
            match self
                .market
                .fetch_last(&chrysus.token, &chrysus.strategy.duration)
            {
                Ok(_samples) => {
                    // compute strategy performance
                    //backtest(&chrysus, samples);
                }
                Err(e) => {
                    let t = chrysus.token.clone();
                    ERROR!("ERROR {:?} {:?}.", e, t);
                    return;
                }
            }
            self.market
                .kline_service(&chrysus.token, &chrysus.strategy.duration.resolution);
            self.market.order_book_service(&chrysus.token);
        }
    }

    pub fn add_token(&mut self, token: &Token) -> Option<usize> {
        let index = self.hesperides.len();
        self.hesperides.push(Chrysus::new(token));
        let mut strategy = Strategy::default();
        strategy
            .counselors
            .push(Counselor::MeanReversion((20, 2.0.into())));
        strategy.duration.count = 200;
        self.set_strategy(index, &strategy);
        Some(index)
    }

    pub fn run_backtest(&mut self, index: usize, period: &TimeWindow) -> Backtest {
        match self.market.get_last(&self.hesperides[index].token, &period) {
            Ok(samples) => {
                return backtest(&self.hesperides[index], samples);
            }
            Err(e) => ERROR!("{:?}", e),
        }
        Backtest::default()
    }

    pub fn get_history(&self, index: usize) -> Option<&[Sample]> {
        let t = &self.hesperides[index];
        match self.market.get_last(&t.token, &t.strategy.duration) {
            Ok(samples) => return Some(samples),
            Err(e) => ERROR!("{:?}", e),
        }
        None
    }

    pub fn set_strategy(&mut self, index: usize, strategy: &Strategy) {
        self.hesperides[index].strategy = strategy.clone();
        self.init_token(index);
    }

    pub fn get(&self, index: usize) -> Option<&Chrysus> {
        Some(&self.hesperides[index])
    }

    pub fn get_balance(&self) -> HashMap<Token, f64> {
        self.balance.clone()
    }

    fn update_ticks(&mut self, ticks: Vec<MarketTick>) {
        for tick in ticks {
            if let Some(t) = self.ticks.get_mut(&tick.token) {
                *t = tick;
            } else {
                self.ticks.insert(tick.token.clone(), tick.clone());
            }
        }
    }

    pub fn get_book(&self, token: &Token) -> Option<Book> {
        if let Some(book) = self.books.get(&token) {
            Some(book.clone())
        } else {
            None
        }
    }

    pub fn touch(&mut self) -> Vec<MidasEvent> {
        let mut events: Vec<MidasEvent> = Vec::new();
        for event in self.market.get_events() {
            match event {
                MarketEvent::KLine((token, sample)) => {
                    for (index, t) in self.hesperides.iter().enumerate() {
                        if t.token == token {
                            if let Err(e) = self.market.append(&t.token, &sample) {
                                ERROR!("{:?}", e);
                            } else {
                                if sample.resolution == t.strategy.duration.resolution {
                                    events.push(MidasEvent::KLineUpdate(index));
                                }
                            }
                        }
                    }
                }
                MarketEvent::Ticks(ticks) => self.update_ticks(ticks),
                MarketEvent::OrderBook(book) => {
                    let token = book.token.clone();
                    self.books.insert(token.clone(), book);
                    events.push(MidasEvent::BookUpdate(token));
                    //for t in &self.hesperides {
                    //    if t.token == token {
                    //let orders = t.decide(book, &self.market);
                    //_submit(orders);
                    //        break;
                    //    }
                    // }
                }
            };
        }
        events
    }

    fn _submit(&mut self, _orders: Vec<Order>) {}
}
