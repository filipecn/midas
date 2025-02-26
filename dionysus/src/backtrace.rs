use super::finance::DiError;
use super::time::Period;
use crate::brownian::BrownianMotionMarket;
use crate::historical_data::HistoricalData;
use crate::strategy::Strategy;
use crate::yahoo::YahooMarket;

pub struct Backtrace {}

pub fn backtrace(symbol: &str, period: &Period, strategy: &mut Strategy) -> Result<(), DiError> {
    match symbol {
        "brownian" => {
            let mut history = BrownianMotionMarket::default();
            //history.fetch_one(symbol, &period)?;
            return run_backtrace(&symbol, &period, strategy, &history);
        }
        &_ => {
            let mut history = YahooMarket::default();
            //history.fetch_one(symbol, &period)?;
            return run_backtrace(&symbol, &period, strategy, &history);
        }
    }
}

fn run_backtrace(
    symbol: &str,
    period: &Period,
    strategy: &mut Strategy,
    history: &impl HistoricalData,
) -> Result<(), DiError> {
    //let period_data;
    //match history.get_period(symbol, &period) {
    //    Ok(data) => period_data = data,
    //    Err(e) => return Err(e),
    //}
    //period_data.iter().for_each(|sample| {
    //    let quote = Quote::from_sample(&symbol, &sample);
    //    match strategy.run(&quote, history) {
    //        Ok(signal) => println!("{:?}", signal),
    //        Err(e) => println!("{:?}", e),
    //    }
    //});
    Ok(())
}
