// fetcher.rs
use encoding_rs::WINDOWS_1252;
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::error::Error;
use std::future::Future;
use std::sync::OnceLock;
use tokio_util::sync::CancellationToken;

use crate::core::FetchState;
use crate::fundamentus::StockDetails;

pub static FUNDAMENTUS: OnceLock<std::sync::Mutex<Fundamentus>> = OnceLock::new();
static FUNDAMENTUS_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    FUNDAMENTUS_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Fundamentus runtime")
        })
        .spawn(future);
}

pub enum Action {
    // Async triggers
    FetchTickerList,
    FetchStockDetails(String),
    FetchAllStockDetails(Vec<String>),
    CancelAllStockDetails,

    // Async completion results
    TickerListLoaded(Result<Vec<String>, String>),
    StockDetailsLoaded {
        ticker: String,
        result: Result<StockDetails, String>,
    },
    AllStockDetailsLoaded {
        ticker: String,
        completed: usize,
        total: usize,
        result: Result<StockDetails, String>,
    },
    AllStockDetailsFinished {
        total: usize,
        completed: usize,
        cancelled: bool,
    },
}

pub enum BulkStockDetailsUpdate {
    Stock {
        ticker: String,
        completed: usize,
        total: usize,
        result: Result<StockDetails, String>,
    },
    Finished {
        total: usize,
        completed: usize,
        cancelled: bool,
    },
}

pub enum StockDetailsState {
    Loading,
    Loaded(StockDetails),
    Error(String),
}

pub struct Fundamentus {
    // State machines for async panel data
    pub stock_details: FetchState<StockDetails>,
    pub stock_details_ticker: Option<String>,
    pub ticker_list: FetchState<Vec<String>>,
    pub bulk_stock_details_running: bool,
    pub bulk_stock_details_updates: VecDeque<BulkStockDetailsUpdate>,

    pub action_tx: tokio::sync::mpsc::UnboundedSender<Action>,
    pub action_rx: tokio::sync::mpsc::UnboundedReceiver<Action>,
    pub fetcher: FundamentusFetcher,
}

impl Fundamentus {
    pub fn new() -> Self {
        let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
        Self {
            stock_details: FetchState::Idle,
            stock_details_ticker: None,
            ticker_list: FetchState::Idle,
            bulk_stock_details_running: false,
            bulk_stock_details_updates: VecDeque::new(),
            fetcher: FundamentusFetcher::new(action_tx.clone()),
            action_tx,
            action_rx,
        }
    }

    pub fn tickers_list() -> Vec<String> {
        let Some(data) = FUNDAMENTUS.get() else {
            return Vec::new();
        };

        let Ok(data) = data.lock() else {
            return Vec::new();
        };

        if let FetchState::Loaded(ref tickers) = data.ticker_list {
            return tickers.clone();
        } else if let FetchState::Idle = data.ticker_list {
            drop(data);
            Self::fetch_tickers();
        }

        Vec::new()
    }

    pub fn ticker_list_state() -> Result<Option<Vec<String>>, String> {
        let data = FUNDAMENTUS
            .get()
            .ok_or_else(|| "Fundamentus is not initialized".to_string())?
            .lock()
            .map_err(|_| "Fundamentus state is unavailable".to_string())?;

        match &data.ticker_list {
            FetchState::Loaded(tickers) => Ok(Some(tickers.clone())),
            FetchState::Error(error) => Err(error.clone()),
            FetchState::Idle | FetchState::Loading => Ok(None),
        }
    }

    pub fn fetch_tickers() {
        let Some(data) = FUNDAMENTUS.get() else {
            return;
        };

        let Ok(mut data) = data.lock() else {
            return;
        };

        if matches!(data.ticker_list, FetchState::Idle)
            || matches!(data.ticker_list, FetchState::Error(_))
        {
            data.ticker_list = FetchState::Loading;
            data.action_tx
                .send(Action::FetchTickerList)
                .expect("Failed to send FetchTickerList action");
        }
    }

    pub fn fetch_stock_details(ticker: String) {
        let Some(data) = FUNDAMENTUS.get() else {
            return;
        };

        let Ok(mut data) = data.lock() else {
            return;
        };

        let request_is_current = data.stock_details_ticker.as_deref() == Some(&ticker);
        if request_is_current
            && (matches!(data.stock_details, FetchState::Loading)
                || matches!(data.stock_details, FetchState::Loaded(_)))
        {
            return;
        }

        data.stock_details_ticker = Some(ticker.clone());
        data.stock_details = FetchState::Loading;
        let _ = data.action_tx.send(Action::FetchStockDetails(ticker));
    }

