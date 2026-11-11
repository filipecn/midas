use crate::market::types::TechnicalAnalysisRequest;
use market_data::{
    EnhancedMarketSeries, Finnhub, Interval, MarketClient, MarketSeries, Twelvedata, YahooFin,
    YahooRange,
};
use std::error::Error;

#[derive(Clone, Debug)]
pub enum PublisherConfig {
    Yahoo,
    TwelveData { api_key: String },
    Finnhub { api_key: String },
}

#[derive(Clone)]
pub struct MultiPublisherClient {
    fallback_chain: Vec<PublisherConfig>,
}

impl MultiPublisherClient {
    pub fn new(fallback_chain: Vec<PublisherConfig>) -> Self {
        Self { fallback_chain }
    }

    /// Default setup trying unauthenticated Yahoo first, then API keys if provided
    pub fn default_with_keys(twelvedata_key: Option<String>, finnhub_key: Option<String>) -> Self {
        let mut chain = vec![PublisherConfig::Yahoo];

        if let Some(api_key) = twelvedata_key {
            chain.push(PublisherConfig::TwelveData { api_key });
        }
        if let Some(api_key) = finnhub_key {
            chain.push(PublisherConfig::Finnhub { api_key });
        }

        Self::new(chain)
    }

    /// Iterates through the fallback chain until a provider succeeds
    pub async fn fetch_enhanced_series(
        &self,
        req: &TechnicalAnalysisRequest,
    ) -> Result<EnhancedMarketSeries, Box<dyn Error + Send + Sync>> {
        let mut errors = Vec::new();

        for (index, publisher) in self.fallback_chain.iter().enumerate() {
            match self.fetch_from_provider(publisher, req).await {
                Ok(enhanced_series) => return Ok(enhanced_series),
                Err(err) => {
                    if index + 1 < self.fallback_chain.len() {
                        log::warn!(
                            target: "midas",
                            "Publisher {:?} failed for {}: {}. Retrying next provider...",
                            publisher, req.ticker, err
                        );
                    } else {
                        log::warn!(
                            target: "midas",
                            "Publisher {:?} failed for {}: {}. No further providers are configured.",
                            publisher, req.ticker, err
                        );
                    }
                    errors.push(format!("{publisher:?}: {err}"));
                }
            }
        }

        if errors.is_empty() {
            return Err("No active publishers available".into());
        }

        if self.fallback_chain.len() == 1
            && matches!(self.fallback_chain.first(), Some(PublisherConfig::Yahoo))
        {
            return Err(format!(
                "Yahoo Finance could not load {} ({}). No alternative provider is configured; set TWELVEDATA_API_KEY or FINNHUB_API_KEY to enable fallback.",
                req.ticker, errors[0]
            )
            .into());
        }

        Err(format!(
            "All configured market data providers failed: {}",
            errors.join("; ")
        )
        .into())
    }

    async fn fetch_from_provider(
        &self,
        publisher: &PublisherConfig,
        req: &TechnicalAnalysisRequest,
    ) -> Result<EnhancedMarketSeries, Box<dyn Error + Send + Sync>> {
        let series = match publisher {
            PublisherConfig::Yahoo => {
                let client = MarketClient::new(YahooFin::new());
                let request = if matches!(req.interval, Interval::Daily) {
                    client.site.daily_series(&req.ticker, YahooRange::Max)
                } else {
                    client.site.intraday_series(
                        &req.ticker,
                        req.interval.clone(),
                        YahooRange::Max,
                    )?
                };
                client.fetch(request).await?
            }
            PublisherConfig::TwelveData { api_key } => {
                let client = MarketClient::new(Twelvedata::new(api_key.clone()));
                let request = if matches!(req.interval, Interval::Daily) {
                    client
                        .site
                        .daily_series(&req.ticker, req.num_samples as u32)
                } else {
                    client.site.intraday_series(
                        &req.ticker,
                        req.num_samples as u32,
                        req.interval.clone(),
                    )?
                };
                client.fetch(request).await?
            }
            PublisherConfig::Finnhub { api_key } => {
                let client = MarketClient::new(Finnhub::new(api_key.clone()));
                let now = chrono::Utc::now().timestamp();
                let from = now - (req.num_samples as i64 * 86_400);
                let request = if matches!(req.interval, Interval::Daily) {
                    client.site.daily_series(&req.ticker, from, now)
                } else {
                    client
                        .site
                        .intraday_series(&req.ticker, from, now, req.interval.clone())?
                };
                client.fetch(request).await?
            }
        };

        enhance_series(series, req)
    }
}

fn enhance_series(
    series: MarketSeries,
    req: &TechnicalAnalysisRequest,
) -> Result<EnhancedMarketSeries, Box<dyn Error + Send + Sync>> {
    if series.data.is_empty() {
        return Err(format!("No market data returned for {}", req.ticker).into());
    }

    let data_len = series.data.len();

    // Standardized indicator calculations
    let mut builder = series.enhance_data();

    if let Some(period) = req
        .sma_period
        .filter(|period| *period > 0 && *period <= data_len)
    {
        builder = builder.with_sma(period);
    }
    if let Some(period) = req.ema_period {
        builder = builder.with_ema(period);
    }
    if let Some(period) = req.rsi_period {
        builder = builder.with_rsi(period);
    }
    if let Some((fast, slow, signal)) = req.macd_params {
        builder = builder.with_macd(fast, slow, signal);
    }

    Ok(builder.calculate())
}

#[cfg(test)]
mod tests {
    use super::*;
    use market_data::Series;

    #[test]
    fn empty_series_returns_an_error_before_indicator_calculation() {
        let request = TechnicalAnalysisRequest {
            ticker: "TEST.SA".to_string(),
            ..TechnicalAnalysisRequest::default()
        };
        let series = MarketSeries {
            symbol: request.ticker.clone(),
            interval: Interval::Daily,
            data: Vec::new(),
        };

        let error = enhance_series(series, &request).unwrap_err();

        assert!(error.to_string().contains("No market data returned"));
    }

    #[test]
    fn short_series_skips_sma_calculation() {
        let request = TechnicalAnalysisRequest {
            ticker: "TEST.SA".to_string(),
            ..TechnicalAnalysisRequest::default()
        };
        let series = MarketSeries {
            symbol: request.ticker.clone(),
            interval: Interval::Daily,
            data: (0..3)
                .map(|day| Series {
                    datetime: chrono::NaiveDate::from_ymd_opt(2024, 1, day + 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap(),
                    open: 1.0,
                    close: 1.0,
                    high: 1.0,
                    low: 1.0,
                    volume: 1.0,
                })
                .collect(),
        };

        let enhanced = enhance_series(series, &request).unwrap();

        assert_eq!(enhanced.series.len(), 3);
        assert!(!enhanced.indicators.sma.contains_key("SMA 14"));
    }
}
