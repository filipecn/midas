use super::method::Method;

pub struct Backtrace {}

pub async fn backtrace(symbol: &str, period: Period, strategy: &impl Oracle) {
    let mut history = HistoricalData::new();
    history.fetch(vec![symbol.to_string()], period).await;
    let period_data;
    match history.get_period(symbol, period) {
        Some(data) => period_data = data,
        None => return,
    }
    let mut i = 0;
    period_data.iter().for_each(|sample| {
        let quote = Quote::from_sample(sample);
        let symbol_history = SymbolHistory::new(&period_data[..i]);
        i += 1;
        match strategy.run(&quote, &symbol_history) {
            Ok(signal) => println!("{:?}", signal),
            Err(e) => println!("{:?}", e),
        }
        println!("{:?}", sample)
    });
}