    pub fn stock_details(ticker: &str) -> Option<StockDetailsState> {
        let data = FUNDAMENTUS.get()?.lock().ok()?;
        if data.stock_details_ticker.as_deref() != Some(ticker) {
            return None;
        }

        match &data.stock_details {
            FetchState::Loading => Some(StockDetailsState::Loading),
            FetchState::Loaded(details) => Some(StockDetailsState::Loaded(details.clone())),
            FetchState::Error(error) => Some(StockDetailsState::Error(error.clone())),
            FetchState::Idle => None,
        }
    }

    pub fn fetch_all_stock_details(tickers: Vec<String>) -> Result<(), String> {
        let data = FUNDAMENTUS
            .get()
            .ok_or_else(|| "Fundamentus is not initialized".to_string())?;
        let mut data = data
            .lock()
            .map_err(|_| "Fundamentus state is unavailable".to_string())?;
        if data.bulk_stock_details_running {
            return Err("A bulk stock-detail load is already running".to_string());
        }
        if tickers.is_empty() {
            return Err("No stock tickers are available to load".to_string());
        }
        data.bulk_stock_details_updates.clear();
        data.bulk_stock_details_running = true;
        if data
            .action_tx
            .send(Action::FetchAllStockDetails(tickers))
            .is_err()
        {
            data.bulk_stock_details_running = false;
            return Err("Could not start the bulk stock-detail load".to_string());
        }
        Ok(())
    }

    pub fn cancel_all_stock_details() -> Result<(), String> {
        let data = FUNDAMENTUS
            .get()
            .ok_or_else(|| "Fundamentus is not initialized".to_string())?;
        let data = data
            .lock()
            .map_err(|_| "Fundamentus state is unavailable".to_string())?;
        if !data.bulk_stock_details_running {
            return Err("No bulk stock-detail load is running".to_string());
        }
        data.action_tx
            .send(Action::CancelAllStockDetails)
            .map_err(|_| "Could not cancel the bulk stock-detail load".to_string())
    }

    pub fn take_bulk_stock_details_updates() -> Result<Vec<BulkStockDetailsUpdate>, String> {
        let data = FUNDAMENTUS
            .get()
            .ok_or_else(|| "Fundamentus is not initialized".to_string())?;
        let mut data = data
            .lock()
            .map_err(|_| "Fundamentus state is unavailable".to_string())?;
        Ok(data.bulk_stock_details_updates.drain(..).collect())
    }

    pub fn update() {
        let Some(data) = FUNDAMENTUS.get() else {
            return;
        };

        let Ok(mut data) = data.lock() else {
            return;
        };

        while let Ok(action) = data.action_rx.try_recv() {
            match action {
                action @ Action::FetchTickerList
                | action @ Action::FetchStockDetails(_)
                | action @ Action::FetchAllStockDetails(_)
                | action @ Action::CancelAllStockDetails => {
                    data.fetcher.handle_action(action);
                }

                Action::StockDetailsLoaded { ticker, result } => {
                    if data.stock_details_ticker.as_deref() == Some(&ticker) {
                        data.stock_details = match result {
                            Ok(details) => FetchState::Loaded(details),
                            Err(err) => FetchState::Error(err),
                        };
                    }
                }

                Action::TickerListLoaded(Ok(list)) => {
                    data.ticker_list = FetchState::Loaded(list);
                }

                Action::TickerListLoaded(Err(err)) => {
                    data.ticker_list = FetchState::Error(err);
                }

                Action::AllStockDetailsLoaded {
                    ticker,
                    completed,
                    total,
                    result,
                } => data
                    .bulk_stock_details_updates
                    .push_back(BulkStockDetailsUpdate::Stock {
                        ticker,
                        completed,
                        total,
                        result,
                    }),

                Action::AllStockDetailsFinished {
                    total,
                    completed,
                    cancelled,
                } => {
                    data.bulk_stock_details_running = false;
                    data.bulk_stock_details_updates
                        .push_back(BulkStockDetailsUpdate::Finished {
                            total,
                            completed,
                            cancelled,
                        });
                }
            }
        }
    }
}

