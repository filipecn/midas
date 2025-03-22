use slog::slog_error;
use std::collections::HashMap;

use dionysus::{
    binance::BinanceMarket,
    finance::{Book, MarketEvent, Position, Token},
    historical_data::HistoricalData,
    oracles::Advice,
    strategy::{Decision, Strategy},
    wallet::{BinanceWallet, DigitalWallet},
    ERROR,
};

pub struct Tyche {
    token: Token,
    strategy: Strategy,
    capital: f64,
    positions: Vec<Position>,
    balance: f64,
    book: Book,
}

impl Tyche {
    pub fn new(token: &Token) -> Self {
        Self {
            token: token.clone(),
            strategy: Strategy::new(),
            capital: 0.0,
            positions: Vec::new(),
            balance: 0.0,
            book: Book::default(),
        }
    }

    pub fn decide(&mut self, book: Book, history: &impl HistoricalData) -> Option<Decision> {
        if let Some(quote) = book.quote() {
            if let Ok(samples) = history.get_last(&book.token, &self.strategy.time_window()) {
                match self.strategy.run(&quote, samples) {
                    Ok(decision) => return Some(decision),
                    Err(e) => ERROR!("{:?}", e),
                };
            }
        }
        None
    }
}

pub struct Midas {
    account: BinanceWallet,
    market: BinanceMarket,
    phrygia: HashMap<Token, Tyche>,
}

impl Midas {
    pub fn new(keys_file: &str) -> Midas {
        Self {
            account: BinanceWallet::new(&keys_file),
            market: BinanceMarket::default(),
            phrygia: HashMap::new(),
        }
    }

    pub fn init(&mut self) {}

    pub fn add_token(&mut self, token: &Token) {
        if self.phrygia.contains_key(&token) {
            ERROR!("Midas already touched {:?}.", token);
        }
        self.phrygia.insert(token.clone(), Tyche::new(token));
        self.set_strategy(token, Strategy::new());
    }

    pub fn set_strategy(&mut self, token: &Token, strategy: Strategy) {
        if let Some(t) = self.phrygia.get_mut(token) {
            t.strategy = strategy;
        } else {
            ERROR!("Cannot set strategy for untouched token {:?}.", token);
        }
    }

    fn set_balance(&mut self, token: &Token, balance: f64) {
        if let Some(t) = self.phrygia.get_mut(token) {
            t.balance = balance;
        } else {
            ERROR!("Cannot set balance for untouched token {:?}.", token);
        }
    }

    pub fn touch(&mut self) {
        match self.account.get_balance() {
            Ok(balance) => {
                for (token, asset) in balance {
                    self.set_balance(&token, asset.free);
                }
            }
            Err(e) => ERROR!("{:?}", e),
        };
        for event in self.market.get_events() {
            match event {
                MarketEvent::KLine((token, sample)) => {
                    if let Err(e) = self.market.append(&token, &sample) {
                        ERROR!("{:?}", e);
                    }
                }
                MarketEvent::OrderBook(book) => {
                    let token = book.token.clone();
                    if let Some(t) = self.phrygia.get_mut(&token) {
                        if let Some(decision) = t.decide(book, &self.market) {
                            self.submit(&token, &decision);
                        }
                    }
                }
                _ => (),
            };
        }
    }

    fn submit(&mut self, token: &Token, decision: &Decision) {}
}
