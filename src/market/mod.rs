pub mod client;
pub mod publisher;
pub mod ta_view;
pub mod types;
pub mod view;

use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use tokio_util::sync::CancellationToken;

pub use client::MarketDataClient;
pub use publisher::{MultiPublisherClient, PublisherConfig};
pub use types::{Action, HistoryState, TechnicalAnalysisRequest};

use crate::core::FetchState;
use market_data::EnhancedMarketSeries;

pub static MARKET_DATA: OnceLock<Mutex<MarketData>> = OnceLock::new();
static MARKET_DATA_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    MARKET_DATA_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create MarketData runtime")
        })
        .spawn(future);
}

pub struct MarketData {
    pub history: FetchState<Arc<EnhancedMarketSeries>>,
    pub active_ticker: Option<String>,

    pub action_tx: tokio::sync::mpsc::UnboundedSender<Action>,
    pub action_rx: tokio::sync::mpsc::UnboundedReceiver<Action>,
    pub fetcher: MarketDataFetcher,
}

impl MarketData {
    pub fn new(twelvedata_key: Option<String>, finnhub_key: Option<String>) -> Self {
        let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
        let publisher_client = MultiPublisherClient::default_with_keys(twelvedata_key, finnhub_key);
        let client = MarketDataClient::new(publisher_client);

        Self {
            history: FetchState::Idle,
            active_ticker: None,
            fetcher: MarketDataFetcher::new(client, action_tx.clone()),
            action_tx,
            action_rx,
        }
    }

    /// Synchronous UI request entry point with cache fast-pathing
    pub fn fetch_history(req: TechnicalAnalysisRequest) {
        let Some(data) = MARKET_DATA.get() else {
            return;
        };

        let Ok(mut data) = data.lock() else {
            return;
        };

        // Cache fast-path check
        if let Some(cached_series) = data.fetcher.client.get_cached(&req) {
            data.active_ticker = Some(req.ticker);
            data.history = FetchState::Loaded(cached_series);
            return;
        }

        let request_is_current = data.active_ticker.as_deref() == Some(&req.ticker);
        if request_is_current && matches!(data.history, FetchState::Loading) {
            return;
        }

        data.active_ticker = Some(req.ticker.clone());
        data.history = FetchState::Loading;
        let _ = data.action_tx.send(Action::FetchHistory(req));
    }

    pub fn history(ticker: &str) -> Option<HistoryState> {
        let data = MARKET_DATA.get()?.lock().ok()?;
        if data.active_ticker.as_deref() != Some(ticker) {
            return None;
        }

        match &data.history {
            FetchState::Loading => Some(HistoryState::Loading),
            FetchState::Loaded(series) => Some(HistoryState::Loaded(series.clone())),
            FetchState::Error(err) => Some(HistoryState::Error(err.clone())),
            FetchState::Idle => None,
        }
    }

    pub fn update() {
        let Some(data) = MARKET_DATA.get() else {
            return;
        };

        let Ok(mut data) = data.lock() else {
            return;
        };

        while let Ok(action) = data.action_rx.try_recv() {
            match action {
                action @ Action::FetchHistory(_) => {
                    data.fetcher.handle_action(action);
                }

                Action::HistoryLoaded { ticker, result } => {
                    if data.active_ticker.as_deref() == Some(&ticker) {
                        data.history = match result {
                            Ok(series) => FetchState::Loaded(series),
                            Err(err) => FetchState::Error(err),
                        };
                    }
                }
            }
        }
    }
}

pub struct MarketDataFetcher {
    pub client: MarketDataClient,
    pub task_cancel: Option<CancellationToken>,
    pub action_tx: tokio::sync::mpsc::UnboundedSender<Action>,
}

impl MarketDataFetcher {
    pub fn new(
        client: MarketDataClient,
        action_tx: tokio::sync::mpsc::UnboundedSender<Action>,
    ) -> Self {
        Self {
            action_tx,
            client,
            task_cancel: None,
        }
    }

    pub fn handle_action(&mut self, action: Action) {
        if let Action::FetchHistory(req) = action {
            if let Some(token) = self.task_cancel.take() {
                token.cancel();
            }

            let token = CancellationToken::new();
            self.task_cancel = Some(token.clone());

            let tx = self.action_tx.clone();
            let client = self.client.clone();
            let ticker = req.ticker.clone();

            spawn_task(async move {
                tokio::select! {
                    _ = token.cancelled() => {}
                    res = client.fetch_enhanced_series(&req) => {
                        let result = res.map_err(|e| e.to_string());
                        let _ = tx.send(Action::HistoryLoaded { ticker, result });
                    }
                }
            });
        }
    }
}
