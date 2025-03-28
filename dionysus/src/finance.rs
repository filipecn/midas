use std::hash::Hash;

use super::time::{Date, TimeUnit};
use ta::{Close, High, Low, Open, Volume};

#[derive(Debug, PartialEq, Eq)]
pub enum DiError {
    NotFound,
    NotImplemented,
    Message(String),
    Error,
    OutOfBounds,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Token {
    Symbol(String),
    Currency(String),
    Pair((String, String)),
    #[default]
    None,
}

impl Token {
    pub fn pair(symbol: &str, currency: &str) -> Token {
        Token::Pair((symbol.to_string(), currency.to_string()))
    }

    pub fn to_string(&self) -> String {
        match self {
            Self::Pair((symbol, currency)) => format!("{}{}", symbol, currency),
            Self::Symbol(symbol) => symbol.clone(),
            Self::Currency(currency) => currency.clone(),
            Self::None => "NONE".to_string(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Pair((symbol, currency)) => format!("{}/{}", symbol, currency),
            Self::Symbol(symbol) => symbol.clone(),
            Self::Currency(currency) => currency.clone(),
            Self::None => "NONE".to_string(),
        }
    }

    pub fn get_symbol(&self) -> String {
        match self {
            Self::Pair((symbol, _)) => symbol.clone(),
            Self::Symbol(symbol) => symbol.clone(),
            _ => String::new(),
        }
    }

    pub fn get_currency(&self) -> String {
        match self {
            Self::Pair((_, currency)) => currency.clone(),
            Self::Currency(currency) => currency.clone(),
            _ => String::new(),
        }
    }

    pub fn symbol(&self) -> Token {
        match self {
            Self::Pair((s, _)) => Token::Symbol(s.clone()),
            Self::Symbol(s) => Token::Symbol(s.clone()),
            _ => Token::Symbol(String::new()),
        }
    }
}

impl Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_string().hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct Position {
    pub token: Token,
    pub quantity: f64,
    pub price: f64,
    pub date: Date,
}

/// A financial quote is the price at which an asset was last traded, or the
/// price at which it can be bought or sold. It can also refer to the bid
/// or ask price of a security.
#[derive(Debug, Clone)]
pub struct Quote {
    pub token: Token,
    /// The highest price a buyer is willing to pay.
    pub bid: f64,
    /// The lowest price a seller is willing to accept.
    pub ask: f64,
    pub biddate: Date,
    pub askdate: Date,
}

/// Summary of price movements of an asset over a time period.
#[derive(Debug, Default, Clone)]
pub struct Sample {
    pub resolution: TimeUnit,
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

impl Low for Sample {
    fn low(&self) -> f64 {
        self.low
    }
}

impl High for Sample {
    fn high(&self) -> f64 {
        self.high
    }
}

impl Close for Sample {
    fn close(&self) -> f64 {
        self.close
    }
}

impl Open for Sample {
    fn open(&self) -> f64 {
        self.open
    }
}

impl Volume for Sample {
    fn volume(&self) -> f64 {
        self.volume as f64
    }
}

impl Sample {
    pub fn date(&self) -> Date {
        Date::from_timestamp(self.timestamp)
    }
}

#[derive(Clone)]
pub struct BookLine {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Default, Clone)]
pub struct Book {
    pub token: Token,
    pub bids: Vec<BookLine>,
    pub asks: Vec<BookLine>,
}

impl Book {
    pub fn quote(&self) -> Option<Quote> {
        if self.bids.is_empty() || self.asks.is_empty() {
            None
        } else {
            Some(Quote {
                bid: if let Some(l) = self
                    .bids
                    .iter()
                    .max_by(|a, b| a.price.partial_cmp(&b.price).unwrap())
                {
                    l.price
                } else {
                    0.0
                },
                ask: if let Some(l) = self
                    .bids
                    .iter()
                    .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap())
                {
                    l.price
                } else {
                    0.0
                },
                token: self.token.clone(),
                biddate: Date::now(),
                askdate: Date::now(),
            })
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct MarketTick {
    pub token: Token,
    pub price: f64,
    pub change_pct: f64,
}

pub enum MarketEvent {
    KLine((Token, Sample)),
    Ticks(Vec<MarketTick>),
    OrderBook(Book),
}
