use super::data::HistoricalData;
use super::finance::*;
use super::time::*;

/// A signal represents the sentiment of an strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Buy,
    Sell,
    None,
}

/// Bot strategies emit signals for each given new quote.
pub trait Oracle {
    fn run(&self, quote: &Quote, history: &SymbolHistory) -> Result<Signal, String>;
}

pub struct TraceOracle;
pub struct MeanReversionOracle;

impl Oracle for TraceOracle {
    fn run(&self, quote: &Quote, _history: &SymbolHistory) -> Result<Signal, String> {
        println!("{:?}", quote);
        Ok(Signal::None)
    }
}

impl Oracle for MeanReversionOracle {
    fn run(&self, quote: &Quote, history: &SymbolHistory) -> Result<Signal, String> {
        println!("{:?}\n{:?}", quote, history);
        Ok(Signal::None)
    }
}
