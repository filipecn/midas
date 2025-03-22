use crate::finance::{DiError, Quote, Sample};
use crate::oracles::{Advice, Oracle, Signal};
use crate::time::TimeWindow;

#[derive(Default)]
pub struct Decision {
    advice: Advice,
    pct: f64,
}

pub struct Strategy {
    pub oracles: Vec<Oracle>,
    duration: TimeWindow,
}

impl Strategy {
    pub fn new() -> Self {
        Self {
            oracles: Vec::new(),
            duration: TimeWindow::default(),
        }
    }

    pub fn time_window(&self) -> TimeWindow {
        self.duration.clone()
    }

    pub fn run(&self, quote: &Quote, history: &[Sample]) -> Result<Decision, DiError> {
        let quote_time = quote.biddate.clone();
        let mut buy_signal_count = 0;
        let mut _sell_signal_count = 0;
        let mut _none_signal_count = 0;
        for strategy in &self.oracles {
            let required_samples = strategy.required_samples();
            //let period_data;
            //match history.get_previous_samples(
            //    &quote_time,
            //    &quote.symbol[..],
            //    &self.time_resolution,
            //    required_samples as u64,
            //) {
            //    Ok(data) => period_data = data,
            //    Err(e) => return Err(e),
            //}
            //match strategy.run(&quote, &period_data[..]) {
            //    Ok(Signal::Buy) => buy_signal_count += 1,
            //    Ok(Signal::Sell) => _sell_signal_count += 1,
            //    Ok(Signal::None) => _none_signal_count += 1,
            //    Err(_s) => continue,
            //}
        }
        //if buy_signal_count > 0 {
        //    return Ok(Signal::Buy);
        //}
        Ok(Decision::default())
    }
}
