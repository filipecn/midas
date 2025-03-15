use crate::binance::BinanceMarket;
use crate::brownian::{generate_brownian_data, BrownianMotionMarket};
use crate::finance::{DiError, Quote, Sample, Token};
use crate::time::{TimeUnit, TimeWindow};
use crate::yahoo::YahooMarket;
use std::cmp::Ordering;

pub fn sample_quotes(quotes: &[Quote], resolution: &TimeUnit) -> Vec<Sample> {
    let mut samples = Vec::new();
    if quotes.is_empty() {
        return samples;
    }

    let mut sample = Sample::default();
    let sample_size_in_seconds = resolution.num_seconds();
    let mut sample_start = quotes[0].biddate;

    for quote in quotes {
        let curr_size_in_seconds = (quote.biddate - sample_start).num_seconds();
        if curr_size_in_seconds >= sample_size_in_seconds || sample.volume == 0 {
            if sample.volume != 0 {
                samples.push(sample.clone());
            }
            sample_start = quote.biddate.clone();
            sample.timestamp = quote.biddate.timestamp() as u64;
            sample.open = quote.bid.clone();
            sample.high = quote.bid.clone();
            sample.low = quote.bid.clone();
            sample.close = quote.bid.clone();
            sample.volume = 1;
        } else {
            sample.volume += 1;
            sample.close = quote.bid.clone();
            if quote.bid.total_cmp(&sample.high) == Ordering::Greater {
                sample.high = quote.bid.clone();
            }
            if quote.bid.total_cmp(&sample.low) == Ordering::Less {
                sample.low = quote.bid.clone();
            }
        }
    }
    if sample.volume != 0 {
        samples.push(sample.clone());
    }
    samples
}

macro_rules! _check {
    ($call:expr) => {
        if let Err(e) = $call {
            return Err(e);
        }
    };
}

pub trait HistoricalData {
    fn append(&mut self, token: &Token, sample: &Sample) -> Result<(), DiError>;
    fn fetch_last(&mut self, token: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError>;
    fn get_last(&mut self, token: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError>;

    //fn get_previous_samples(
    //    &self,
    //    date: &Date,
    //    symbol: &str,
    //    _resolution: &TimeUnit,
    //    sample_count: u64,
    //) -> Result<&[Sample], DiError> {
    //    match self.get_symbol(symbol) {
    //        Ok(history) => {
    //            let end_index;
    //            match history.binary_search_by(|sample| {
    //                let t = date.timestamp() as u64;
    //                sample.timestamp.cmp(&t)
    //            }) {
    //                Ok(index) => end_index = index as usize,
    //                Err(index) => end_index = index,
    //            }
    //            let start_index = if end_index >= sample_count as usize {
    //                end_index - sample_count as usize
    //            } else {
    //                0
    //            };

    //            Ok(&history[start_index..end_index])
    //        }
    //        _ => Err(DiError::NotFound),
    //    }
    //}
    //fn get_period(&self, symbol: &str, period: &Period) -> Result<&[Sample], DiError> {
    //    match self.get_symbol(symbol) {
    //        Ok(history) => {
    //            let start_index: usize;
    //            match history.binary_search_by(|sample| {
    //                let t = period.start().timestamp() as u64;
    //                sample.timestamp.cmp(&t)
    //            }) {
    //                Ok(index) => start_index = index as usize,
    //                Err(index) => start_index = index,
    //            }
    //            let end_index = history
    //                .binary_search_by(|sample| {
    //                    let t = period.end().timestamp() as u64;
    //                    sample.timestamp.cmp(&t)
    //                })
    //                .unwrap_err() as usize;

    //            Ok(&history[start_index..end_index])
    //        }
    //        _ => Err(DiError::NotFound),
    //    }
    //}
    //fn fetch_last(&mut self, _symbol: &str, _duration: &TimeWindow) -> Result<(), DiError> {
    //    Err(DiError::NotImplemented)
    //}
    //fn fetch(&mut self, symbols: &[String], period: &Period) -> Result<(), DiError> {
    //    for symbol in symbols {
    //        self.fetch_one(&symbol[..], &period)?;
    //    }
    //    Ok(())
    //}
}

impl HistoricalData for BinanceMarket {
    fn append(&mut self, token: &Token, sample: &Sample) -> Result<(), DiError> {
        let v = vec![sample.clone()];
        self.cache.write(token, &v[..])
    }
    fn fetch_last(&mut self, token: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        let mut samples: Vec<Sample> = Vec::new();
        match self.market.get_klines(
            token.to_string().as_str(),
            duration.resolution.name(),
            duration.count as u16,
            None,
            None,
        ) {
            Ok(klines) => match klines {
                binance::model::KlineSummaries::AllKlineSummaries(klines) => {
                    for kline in klines {
                        samples.push(Sample {
                            resolution: duration.resolution.clone(),
                            timestamp: kline.open_time as u64,
                            open: kline.open.parse::<f64>().unwrap(),
                            high: kline.high.parse::<f64>().unwrap(),
                            low: kline.low.parse::<f64>().unwrap(),
                            close: kline.close.parse::<f64>().unwrap(),
                            volume: kline.number_of_trades as u64,
                        });
                    }
                }
            },
            Err(e) => return Err(DiError::Message(format!("{:?}", e))),
        };
        if !samples.is_empty() {
            self.cache.write(token, &samples[..])?;
        }
        self.cache.read(token, duration)
    }
    fn get_last(&mut self, token: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        self.cache.read(token, duration)
    }
}

impl HistoricalData for YahooMarket {
    fn append(&mut self, _token: &Token, _sample: &Sample) -> Result<(), DiError> {
        Err(DiError::NotImplemented)
    }
    fn fetch_last(&mut self, _token: &Token, _duration: &TimeWindow) -> Result<&[Sample], DiError> {
        Err(DiError::NotImplemented)
    }
    fn get_last(&mut self, _token: &Token, _duration: &TimeWindow) -> Result<&[Sample], DiError> {
        //let period_end = Date::now();
        Err(DiError::NotImplemented)
    }
    //fn fetch_one(&mut self, symbol: &str, period: &Period) -> Result<(), DiError> {
    //    match fetch_history(symbol, &period) {
    //        Ok(history) => {
    //            self.cache.insert(symbol.to_string(), history);
    //            Ok(())
    //        }
    //        Err(_) => Err(DiError::NotFound),
    //    }
    //}
}

impl HistoricalData for BrownianMotionMarket {
    fn append(&mut self, _token: &Token, _sample: &Sample) -> Result<(), DiError> {
        Err(DiError::NotImplemented)
    }
    fn fetch_last(&mut self, token: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        let quotes = generate_brownian_data(self.mu, self.sigma, &duration);
        let samples = sample_quotes(&quotes[..], &duration.resolution);
        self.cache.write(token, &samples[..])?;
        self.cache.read(token, duration)
    }
    fn get_last(&mut self, token: &Token, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        self.cache.read(token, duration)
    }
}
