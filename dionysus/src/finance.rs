use serde::{Deserialize, Serialize};
use std::convert::From;
use std::hash::Hash;

use super::time::{Date, TimeUnit};
use ta::{Close, High, Low, Open, Volume};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct F64 {
    pub value: f64,
}

impl F64 {
    pub fn new(value: f64) -> Self {
        Self { value }
    }
}

impl Eq for F64 {}

impl From<f64> for F64 {
    fn from(item: f64) -> Self {
        Self { value: item }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiError {
    NotFound,
    NotImplemented,
    Message(String),
    Error,
    OutOfBounds,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Token {
    Symbol(String),
    Currency(String),
    Pair((String, String)),
    #[default]
    None,
}

impl Token {
    pub fn from_string(s: &String) -> Token {
        Token::Pair((s[0..3].to_string(), s[3..6].to_string()))
    }

    pub fn pair(symbol: &str, currency: &str) -> Token {
        Token::Pair((symbol.to_string(), currency.to_string()))
    }

    pub fn is_pair(&self) -> bool {
        match self {
            Self::Pair((_, _)) => true,
            _ => false,
        }
    }

    pub fn reverse(&self) -> Token {
        match self {
            Self::Pair((symbol, currency)) => Token::pair(currency, symbol),
            _ => self.clone(),
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub token: Token,
    pub quantity: f64,
    pub price: f64,
    pub date: Date,
    pub attached_order: Option<usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum OrderType {
    #[default]
    Market,
    Limit,
    StopMarket,
    StopLimit,
}

impl OrderType {
    pub fn from_string(name: &String) -> OrderType {
        match &name[..] {
            "Market" => OrderType::Market,
            "Limit" => OrderType::Limit,
            "StopMarket" => OrderType::StopMarket,
            "StopLimit" => OrderType::StopLimit,
            _ => OrderType::Market,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn from_string(name: &String) -> Side {
        if name == &String::from("Buy") {
            Side::Buy
        } else {
            Side::Sell
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum TimeInForce {
    #[default]
    GTC, // Good Till Cancel
    IOC, // Immediate or Cancel
    FOK, // Fill or Kill
}

impl TimeInForce {
    pub fn from_string(name: &String) -> TimeInForce {
        match &name[..] {
            "GTC" => TimeInForce::GTC,
            "IOC" => TimeInForce::IOC,
            "FOK" => TimeInForce::FOK,
            _ => TimeInForce::GTC,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub index: usize,
    pub position_index: Option<usize>,
    pub id: Option<i64>,
    pub token: Token,
    pub date: Date,
    pub side: Side,
    pub quantity: f64,
    pub price: f64,
    pub stop_price: Option<f64>,
    pub order_type: OrderType,
    pub tif: TimeInForce,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderStatus {
    pub order: Order,
    pub executed_qty: f64,
    pub status: String,
    pub update_time: Date,
    pub is_working: bool,
}

/// A financial quote is the price at which an asset was last traded, or the
/// price at which it can be bought or sold. It can also refer to the bid
/// or ask price of a security.
#[derive(Debug, Clone)]
pub struct Quote {
    pub token: Token,
    /// The highest price a buyer is willing to pay.
    pub bid: Option<f64>,
    /// The lowest price a seller is willing to accept.
    pub ask: Option<f64>,
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

#[derive(Clone, Serialize, Deserialize)]
pub struct BookLine {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Default, Clone, Serialize, Deserialize)]
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
                    Some(l.price)
                } else {
                    None
                },
                ask: if let Some(l) = self
                    .bids
                    .iter()
                    .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap())
                {
                    Some(l.price)
                } else {
                    None
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