pub(crate) struct FundamentusFetcher {
    pub client: FundamentusClient,
    pub task_cancel: Option<CancellationToken>,
    pub bulk_task_cancel: Option<CancellationToken>,
    pub action_tx: tokio::sync::mpsc::UnboundedSender<Action>,
}

impl FundamentusFetcher {
    pub fn new(action_tx: tokio::sync::mpsc::UnboundedSender<Action>) -> Self {
        Self {
            action_tx,
            client: FundamentusClient::new(),
            task_cancel: None,
            bulk_task_cancel: None,
        }
    }

    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::FetchTickerList => {
                let tx = self.action_tx.clone();
                let client = self.client.clone();

                spawn_task(async move {
                    let result = client.list_tickers().await.map_err(|e| e.to_string());
                    let _ = tx.send(Action::TickerListLoaded(result));
                });
            }

            Action::FetchStockDetails(ticker) => {
                if let Some(token) = self.task_cancel.take() {
                    token.cancel();
                }

                let token = CancellationToken::new();
                self.task_cancel = Some(token.clone());

                let tx = self.action_tx.clone();
                let client = self.client.clone();
                let ticker = ticker.clone();

                spawn_task(async move {
                    tokio::select! {
                        _ = token.cancelled() => {}
                        res = client.fetch_stock(&ticker) => {
                            let result = res.map_err(|e| e.to_string());
                            let _ = tx.send(Action::StockDetailsLoaded { ticker, result });
                        }
                    }
                });
            }

            Action::FetchAllStockDetails(tickers) => {
                if let Some(token) = self.bulk_task_cancel.take() {
                    token.cancel();
                }
                let token = CancellationToken::new();
                self.bulk_task_cancel = Some(token.clone());
                let tx = self.action_tx.clone();
                let client = self.client.clone();

                spawn_task(async move {
                    let total = tickers.len();
                    let mut completed = 0;
                    let mut cancelled = false;
                    for ticker in tickers {
                        let result = tokio::select! {
                            _ = token.cancelled() => {
                                cancelled = true;
                                break;
                            }
                            result = client.fetch_stock(&ticker) => {
                                result.map_err(|error| error.to_string())
                            }
                        };
                        completed += 1;
                        if tx
                            .send(Action::AllStockDetailsLoaded {
                                ticker,
                                completed,
                                total,
                                result,
                            })
                            .is_err()
                        {
                            return;
                        }
                        if completed < total {
                            tokio::select! {
                                _ = token.cancelled() => {
                                    cancelled = true;
                                    break;
                                }
                                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
                            }
                        }
                    }
                    let _ = tx.send(Action::AllStockDetailsFinished {
                        total,
                        completed,
                        cancelled,
                    });
                });
            }

            Action::CancelAllStockDetails => {
                if let Some(token) = &self.bulk_task_cancel {
                    token.cancel();
                }
            }

            _ => {}
        }
    }
}

