use super::time::{Date, TimeUnit};
use ta::{Close, High, Low, Open, Volume};

#[derive(Debug, PartialEq, Eq)]
pub enum DiError {
    NotFound,
    NotImplemented,
    Message(String),
    Error,
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

    pub fn get_symbol(&self) -> String {
        match self {
            Self::Pair((symbol, _)) => symbol.clone(),
            Self::Symbol(symbol) => symbol.clone(),
            _ => String::new(),
        }
    }
}

/// A financial quote is the price at which an asset was last traded, or the
/// price at which it can be bought or sold. It can also refer to the bid
/// or ask price of a security.
#[derive(Debug, Clone)]
pub struct Quote {
    pub symbol: String,
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

impl Quote {
    pub fn from_sample(symbol: &str, sample: &Sample) -> Quote {
        Quote {
            symbol: symbol.to_string(),
            bid: (sample.open + sample.close) / 2.0,
            ask: (sample.open + sample.close) / 2.0,
            biddate: Date::from_timestamp(sample.timestamp),
            askdate: Date::from_timestamp(sample.timestamp),
        }
    }
}
