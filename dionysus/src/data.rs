use crate::finance::{DiError, Quote, Sample};
use crate::time::{Date, Period, TimeUnit, TimeWindow};
use binance;
use rand::thread_rng;
use rand_distr::{Distribution, Normal};
use std::cmp::Ordering;
use std::collections::HashMap;
use yahoo_finance_api::{self as yahoo, time::OffsetDateTime};

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

macro_rules! check {
    ($call:expr) => {
        if let Err(e) = $call {
            return Err(e);
        }
    };
}

pub trait HistoricalData {
    fn fetch_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<(), DiError>;
    fn get_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<&[Sample], DiError>;

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

type SampleCache = HashMap<TimeUnit, Vec<Sample>>;
type SymbolCache = HashMap<String, SampleCache>;

#[derive(Default)]
pub struct Cache {
    data: SymbolCache,
}

impl Cache {
    pub fn contains(&self, symbol: &str, period: &Period) -> bool {
        if let Some(unit_cache) = self.data.get(symbol) {
            if let Some(cache) = unit_cache.get(&period.duration.resolution) {
                if cache.is_empty() {
                    return false;
                }
                if cache.first().unwrap().timestamp > period.end().timestamp() as u64 {
                    return false;
                }
                if cache.last().unwrap().timestamp < period.end().timestamp() as u64 {
                    return false;
                }
                return true;
            }
        }
        false
    }
    pub fn read(&self, symbol: &str, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        match self
            .data
            .get(symbol)
            .and_then(|unit_cache| unit_cache.get(&duration.resolution))
        {
            Some(samples) => {
                let first_index = samples.len().saturating_sub(duration.count as usize);
                return Ok(&samples[first_index..]);
            }
            None => return Err(DiError::NotFound),
        }
    }
    pub fn write(&mut self, symbol: &str, samples: &[Sample]) -> Result<(), DiError> {
        let v: Vec<Sample> = samples.iter().map(|sample| sample.clone()).collect();
        if v.is_empty() {
            return Ok(());
        }
        let resolution = v[0].resolution.clone();
        match &mut self.data.get_mut(symbol) {
            Some(unit_cache) => match unit_cache.get_mut(&resolution) {
                Some(cache) => {
                    if v[0].timestamp <= cache[0].timestamp {
                        return Err(DiError::NotImplemented);
                    } else {
                        for sample in v {
                            cache.push(sample);
                        }
                    }
                }
                None => {
                    unit_cache.insert(resolution, v);
                }
            },
            None => {
                let mut sample_cache = SampleCache::new();
                sample_cache.insert(resolution, v);
                self.data.insert(symbol.to_string(), sample_cache);
            }
        }
        Ok(())
    }
}

pub struct BinanceProvider {
    pub market: binance::market::Market,
    cache: Cache,
}

impl BinanceProvider {
    pub fn new() -> Self {
        Self {
            market: binance::api::Binance::new(None, None),
            cache: Cache::default(),
        }
    }
}

impl HistoricalData for BinanceProvider {
    fn fetch_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<(), DiError> {
        let mut samples: Vec<Sample> = Vec::new();
        match self.market.get_klines(
            symbol,
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
            self.cache.write(symbol, &samples[..])?;
        }
        Ok(())
    }
    fn get_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        self.fetch_last(symbol, duration)?;
        self.cache.read(symbol, duration)
    }
}

pub struct YahooProvider {
    cache: SampleCache,
}

impl YahooProvider {
    pub fn new() -> YahooProvider {
        YahooProvider {
            cache: HashMap::new(),
        }
    }
}

fn fetch_history(symbol: &str, period: &Period) -> Result<Vec<Sample>, DiError> {
    let provider = yahoo::YahooConnector::new().unwrap();
    let start = OffsetDateTime::from_unix_timestamp(period.start().timestamp()).unwrap();
    let end = OffsetDateTime::from_unix_timestamp(period.end().timestamp()).unwrap();
    let response;
    match provider.get_quote_history_interval(
        symbol,
        start,
        end,
        &period.duration.resolution.name(),
    ) {
        Ok(y_response) => response = y_response,
        Err(e) => panic!("{:?}", e),
    };
    let quotes = response.quotes().unwrap();

    let mut data = Vec::new();

    for quote in quotes.iter() {
        data.push(Sample {
            resolution: period.duration.resolution,
            timestamp: quote.timestamp,
            open: quote.open,
            high: quote.high,
            low: quote.low,
            close: quote.close,
            volume: quote.volume,
        })
    }
    Ok(data)
}

impl HistoricalData for YahooProvider {
    fn fetch_last(&mut self, _symbol: &str, _duration: &TimeWindow) -> Result<(), DiError> {
        Err(DiError::NotImplemented)
    }
    fn get_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        //let period_end = Date::now();
        check!(self.fetch_last(symbol, &duration));
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

pub struct BrownianMotionProvider {
    // - Drift (mu): 0.2
    pub mu: f64,
    // - Volatility (sigma): 0.4
    pub sigma: f64,
    // - Time horizon: 1.0
    pub time_horizon: f64,
    cache: Cache,
}

impl BrownianMotionProvider {
    pub fn new() -> BrownianMotionProvider {
        BrownianMotionProvider {
            mu: 0.2,
            sigma: 0.4,
            time_horizon: 1.0,
            cache: Cache::default(),
        }
    }
}

pub fn generate_brownian_data(mu: f64, sigma: f64, duration: &TimeWindow) -> Vec<Quote> {
    // generate data in minute resolution, then sample
    let total_minutes = duration.num_minutes() as usize;
    let time_increment = TimeWindow::minutes(1);

    let mut quotes = Vec::with_capacity(total_minutes);
    let normal = Normal::new(0.0, 1.0).unwrap();

    // from https://github.com/nzengi/stochastic-gbm/blob/master/src/gbm.rs
    let dt = 1.0 / total_minutes as f64;
    let drift = (mu - 0.5 * sigma.powi(2)) * dt;
    let vol_sqrt_dt = sigma * dt.sqrt();
    let mut old_price = 500.0;
    let mut rng = thread_rng();
    let mut quote_date = Date::now();
    for _ in 0..total_minutes {
        let z = normal.sample(&mut rng);
        let price = old_price * (drift + vol_sqrt_dt * z).exp();
        old_price = price;
        let quote = Quote {
            symbol: "brownian".to_string(),
            biddate: quote_date.clone(),
            askdate: quote_date.clone(),
            bid: price,
            ask: price,
        };
        quotes.push(quote);
        quote_date += time_increment;
    }

    quotes
}

impl HistoricalData for BrownianMotionProvider {
    fn fetch_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<(), DiError> {
        let quotes = generate_brownian_data(self.mu, self.sigma, &duration);
        let samples = sample_quotes(&quotes[..], &duration.resolution);
        self.cache.write(symbol, &samples[..])
    }
    fn get_last(&mut self, symbol: &str, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        self.fetch_last(symbol, duration)?;
        self.cache.read(symbol, duration)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        data::{generate_brownian_data, sample_quotes, BrownianMotionProvider},
        time::{TimeUnit, TimeWindow},
    };

    #[test]
    fn test_sample_quotes() {
        let bmp = BrownianMotionProvider::new();
        let quotes = generate_brownian_data(bmp.mu, bmp.sigma, &TimeWindow::days(1));
        assert_eq!(quotes.len(), 1440);
        let samples = sample_quotes(&quotes[..], &TimeUnit::Min(2));
        assert_eq!(samples.len(), 720);
    }
}