fn parse_market_tickers(html: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("table tbody tr td a[href^='/acoes/']")
        .expect("static stock-list selector is valid");
    let mut tickers = document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .filter_map(|href| href.strip_prefix("/acoes/"))
        .filter(|ticker| {
            !ticker.is_empty()
                && ticker
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .map(str::to_ascii_uppercase)
        .collect::<Vec<_>>();
    tickers.sort();
    tickers.dedup();

    if tickers.is_empty() {
        return Err(std::io::Error::other(
            "Dados de Mercado stock list did not contain any ticker rows",
        )
        .into());
    }

    Ok(tickers)
}

#[derive(Clone)]
pub(crate) struct FundamentusClient {
    client: reqwest::Client,
}

impl FundamentusClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Returns the stock tickers published in Dados de Mercado's B3 stocks list.
    pub async fn list_tickers(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let response = self
            .client
            .get("https://www.dadosdemercado.com.br/acoes")
            .header(
                USER_AGENT,
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send()
            .await?
            .error_for_status()?;

        parse_market_tickers(&response.text().await?)
    }

    pub async fn fetch_stock(&self, ticker: &str) -> Result<StockDetails, Box<dyn Error>> {
        let url = format!(
            "https://www.fundamentus.com.br/detalhes.php?papel={}",
            ticker.to_uppercase()
        );

        // Fundamentus requires a browser-like User-Agent header
        let response = self
            .client
            .get(&url)
            .header(
                USER_AGENT,
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send()
            .await?
            .error_for_status()?;

        let bytes = response.bytes().await?;
        // Fundamentus uses Windows-1252 / ISO-8859-1 encoding
        let (html_content, _, _) = WINDOWS_1252.decode(&bytes);

        let data = Self::parse_html(&html_content);
        Ok(Self::map_to_stock_details(ticker, data))
    }

    fn parse_html(html: &str) -> HashMap<String, String> {
        let document = Html::parse_document(html);
        let tr_selector = Selector::parse("tr").unwrap();
        let td_selector = Selector::parse("td").unwrap();
        let mut map = HashMap::new();

        for tr in document.select(&tr_selector) {
            let tds: Vec<_> = tr.select(&td_selector).collect();
            let mut i = 0;
            // Parse adjacent td pairs (label -> value)
            while i + 1 < tds.len() {
                let label = tds[i]
                    .text()
                    .collect::<String>()
                    .trim()
                    .replace('?', "")
                    .to_string();
                let value = tds[i + 1].text().collect::<String>().trim().to_string();

                if !label.is_empty() {
                    map.insert(label, value);
                }
                i += 2;
            }
        }
        map
    }

    fn map_to_stock_details(ticker: &str, map: HashMap<String, String>) -> StockDetails {
        let get = |k: &str| map.get(k).cloned().unwrap_or_else(|| "-".to_string());

        StockDetails {
            ticker: ticker.to_uppercase(),
            nome: get("Empresa"),
            cotacao: get("Cotação"),
            data_cot: get("Data últ cot"),
            tipo: get("Tipo"),
            setor: get("Setor"),
            subsetor: get("Subsetor"),
            min_52sem: get("Min 52 sem"),
            max_52sem: get("Max 52 sem"),
            vol_medio: get("Vol $ médio (2m)"),
            val_mercado: get("Valor de mercado"),
            val_firma: get("Valor da firma"),
            nro_acoes: get("Nro. Ações"),
            p_l: get("P/L"),
            p_vp: get("P/VP"),
            psr: get("PSR"),
            div_yield: get("Div. Yield"),
            roe: get("ROE"),
            roic: get("ROIC"),
            marg_bruta: get("Marg. Bruta"),
            marg_ebit: get("Marg. EBIT"),
            marg_liquida: get("Marg. Líquida"),
            ev_ebitda: get("EV / EBITDA"),
            lpa: get("LPA"),
            vpa: get("VPA"),
            ativo: get("Ativo"),
            disponibilidades: get("Disponibilidades"),
            div_bruta: get("Dív. Bruta"),
            div_liquida: get("Dív. Líquida"),
            patrim_liquido: get("Patrim. Líq"),
            rec_liq_12m: get("Receita Líquida"),
            lucro_liq_12m: get("Lucro Líquido"),
            cres_rec_5a: get("Cres. Rec (5a)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FundamentusClient, parse_market_tickers};

    #[test]
    fn parses_and_deduplicates_tickers_from_market_list_rows() {
        let html = r#"
            <table>
                <tbody>
                    <tr><td><a href="/acoes/petr4">PETR4</a></td></tr>
                    <tr><td><a href="/acoes/bbdc4">BBDC4</a></td></tr>
                    <tr><td><a href="/acoes/petr4">PETR4</a></td></tr>
                </tbody>
            </table>
            <a href="/acoes/not-a-stock">Outside the stock table</a>
        "#;

        assert_eq!(
            parse_market_tickers(html).unwrap(),
            vec!["BBDC4".to_string(), "PETR4".to_string()]
        );
    }

    #[test]
    fn errors_when_market_list_has_no_tickers() {
        assert!(parse_market_tickers("<table><tbody></tbody></table>").is_err());
    }

    #[test]
    fn maps_fundamentus_five_year_revenue_growth() {
        let html = r#"
            <table>
                <tr><td>Cres. Rec (5a)</td><td>-2,3%</td></tr>
            </table>
        "#;
        let details =
            FundamentusClient::map_to_stock_details("PETR4", FundamentusClient::parse_html(html));

        assert_eq!(details.cres_rec_5a, "-2,3%");
    }
}
