use market_data::EnhancedMarketSeries;
use moka::sync::Cache;
use std::{error::Error, sync::Arc, time::Duration};

use crate::market::{publisher::MultiPublisherClient, types::TechnicalAnalysisRequest};

fn generate_cache_key(req: &TechnicalAnalysisRequest) -> String {
    format!(
        "{}:{}:{}:sma_{:?}:ema_{:?}:rsi_{:?}:macd_{:?}",
        req.ticker.to_uppercase(),
        req.num_samples,
        req.interval,
        req.sma_period,
        req.ema_period,
        req.rsi_period,
        req.macd_params
    )
}

#[derive(Clone)]
pub struct MarketDataClient {
    pub publisher_client: MultiPublisherClient,
    cache: Cache<String, Arc<EnhancedMarketSeries>>,
}

impl MarketDataClient {
    pub fn new(publisher_client: MultiPublisherClient) -> Self {
        let cache = Cache::builder()
            .max_capacity(500)
            .time_to_live(Duration::from_secs(600)) // 10 minutes TTL
            .build();

        Self {
            publisher_client,
            cache,
        }
    }

    /// Synchronous lookup for UI zero-latency checks
    pub fn get_cached(&self, req: &TechnicalAnalysisRequest) -> Option<Arc<EnhancedMarketSeries>> {
        let key = generate_cache_key(req);
        self.cache.get(&key)
    }

    pub async fn fetch_enhanced_series(
        &self,
        req: &TechnicalAnalysisRequest,
    ) -> Result<Arc<EnhancedMarketSeries>, Box<dyn Error + Send + Sync>> {
        let key = generate_cache_key(req);

        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached);
        }

        let series = self.publisher_client.fetch_enhanced_series(req).await?;
        let shared = Arc::new(series);
        self.cache.insert(key, shared.clone());

        Ok(shared)
    }
}
