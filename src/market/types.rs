use market_data::{EnhancedMarketSeries, Interval};

/// Configurable request for technical indicators and range
#[derive(Debug, Clone)]
pub struct TechnicalAnalysisRequest {
    pub ticker: String,
    pub num_samples: usize,
    pub interval: Interval,
    pub sma_period: Option<usize>,
    pub ema_period: Option<usize>,
    pub rsi_period: Option<usize>,
    pub macd_params: Option<(usize, usize, usize)>, // (fast, slow, signal)
}

impl Default for TechnicalAnalysisRequest {
    fn default() -> Self {
        Self {
            ticker: String::new(),
            num_samples: 100,
            interval: Interval::Daily,
            sma_period: Some(14),
            ema_period: Some(20),
            rsi_period: Some(14),
            macd_params: Some((12, 26, 9)),
        }
    }
}

use std::sync::Arc;

pub enum Action {
    FetchHistory(TechnicalAnalysisRequest),
    HistoryLoaded {
        ticker: String,
        result: Result<Arc<EnhancedMarketSeries>, String>,
    },
}

#[derive(Clone, Debug)]
pub enum HistoryState {
    Loading,
    Loaded(Arc<EnhancedMarketSeries>),
    Error(String),
}
