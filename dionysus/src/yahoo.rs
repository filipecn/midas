use crate::cache::SampleCache;
use crate::finance::{DiError, Sample};
use crate::time::Period;
use yahoo_finance_api::{self as yahoo, time::OffsetDateTime};

#[derive(Default)]
pub struct YahooMarket {
    pub cache: SampleCache,
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
