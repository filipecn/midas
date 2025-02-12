use crate::time::TimeUnit;

use super::finance::Sample;
use super::time::{Period, Resolution};
use std::{collections::HashMap, sync::Arc};
use yahoo_finance_api::{self as yahoo, time::OffsetDateTime};

pub struct HistoricalData {
    pub data: Arc<HashMap<String, Vec<Sample>>>,
}

fn convert_to_yahoo_interval(resolution: Resolution) -> String {
    let mut s = resolution.frequency.to_string();
    match resolution.unit {
        TimeUnit::Min => s.push_str("m"),
        TimeUnit::Hour => s.push_str("h"),
        TimeUnit::Day => s.push_str("d"),
        TimeUnit::Week => s.push_str("wk"),
        TimeUnit::Month => s.push_str("mo"),
        TimeUnit::Year => s.push_str("y"),
    }
    s
}

async fn fetch_history(symbol: &str, period: Period) -> Option<Vec<Sample>> {
    let provider = yahoo::YahooConnector::new().unwrap();
    let start = OffsetDateTime::from_unix_timestamp(period.start.local.timestamp()).unwrap();
    let end = OffsetDateTime::from_unix_timestamp(period.end().local.timestamp()).unwrap();
    let response = provider
        .get_quote_history_interval(
            symbol,
            start,
            end,
            &convert_to_yahoo_interval(period.duration.resolution),
        )
        .unwrap();
    let quotes = response.quotes().unwrap();

    let mut data = Vec::new();

    for quote in quotes.iter() {
        data.push(Sample {
            timestamp: quote.timestamp,
            open: quote.open,
            high: quote.high,
            low: quote.low,
            close: quote.close,
            volume: quote.volume,
        })
    }

    Some(data)
}

impl HistoricalData {
    pub fn new() -> HistoricalData {
        HistoricalData {
            data: Arc::new(HashMap::new()),
        }
    }

    pub fn get_period(&self, symbol: &str, period: Period) -> Option<&[Sample]> {
        match self.data.get(symbol) {
            Some(history) => {
                let start_index = history
                    .binary_search_by(|sample| {
                        let t = period.start.local.timestamp() as u64;
                        sample.timestamp.cmp(&t)
                    })
                    .unwrap_err() as usize;
                let end_index = history
                    .binary_search_by(|sample| {
                        let t = period.end().local.timestamp() as u64;
                        sample.timestamp.cmp(&t)
                    })
                    .unwrap_err() as usize;

                Some(&history[start_index..end_index])
            }
            _ => None,
        }
    }

    pub async fn fetch(&mut self, symbols: Vec<String>, period: Period) {
        let mut data = HashMap::new();
        for symbol in symbols {
            let history = fetch_history(&symbol[..], period).await.unwrap();
            data.insert(symbol, history);
        }
        self.data = Arc::new(data);
    }
}

#[cfg(test)]
mod tests {
    use super::convert_to_yahoo_interval;
    use crate::time::{Resolution, TimeUnit};

    #[test]
    fn test_convert_to_yahoo_interval() {
        assert_eq!(
            convert_to_yahoo_interval(Resolution {
                unit: TimeUnit::Min,
                frequency: 1
            }),
            "1m".to_string()
        );
    }
}
