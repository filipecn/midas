use slog::slog_error;
use std::collections::HashMap;

use dionysus::{
    binance::BinanceMarket,
    counselor::Counselor,
    finance::{Book, MarketEvent, MarketTick, Position, Sample, Token},
    historical_data::HistoricalData,
    strategy::{Decision, Strategy},
    time::TimeWindow,
    wallet::{BinanceWallet, DigitalWallet},
    ERROR,
};

pub struct Chrysus {
    token: Token,
    pub strategy: Strategy,
    capital: f64,
    positions: Vec<Position>,
    balance: f64,
    pub book: Book,
}

impl Chrysus {
    pub fn new(token: &Token) -> Self {
        Self {
            token: token.clone(),
            strategy: Strategy::default(),
            capital: 0.0,
            positions: Vec::new(),
            balance: 0.0,
            book: Book::default(),
        }
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.token.to_string(), self.strategy.name())
    }

    pub fn decide(&mut self, book: Book, history: &impl HistoricalData) -> Option<Decision> {
        self.book = book;
        if let Some(quote) = self.book.quote() {
            if let Ok(samples) = history.get_last(&self.token, &self.strategy.duration) {
                match self.strategy.run(&quote, samples) {
                    Ok(decision) => return Some(decision),
                    Err(e) => ERROR!("{:?}", e),
                };
            }
        }
        None
    }
}

pub enum MidasEvent {
    BookUpdate(Token),
    KLineUpdate(Token),
}

pub struct Midas {
    account: BinanceWallet,
    pub market: BinanceMarket,
    pub hesperides: HashMap<Token, Chrysus>,
    pub ticks: HashMap<Token, MarketTick>,
}

impl Midas {
    pub fn new(keys_file: &str) -> Midas {
        Self {
            account: BinanceWallet::new(&keys_file),
            market: BinanceMarket::default(),
            hesperides: HashMap::new(),
            ticks: HashMap::new(),
        }
    }

    pub fn init(&mut self) {
        self.market.day_ticker_all_service("USDT");
        match self.account.get_balance() {
            Ok(balance) => {
                for (token, asset) in balance {
                    self.add_token(&token);
                    self.set_balance(&token, asset.free);
                }
            }
            Err(e) => ERROR!("{:?}", e),
        };
    }

    fn init_token(&mut self, token: &Token, duration: &TimeWindow) {
        if let Err(e) = self.market.fetch_last(token, &duration) {
            ERROR!("ERROR {:?} {:?}.", e, token);
            return;
        }
        self.market.kline_service(token, &duration.resolution);
        self.market.order_book_service(token);
    }

    pub fn add_token(&mut self, token: &Token) -> bool {
        if self.hesperides.contains_key(&token) {
            ERROR!("Midas already touched {:?}.", token);
            return false;
        }
        self.hesperides.insert(token.clone(), Chrysus::new(token));
        let mut strategy = Strategy::default();
        strategy.duration.count = 200;
        self.set_strategy(token, &strategy);
        return self.is_token_ok(token);
    }

    pub fn get_history(&self, token: &Token) -> Option<&[Sample]> {
        if let Some(t) = self.hesperides.get(token) {
            match self.market.get_last(token, &t.strategy.duration) {
                Ok(samples) => return Some(samples),
                Err(e) => ERROR!("{:?}", e),
            }
        }
        None
    }

    pub fn is_token_ok(&self, token: &Token) -> bool {
        if self.hesperides.contains_key(&token) {
            // TODO
            true
        } else {
            false
        }
    }

    pub fn set_strategy(&mut self, token: &Token, strategy: &Strategy) {
        if let Some(t) = self.hesperides.get_mut(token) {
            t.strategy = strategy.clone();
            t.strategy.counselors.push(Counselor::MeanReversion(20));
            self.init_token(token, &strategy.duration);
        }
    }

    pub fn get(&self, token: &Token) -> Option<&Chrysus> {
        self.hesperides.get(token)
    }

    pub fn get_balance(&self) -> HashMap<Token, f64> {
        let mut m: HashMap<Token, f64> = HashMap::new();
        for (token, chrysus) in &self.hesperides {
            if chrysus.balance > 0.0 {
                m.insert(token.clone(), chrysus.balance.clone());
            }
        }
        m
    }

    fn set_balance(&mut self, token: &Token, balance: f64) {
        if let Some(t) = self.hesperides.get_mut(token) {
            t.balance = balance;
        }
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

    pub fn touch(&mut self) -> Vec<MidasEvent> {
        let mut events: Vec<MidasEvent> = Vec::new();
        for event in self.market.get_events() {
            match event {
                MarketEvent::KLine((token, sample)) => {
                    if let Some(t) = self.hesperides.get(&token) {
                        if let Err(e) = self.market.append(&token, &sample) {
                            ERROR!("{:?}", e);
                        } else {
                            if sample.resolution == t.strategy.duration.resolution {
                                events.push(MidasEvent::KLineUpdate(token));
                            }
                        }
                    }
                }
                MarketEvent::Ticks(ticks) => self.update_ticks(ticks),
                MarketEvent::OrderBook(book) => {
                    let token = book.token.clone();
                    if let Some(t) = &mut self.hesperides.get_mut(&token) {
                        t.decide(book, &self.market);
                        //if let Some(decision) = t.decide(book, &self.market) {
                        //    self.submit(&token, &decision);
                        //}
                        events.push(MidasEvent::BookUpdate(token));
                    }
                }
            };
        }
        events
    }

    fn submit(&mut self, token: &Token, decision: &Decision) {}
}
