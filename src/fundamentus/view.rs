use crate::{
    fundamentus::{StockDetails, StockSummary, fetcher, fetcher::StockDetailsState},
    scoring::{self, Category, QarpStrategy, ScoringReport, StockEvaluation, StockFundamentals},
    widgets::{InteractableWidget, input_box::InputBox, list::GenericList},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use ratatui_interact::components::{Spinner, SpinnerFrames, SpinnerState, SpinnerStyle};

enum DetailsContent {
    Loading,
    Loaded {
        details: StockDetails,
        score: Result<StockScore, String>,
    },
    Error(String),
}

struct StockScore {
    evaluation: StockEvaluation,
    eligible_count: usize,
    warnings: Vec<String>,
}

struct DetailsWindow {
    ticker: String,
    content: DetailsContent,
    spinner_state: SpinnerState,
}

pub struct FundamentusView {
    pub stocks_list: GenericList<StockSummary>,
    stock_summaries: Vec<StockSummary>,
    detail_snapshots: std::collections::BTreeMap<String, StockDetails>,
    ticker_filter: InputBox,
    loaded: bool,
    score_sorted: bool,
    sector_data_loaded: bool,
    sector_by_ticker: std::collections::BTreeMap<String, String>,
    sectors: Vec<String>,
    selected_sector_index: usize,
    sector_panel_focused: bool,
    status_message: String,
    bulk_load: Option<BulkLoadStatus>,
    market_tickers: Option<Vec<String>>,
    market_list_error: Option<String>,
    b3_listing_data_loaded: bool,
    b3_snapshot_available: bool,
    b3_listing_snapshot_current: bool,
    b3_listing_load_error: Option<String>,
    b3_listed_tickers: std::collections::BTreeSet<String>,
    b3_only_filter_enabled: bool,
    spinner_state: SpinnerState,
    details_window: Option<DetailsWindow>,
}

struct BulkLoadStatus {
    total: usize,
    completed: usize,
    saved: usize,
    failed: usize,
    failed_tickers: Vec<String>,
    running: bool,
}

impl FundamentusView {
    pub fn new() -> Self {
        let stocks_list = crate::fundamentus::stocks_list::create_stocks_summaries_list(Vec::new());
        let mut ticker_filter = InputBox::new("Ticker");
        ticker_filter.state.focused = true;
        fetcher::Fundamentus::fetch_tickers();
        Self {
            stocks_list,
            stock_summaries: Vec::new(),
            detail_snapshots: std::collections::BTreeMap::new(),
            ticker_filter,
            loaded: false,
            score_sorted: false,
            sector_data_loaded: false,
            sector_by_ticker: std::collections::BTreeMap::new(),
            sectors: Vec::new(),
            selected_sector_index: 0,
            sector_panel_focused: false,
            status_message: String::new(),
            bulk_load: None,
            market_tickers: None,
            market_list_error: None,
            b3_listing_data_loaded: false,
            b3_snapshot_available: false,
            b3_listing_snapshot_current: false,
            b3_listing_load_error: None,
            b3_listed_tickers: std::collections::BTreeSet::new(),
            b3_only_filter_enabled: false,
            spinner_state: SpinnerState::for_frames(SpinnerFrames::Dots),
            details_window: None,
        }
    }

    pub fn update(&mut self) {
        if !self.sector_data_loaded {
            self.load_saved_sectors();
        }
        if !self.b3_listing_data_loaded {
            self.load_b3_listing_snapshot();
        }

        if self.market_tickers.is_none() && self.market_list_error.is_none() {
            match fetcher::Fundamentus::ticker_list_state() {
                Ok(Some(tickers)) => {
                    self.load_detail_snapshots(&tickers);
                    self.stock_summaries =
                        summaries_for_market_tickers(&tickers, &self.detail_snapshots);
                    self.market_tickers = Some(tickers);
                    self.update_stock_filter();
                    self.loaded = true;
                }
                Ok(None) => {
                    self.spinner_state
                        .tick_with_frames(SpinnerFrames::Dots.frames().len());
                }
                Err(error) => {
                    log::error!(target: "midas", "Could not load B3 stock list: {error}");
                    self.market_list_error = Some(error.clone());
                    self.status_message = format!("Could not load B3 stock list: {error}");
                    self.loaded = true;
                }
            }
        }

        let mut loaded_sector = None;
        let mut loaded_details = None;
        if let Some(window) = &mut self.details_window {
            match fetcher::Fundamentus::stock_details(&window.ticker) {
                Some(StockDetailsState::Loaded(details)) => {
                    if !matches!(window.content, DetailsContent::Loaded { .. }) {
                        if let Err(error) =
                            crate::fundamentus::db::Stocks::write_stock_to_db(&details)
                        {
                            log::error!(target: "midas", "Failed to save stock details for {}: {error}", details.ticker);
                        }
                        if let Some(sector) = sector_name(&details.setor) {
                            loaded_sector = Some((details.ticker.clone(), sector));
                        }
                        loaded_details = Some((window.ticker.clone(), details));
                    }
                }
                Some(StockDetailsState::Error(error)) => {
                    window.content = DetailsContent::Error(error);
                }
                Some(StockDetailsState::Loading) | None => {
                    window
                        .spinner_state
                        .tick_with_frames(SpinnerFrames::Dots.frames().len());
                }
            }
        }
        if let Some((ticker, details)) = loaded_details {
            self.detail_snapshots
                .insert(ticker.trim().to_ascii_uppercase(), details.clone());
            if let Some((sector_ticker, sector)) = &loaded_sector {
                self.sector_by_ticker
                    .insert(sector_ticker.to_ascii_uppercase(), sector.clone());
                self.refresh_sector_options();
            }
            self.refresh_stock_summaries_from_details();
            self.update_stock_filter();
            self.score_visible_stocks();
            let score = score_stock_details_universe(
                &self.stocks_list.rows,
                &self.detail_snapshots,
                &ticker,
            )
            .map_err(|error| error.to_string());
            if let Some(window) = &mut self.details_window {
                window.content = DetailsContent::Loaded { details, score };
            }
        }
        if let Some((ticker, sector)) = loaded_sector {
            self.sector_by_ticker.insert(ticker.to_uppercase(), sector);
            self.refresh_sector_options();
            self.update_stock_filter();
        }

        match fetcher::Fundamentus::take_bulk_stock_details_updates() {
            Ok(updates) => {
                for update in updates {
                    match update {
                        fetcher::BulkStockDetailsUpdate::Stock {
                            ticker,
                            completed,
                            total,
                            result,
                        } => {
                            let (saved, failed_ticker, loaded_sector) = match result {
                                Ok(details) => {
                                    match crate::fundamentus::db::Stocks::write_stock_to_db(
                                        &details,
                                    ) {
                                        Ok(_) => {
                                            self.detail_snapshots.insert(
                                                details.ticker.trim().to_ascii_uppercase(),
                                                details.clone(),
                                            );
                                            (
                                                true,
                                                None,
                                                sector_name(&details.setor)
                                                    .map(|sector| (details.ticker, sector)),
                                            )
                                        }
                                        Err(error) => {
                                            log::error!(target: "midas", "Failed to save stock details for {ticker}: {error}");
                                            (false, Some(ticker), None)
                                        }
                                    }
                                }
                                Err(error) => {
                                    log::error!(target: "midas", "Failed to fetch stock details for {ticker}: {error}");
                                    (false, Some(ticker), None)
                                }
                            };
                            self.refresh_stock_summaries_from_details();
                            self.update_stock_filter();
                            if let Some((ticker, sector)) = loaded_sector {
                                self.sector_by_ticker.insert(ticker.to_uppercase(), sector);
                                self.refresh_sector_options();
                                self.update_stock_filter();
                            }
                            let status = self.bulk_load.get_or_insert(BulkLoadStatus {
                                total,
                                completed: 0,
                                saved: 0,
                                failed: 0,
                                failed_tickers: Vec::new(),
                                running: true,
                            });
                            status.total = total;
                            status.completed = completed;
                            status.saved += usize::from(saved);
                            if let Some(ticker) = failed_ticker {
                                status.failed += 1;
                                status.failed_tickers.push(ticker);
                            }
                            self.status_message = format!(
                                "Loading details {}/{} (saved {}, failed {}) | x cancel",
                                status.completed, status.total, status.saved, status.failed
                            );
                        }
                        fetcher::BulkStockDetailsUpdate::Finished {
                            total,
                            completed,
                            cancelled,
                        } => {
                            let status = self.bulk_load.get_or_insert(BulkLoadStatus {
                                total,
                                completed,
                                saved: 0,
                                failed: 0,
                                failed_tickers: Vec::new(),
                                running: false,
                            });
                            status.total = total;
                            status.completed = completed;
                            status.running = false;
                            let failure_list = if status.failed_tickers.is_empty() {
                                String::new()
                            } else {
                                let sample = status
                                    .failed_tickers
                                    .iter()
                                    .take(3)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let remaining = status.failed_tickers.len().saturating_sub(3);
                                if remaining > 0 {
                                    format!("; failed: {sample} and {remaining} more")
                                } else {
                                    format!("; failed: {sample}")
                                }
                            };
                            self.status_message = format!(
                                "Bulk load {}: {}/{} fetched, {} saved, {} failed{}",
                                if cancelled { "cancelled" } else { "complete" },
                                status.completed,
                                status.total,
                                status.saved,
                                status.failed,
                                failure_list
                            );
                        }
                    }
                }
            }
            Err(error) => {
                log::error!(target: "midas", "Could not read bulk stock-detail updates: {error}");
                self.status_message = format!("Could not read bulk-load status: {error}");
            }
        }
    }

    pub fn close_details_window(&mut self) -> bool {
        self.details_window.take().is_some()
    }

    fn open_selected_stock(&mut self) {
        if self.details_window.is_some()
            || self.bulk_load.as_ref().is_some_and(|status| status.running)
        {
            return;
        }

        let Some(stock) = self.stocks_list.selected_row() else {
            return;
        };
        let ticker = stock.ticker.clone();
        self.score_visible_stocks();
        fetcher::Fundamentus::fetch_stock_details(ticker.clone());
        self.details_window = Some(DetailsWindow {
            ticker,
            content: DetailsContent::Loading,
            spinner_state: SpinnerState::for_frames(SpinnerFrames::Dots),
        });
    }

    fn load_detail_snapshots(&mut self, tickers: &[String]) {
        match crate::fundamentus::db::Stocks::read_all() {
            Ok(stocks) => {
                self.detail_snapshots = stocks
                    .into_iter()
                    .map(|stock| (stock.ticker.trim().to_ascii_uppercase(), stock))
                    .filter(|(ticker, _)| {
                        tickers
                            .iter()
                            .any(|market_ticker| market_ticker.eq_ignore_ascii_case(ticker))
                    })
                    .collect();
            }
            Err(error) => {
                log::error!(target: "midas", "Could not load saved stock details: {error}");
                self.status_message = format!("Could not load saved stock details: {error}");
            }
        }
    }

    fn refresh_stock_summaries_from_details(&mut self) {
        if let Some(tickers) = self.market_tickers.as_deref() {
            self.stock_summaries = summaries_for_market_tickers(tickers, &self.detail_snapshots);
        }
    }

    fn start_bulk_load(&mut self) {
        let tickers = match fetcher::Fundamentus::ticker_list_state() {
            Ok(Some(tickers)) => tickers,
            Ok(None) => {
                fetcher::Fundamentus::fetch_tickers();
                self.status_message =
                    "Available-stock list is loading; press D again when it finishes.".to_string();
                return;
            }
            Err(error) => {
                fetcher::Fundamentus::fetch_tickers();
                self.status_message =
                    format!("Could not load available stocks: {error}; retry started.");
                return;
            }
        };
        let total = tickers.len();
        match fetcher::Fundamentus::fetch_all_stock_details(tickers) {
            Ok(()) => {
                self.bulk_load = Some(BulkLoadStatus {
                    total,
                    completed: 0,
                    saved: 0,
                    failed: 0,
                    failed_tickers: Vec::new(),
                    running: true,
                });
                self.status_message = format!("Loading details for {total} stocks | X cancel");
            }
            Err(error) => self.status_message = format!("Could not start bulk load: {error}"),
        }
    }

    fn cancel_bulk_load(&mut self) {
        match fetcher::Fundamentus::cancel_all_stock_details() {
            Ok(()) => self.status_message = "Cancelling bulk detail load...".to_string(),
            Err(error) => self.status_message = format!("Could not cancel bulk load: {error}"),
        }
    }

    fn update_stock_filter(&mut self) {
        let selected_ticker = self
            .stocks_list
            .selected_row()
            .map(|summary| summary.ticker.clone());
        let previous_index = self.stocks_list.state.selected_index;
        let filter = self.ticker_filter.state.text.to_uppercase();
        let selected_sector = self.selected_sector();
        let b3_listed_tickers = self
            .b3_only_filter_enabled
            .then_some(&self.b3_listed_tickers);
        let summaries = filter_summaries(
            &self.stock_summaries,
            &filter,
            selected_sector,
            &self.sector_by_ticker,
            b3_listed_tickers,
        );
        let summaries = if self.score_sorted {
            sort_summaries_by_score(summaries)
        } else {
            summaries
        };
        let mut stocks_list =
            crate::fundamentus::stocks_list::create_stocks_summaries_list(summaries);
        stocks_list.state.selected_index = restored_selection_index(
            &stocks_list.rows,
            selected_ticker.as_deref(),
            previous_index,
        );
        self.stocks_list = stocks_list;
    }

    fn load_b3_listing_snapshot(&mut self) {
        let result = (|| {
            let db = crate::db::DB
                .get()
                .ok_or_else(|| "Database is not initialized".to_string())?
                .lock()
                .map_err(|_| "Database is unavailable".to_string())?;
            crate::b3_eligibility::load_snapshot(&db.conn).map_err(|error| error.to_string())
        })();
        match result {
            Ok(Some(snapshot)) => {
                self.b3_snapshot_available = true;
                self.b3_listing_snapshot_current = crate::b3_eligibility::snapshot_is_current(
                    &snapshot,
                    crate::b3_eligibility::DEFAULT_MAX_DATA_AGE_DAYS,
                );
                self.b3_listed_tickers = crate::b3_eligibility::listed_tickers(
                    &snapshot,
                    crate::b3_eligibility::DEFAULT_MAX_DATA_AGE_DAYS,
                );
            }
            Ok(None) => {}
            Err(error) => {
                log::error!(target: "midas", "Could not load B3 listing snapshot: {error}");
                self.b3_listing_load_error = Some(error);
            }
        }
        self.b3_listing_data_loaded = true;
    }

    fn toggle_b3_only_filter(&mut self) {
        self.b3_only_filter_enabled = !self.b3_only_filter_enabled;
        self.update_stock_filter();
        if !self.b3_only_filter_enabled {
            self.status_message = "B3 listing filter disabled.".to_string();
        } else if self.b3_listing_snapshot_current {
            self.status_message = format!(
                "B3 listed-only filter enabled using fresh B3 evidence; {} stocks remain.",
                self.stocks_list.rows.len()
            );
        } else if let Some(error) = &self.b3_listing_load_error {
            self.status_message = format!("Could not load B3 listing snapshot: {error}");
        } else if !self.b3_snapshot_available || !self.b3_listing_snapshot_current {
            self.status_message =
                "No fresh B3 listing evidence; B3-only filter is empty. The stock list still shows the Dados de Mercado B3 universe.".to_string();
        } else {
            self.status_message = format!(
                "B3 listed-only filter enabled; {} stocks remain.",
                self.stocks_list.rows.len()
            );
        }
    }

    fn load_saved_sectors(&mut self) {
        match crate::fundamentus::db::Stocks::read_all() {
            Ok(stocks) => {
                self.sector_by_ticker = stocks
                    .iter()
                    .filter_map(|stock| {
                        sector_name(&stock.setor)
                            .map(|sector| (stock.ticker.to_uppercase(), sector))
                    })
                    .collect();
                self.refresh_sector_options();
                self.sector_data_loaded = true;
            }
            Err(error) => {
                self.status_message = format!("Could not load saved sectors: {error}");
                self.sector_data_loaded = true;
            }
        }
    }

    fn refresh_sector_options(&mut self) {
        let selected = self.selected_sector().map(str::to_string);
        self.sectors = self
            .sector_by_ticker
            .values()
            .map(String::as_str)
            .filter_map(sector_name)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        self.selected_sector_index = selected
            .and_then(|selected| {
                self.sectors
                    .iter()
                    .position(|sector| sector.eq_ignore_ascii_case(&selected))
                    .map(|index| index + 1)
            })
            .unwrap_or(0);
    }

    fn selected_sector(&self) -> Option<&str> {
        self.selected_sector_index
            .checked_sub(1)
            .and_then(|index| self.sectors.get(index))
            .map(String::as_str)
    }

    fn move_sector_selection(&mut self, down: bool) {
        let option_count = self.sectors.len() + 1;
        if down {
            self.selected_sector_index = (self.selected_sector_index + 1) % option_count;
        } else {
            self.selected_sector_index = self
                .selected_sector_index
                .checked_sub(1)
                .unwrap_or(option_count - 1);
        }
        self.update_stock_filter();
    }

    fn score_visible_stocks(&mut self) {
        for summary in &mut self.stock_summaries {
            summary.qarp_score = None;
            summary.qarp_rank = None;
            summary.qarp_eligible = None;
        }
        let details = visible_detail_snapshots(&self.stocks_list.rows, &self.detail_snapshots);
        if details.is_empty() {
            self.status_message =
                "No visible detailed snapshots to score. Press D to load details.".to_string();
            return;
        }
        let stocks = details
            .iter()
            .map(StockFundamentals::from)
            .collect::<Vec<_>>();

        match scoring::run_strategy(&QarpStrategy::default(), &stocks) {
            Ok(report) => {
                let results = report
                    .eligible
                    .iter()
                    .chain(&report.ineligible)
                    .map(|evaluation| {
                        (
                            evaluation.ticker.to_uppercase(),
                            (
                                evaluation.score.as_ref().map(|score| score.total_score),
                                evaluation.rank,
                                evaluation.eligibility.eligible,
                            ),
                        )
                    })
                    .collect::<std::collections::BTreeMap<_, _>>();
                for summary in &mut self.stock_summaries {
                    if let Some((score, rank, eligible)) =
                        results.get(&summary.ticker.to_uppercase())
                    {
                        summary.qarp_score = *score;
                        summary.qarp_rank = *rank;
                        summary.qarp_eligible = Some(*eligible);
                    }
                }
                self.status_message = format!(
                    "Scored {} visible detailed snapshots: {} eligible, {} ineligible.",
                    stocks.len(),
                    report.eligible.len(),
                    report.ineligible.len()
                );
                self.update_stock_filter();
            }
            Err(error) => {
                self.status_message = format!("Could not score stock list: {error}");
            }
        }
    }

    fn sort_visible_stocks_by_score(&mut self) {
        if self.stocks_list.rows.is_empty() {
            self.status_message = "No visible stocks to sort.".to_string();
            return;
        }
        if !self
            .stocks_list
            .rows
            .iter()
            .any(|summary| summary.qarp_eligible.is_some())
        {
            self.status_message = "No visible detailed snapshots are scored. Press S.".to_string();
            return;
        }
        self.score_sorted = true;
        self.update_stock_filter();
        self.status_message =
            "Sorted by QARP rank (1 first); ineligible stocks follow.".to_string();
    }
}

fn sector_name(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "-").then(|| value.to_string())
}

fn summaries_for_market_tickers(
    tickers: &[String],
    snapshots: &std::collections::BTreeMap<String, StockDetails>,
) -> Vec<StockSummary> {
    tickers
        .iter()
        .map(|ticker| {
            let ticker = ticker.trim().to_ascii_uppercase();
            snapshots
                .get(&ticker)
                .map(stock_summary_from_details)
                .unwrap_or_else(|| empty_stock_summary(ticker))
        })
        .collect()
}

fn stock_summary_from_details(details: &StockDetails) -> StockSummary {
    StockSummary {
        ticker: details.ticker.trim().to_ascii_uppercase(),
        cotacao: details.cotacao.clone(),
        p_l: details.p_l.clone(),
        p_vp: details.p_vp.clone(),
        div_yield: details.div_yield.clone(),
        roe: details.roe.clone(),
        qarp_score: None,
        qarp_rank: None,
        qarp_eligible: None,
    }
}

fn empty_stock_summary(ticker: String) -> StockSummary {
    StockSummary {
        ticker,
        cotacao: "-".to_string(),
        p_l: "-".to_string(),
        p_vp: "-".to_string(),
        div_yield: "-".to_string(),
        roe: "-".to_string(),
        qarp_score: None,
        qarp_rank: None,
        qarp_eligible: None,
    }
}

fn visible_detail_snapshots(
    summaries: &[StockSummary],
    snapshots: &std::collections::BTreeMap<String, StockDetails>,
) -> Vec<StockDetails> {
    summaries
        .iter()
        .filter_map(|summary| snapshots.get(&summary.ticker.trim().to_ascii_uppercase()))
        .cloned()
        .collect()
}

fn filter_summaries(
    summaries: &[StockSummary],
    ticker_filter: &str,
    sector_filter: Option<&str>,
    sector_by_ticker: &std::collections::BTreeMap<String, String>,
    b3_listed_tickers: Option<&std::collections::BTreeSet<String>>,
) -> Vec<StockSummary> {
    summaries
        .iter()
        .filter(|summary| summary.ticker.to_uppercase().contains(ticker_filter))
        .filter(|summary| {
            b3_listed_tickers
                .is_none_or(|tickers| tickers.contains(&summary.ticker.trim().to_ascii_uppercase()))
        })
        .filter(|summary| {
            sector_filter.is_none_or(|sector| {
                sector_by_ticker
                    .get(&summary.ticker.to_uppercase())
                    .is_some_and(|stock_sector| stock_sector.eq_ignore_ascii_case(sector))
            })
        })
        .cloned()
        .collect()
}

fn restored_selection_index(
    summaries: &[StockSummary],
    selected_ticker: Option<&str>,
    previous_index: usize,
) -> usize {
    if let Some(selected_ticker) = selected_ticker
        && let Some(index) = summaries
            .iter()
            .position(|summary| summary.ticker.eq_ignore_ascii_case(selected_ticker))
    {
        return index;
    }

    previous_index.min(summaries.len().saturating_sub(1))
}

fn render_sector_panel(
    area: Rect,
    frame: &mut ratatui::Frame,
    sectors: &[String],
    selected_index: usize,
    sector_by_ticker: &std::collections::BTreeMap<String, String>,
    summaries: &[StockSummary],
    focused: bool,
    loaded: bool,
) {
    let mut items = Vec::with_capacity(sectors.len() + 2);
    items.push(ListItem::new(format!(
        "{} ({})",
        if selected_index == 0 {
            "> All sectors"
        } else {
            "  All sectors"
        },
        summaries.len()
    )));
    items.extend(sectors.iter().enumerate().map(|(index, sector)| {
        let count = summaries
            .iter()
            .filter(|summary| {
                sector_by_ticker
                    .get(&summary.ticker.to_uppercase())
                    .is_some_and(|saved_sector| saved_sector.eq_ignore_ascii_case(sector))
            })
            .count();
        let marker = if selected_index == index + 1 {
            "> "
        } else {
            "  "
        };
        ListItem::new(format!("{marker}{sector} ({count})"))
    }));
    if sectors.is_empty() {
        let message = if loaded {
            "Press D to load all sectors"
        } else {
            "Loading saved sectors..."
        };
        items.push(ListItem::new(message));
    }

    let mut list_state = ListState::default().with_selected(Some(selected_index));
    let panel = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                })
                .title(" Sectors (saved details) "),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(panel, area, &mut list_state);
}

fn sort_summaries_by_score(mut summaries: Vec<StockSummary>) -> Vec<StockSummary> {
    summaries.sort_by(|left, right| {
        let eligibility_order = |eligible| match eligible {
            Some(true) => 0,
            Some(false) => 1,
            None => 2,
        };
        eligibility_order(left.qarp_eligible)
            .cmp(&eligibility_order(right.qarp_eligible))
            .then_with(|| left.qarp_rank.cmp(&right.qarp_rank))
            .then_with(|| left.ticker.cmp(&right.ticker))
    });
    summaries
}

impl InteractableWidget for FundamentusView {
    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> crate::budget::ActionResult {
        if self.details_window.is_some() {
            return crate::budget::ActionResult::Consumed;
        }

        if key.code == crossterm::event::KeyCode::Tab
            || key.code == crossterm::event::KeyCode::BackTab
        {
            self.sector_panel_focused = !self.sector_panel_focused;
            self.ticker_filter.state.focused = !self.sector_panel_focused;
            return crate::budget::ActionResult::Consumed;
        }

        if self.sector_panel_focused {
            match key.code {
                crossterm::event::KeyCode::Up => {
                    self.move_sector_selection(false);
                    return crate::budget::ActionResult::Consumed;
                }
                crossterm::event::KeyCode::Down => {
                    self.move_sector_selection(true);
                    return crate::budget::ActionResult::Consumed;
                }
                crossterm::event::KeyCode::Left
                | crossterm::event::KeyCode::Right
                | crossterm::event::KeyCode::Enter => {
                    self.sector_panel_focused = false;
                    self.ticker_filter.state.focused = true;
                    return crate::budget::ActionResult::Consumed;
                }
                _ => {}
            }
        }

        if key.code == crossterm::event::KeyCode::Char('S') {
            self.score_visible_stocks();
            return crate::budget::ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Char('s') {
            self.sort_visible_stocks_by_score();
            return crate::budget::ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Char('B') {
            self.toggle_b3_only_filter();
            return crate::budget::ActionResult::Consumed;
        }

        if self.sector_panel_focused {
            return crate::budget::ActionResult::Consumed;
        }

        if key.code == crossterm::event::KeyCode::Char('D') {
            self.start_bulk_load();
            return crate::budget::ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Char('x')
            && self.bulk_load.as_ref().is_some_and(|status| status.running)
        {
            self.cancel_bulk_load();
            return crate::budget::ActionResult::Consumed;
        }

        if key.code == crossterm::event::KeyCode::Enter {
            self.open_selected_stock();
            return crate::budget::ActionResult::Consumed;
        }

        match self.ticker_filter.handle_key(key) {
            crate::budget::ActionResult::PatternChanged(_) => {
                self.update_stock_filter();
                crate::budget::ActionResult::Consumed
            }
            crate::budget::ActionResult::None => self.stocks_list.handle_key(key),
            result => result,
        }
    }

    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);
        self.ticker_filter.render(chunks[0], frame);

        let [sector_area, stock_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(26), Constraint::Min(0)])
            .areas(chunks[1]);
        render_sector_panel(
            sector_area,
            frame,
            &self.sectors,
            self.selected_sector_index,
            &self.sector_by_ticker,
            &self.stock_summaries,
            self.sector_panel_focused,
            self.sector_data_loaded,
        );

        if self.loaded {
            self.stocks_list.render(stock_area, frame);
        } else {
            let spinner = Spinner::new(&self.spinner_state)
                .style(SpinnerStyle::new(SpinnerFrames::Dots).color(Color::Cyan))
                .label("Loading B3 stock list...");
            let spinner_area = Rect::new(
                stock_area.x,
                stock_area.y + stock_area.height / 2,
                stock_area.width,
                1,
            );
            frame.render_widget(spinner, spinner_area);
        }
        let status = if self.status_message.is_empty() {
            format!(
                "Tab sectors | ↑/↓ select | B B3 listed only {} | D load all details | S score | s sort | Enter details",
                if self.b3_only_filter_enabled {
                    "ON"
                } else {
                    "OFF"
                }
            )
        } else {
            format!(
                "{} | B B3 only {} | D load all | Tab sectors | S score | s sort",
                self.status_message,
                if self.b3_only_filter_enabled {
                    "ON"
                } else {
                    "OFF"
                }
            )
        };
        frame.render_widget(Paragraph::new(status), chunks[2]);

        let Some(window) = &self.details_window else {
            return;
        };

        let popup_width = area.width.saturating_sub(2).min(120);
        let popup_height = area.height.saturating_sub(2).min(38);
        let popup_area = Rect::new(
            area.x + area.width.saturating_sub(popup_width) / 2,
            area.y + area.height.saturating_sub(popup_height) / 2,
            popup_width,
            popup_height,
        );
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} Details ", window.ticker));
        let content_area = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        match &window.content {
            DetailsContent::Loading => {
                let spinner = Spinner::new(&window.spinner_state)
                    .style(SpinnerStyle::new(SpinnerFrames::Dots).color(Color::Cyan))
                    .label("Loading stock details...");
                let spinner_area = Rect::new(
                    content_area.x,
                    content_area.y + content_area.height / 2,
                    content_area.width,
                    1,
                );
                frame.render_widget(spinner, spinner_area);
            }
            DetailsContent::Loaded { details, score } => {
                let [score_area, details_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(5), Constraint::Min(0)])
                    .areas(content_area);
                render_stock_score(score, score_area, frame);
                crate::fundamentus::stock_view::FundamentusWidget::new(details)
                    .render(frame, details_area);
            }

            DetailsContent::Error(error) => {
                let message = Paragraph::new(error.as_str()).alignment(Alignment::Center);
                frame.render_widget(message, content_area);
            }
        }
    }
}

fn score_stock_details_universe(
    summaries: &[StockSummary],
    snapshots: &std::collections::BTreeMap<String, StockDetails>,
    ticker: &str,
) -> Result<StockScore, Box<dyn std::error::Error>> {
    let details = visible_detail_snapshots(summaries, snapshots);
    let stocks = details
        .iter()
        .map(StockFundamentals::from)
        .collect::<Vec<_>>();
    let report = scoring::run_strategy(&QarpStrategy::default(), &stocks)?;
    let evaluation = evaluation_for_ticker(&report, ticker)
        .ok_or_else(|| std::io::Error::other(format!("QARP report did not include {ticker}")))?;
    Ok(StockScore {
        evaluation,
        eligible_count: report.eligible.len(),
        warnings: report.warnings,
    })
}

fn evaluation_for_ticker(report: &ScoringReport, ticker: &str) -> Option<StockEvaluation> {
    report
        .eligible
        .iter()
        .chain(&report.ineligible)
        .find(|evaluation| evaluation.ticker.eq_ignore_ascii_case(ticker))
        .cloned()
}

fn render_stock_score(result: &Result<StockScore, String>, area: Rect, frame: &mut ratatui::Frame) {
    let lines = match result {
        Ok(stock_score) if stock_score.evaluation.eligibility.eligible => {
            let evaluation = &stock_score.evaluation;
            let score = evaluation.score.as_ref();
            let score_text = score
                .map(|score| format!("{:.2}/100", score.total_score))
                .unwrap_or_else(|| "not available".to_string());
            let rank_text = evaluation
                .rank
                .map(|rank| format!("rank {rank}"))
                .unwrap_or_else(|| "unranked".to_string());
            let category_scores = score
                .map(|score| {
                    [
                        Category::Value,
                        Category::Quality,
                        Category::Growth,
                        Category::FinancialHealth,
                        Category::Dividends,
                    ]
                    .iter()
                    .filter_map(|category| {
                        score.category_scores.get(category).and_then(|value| {
                            value.map(|value| format!("{} {:.0}", category_label(*category), value))
                        })
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
                })
                .unwrap_or_default();
            vec![
                Line::from(vec![
                    Span::styled("QARP: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{score_text} ({rank_text})"),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        " | coverage {:.0}% | {} eligible",
                        evaluation.eligibility.overall_coverage * 100.0,
                        stock_score.eligible_count
                    )),
                ]),
                Line::from(category_scores),
                Line::from(stock_score.warnings.first().map_or_else(
                    || "Eligible for ranking; relative score, not a return prediction.".to_string(),
                    |warning| format!("Eligible; {warning}"),
                )),
            ]
        }
        Ok(stock_score) => {
            let evaluation = &stock_score.evaluation;
            let reasons = evaluation
                .eligibility
                .exclusion_reasons
                .iter()
                .map(exclusion_label)
                .collect::<Vec<_>>()
                .join("; ");
            vec![
                Line::from(vec![
                    Span::styled("QARP: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        "Ineligible",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        " | coverage {:.0}%",
                        evaluation.eligibility.overall_coverage * 100.0
                    )),
                ]),
                Line::from(reasons),
            ]
        }
        Err(error) => vec![
            Line::from(vec![
                Span::styled("QARP: ", Style::default().fg(Color::Cyan)),
                Span::styled("Score unavailable", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(error.as_str()),
        ],
    };
    let panel = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Fundamental Score "),
    );
    frame.render_widget(panel, area);
}

fn category_label(category: Category) -> &'static str {
    match category {
        Category::Value => "Value",
        Category::Quality => "Quality",
        Category::Growth => "Growth",
        Category::FinancialHealth => "Health",
        Category::Dividends => "Dividends",
    }
}

fn exclusion_label(reason: &scoring::ExclusionReason) -> String {
    use scoring::ExclusionReason;

    match reason {
        ExclusionReason::MissingMandatoryIndicator(indicator) => {
            format!("missing mandatory {indicator:?}")
        }
        ExclusionReason::NegativeEarnings => "nonpositive earnings make P/L unsuitable".to_string(),
        ExclusionReason::InsufficientOverallCoverage => {
            "insufficient overall data coverage".to_string()
        }
        ExclusionReason::InsufficientCategoryCoverage(category) => {
            format!("insufficient {} coverage", category_label(*category))
        }
        ExclusionReason::StaleQuote => "quote data is stale".to_string(),
        ExclusionReason::MissingQuoteDate => "quote date is missing".to_string(),
        ExclusionReason::FutureQuoteDate => "quote date is in the future".to_string(),
        ExclusionReason::StrategySpecific(reason) => reason.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(ticker: &str, pe: &str) -> StockSummary {
        StockSummary {
            ticker: ticker.to_string(),
            cotacao: "10,00".to_string(),
            p_l: pe.to_string(),
            p_vp: "2,00".to_string(),
            div_yield: "4,00".to_string(),
            roe: "12,00".to_string(),
            qarp_score: None,
            qarp_rank: None,
            qarp_eligible: None,
        }
    }

    fn details(ticker: &str, pe: &str) -> StockDetails {
        StockDetails {
            ticker: ticker.to_string(),
            cotacao: "10,00".to_string(),
            data_cot: "04/11/2026".to_string(),
            p_l: pe.to_string(),
            p_vp: "2,00".to_string(),
            div_yield: "4,00".to_string(),
            roe: "12,00%".to_string(),
            roic: "10,00%".to_string(),
            marg_ebit: "20,00%".to_string(),
            marg_liquida: "10,00%".to_string(),
            ev_ebitda: "8,00".to_string(),
            div_liquida: "100,00".to_string(),
            patrim_liquido: "500,00".to_string(),
            ..StockDetails::default()
        }
    }

    #[test]
    fn detailed_strategy_scores_snapshots_and_rejects_negative_earnings() {
        let snapshots = [details("LOWPE", "8,00"), details("NEG", "-4,00")];
        let stocks = snapshots
            .iter()
            .map(StockFundamentals::from)
            .collect::<Vec<_>>();
        let report = scoring::run_strategy(&QarpStrategy::default(), &stocks).unwrap();

        assert_eq!(report.eligible.len(), 1);
        assert_eq!(report.eligible[0].ticker, "LOWPE");
        assert_eq!(report.eligible[0].rank, Some(1));
        assert_eq!(report.ineligible.len(), 1);
        assert!(
            report.ineligible[0]
                .eligibility
                .exclusion_reasons
                .contains(&scoring::ExclusionReason::NegativeEarnings)
        );
    }

    #[test]
    fn score_sort_uses_ascending_qarp_rank_and_places_ineligible_stocks_last() {
        let sorted = sort_summaries_by_score(vec![
            StockSummary {
                qarp_score: Some(95.0),
                qarp_rank: Some(2),
                qarp_eligible: Some(true),
                ..summary("LOW", "12,00")
            },
            StockSummary {
                qarp_score: None,
                qarp_rank: None,
                qarp_eligible: Some(false),
                ..summary("NOPE", "-1,00")
            },
            StockSummary {
                qarp_score: Some(10.0),
                qarp_rank: Some(1),
                qarp_eligible: Some(true),
                ..summary("HIGH", "8,00")
            },
        ]);

        assert_eq!(
            sorted
                .iter()
                .map(|stock| stock.ticker.as_str())
                .collect::<Vec<_>>(),
            vec!["HIGH", "LOW", "NOPE"]
        );
    }

    #[test]
    fn filtering_or_scoring_preserves_selected_ticker_after_row_reordering() {
        let reordered = vec![
            summary("BBB", "12,00"),
            summary("AAA", "8,00"),
            summary("CCC", "10,00"),
        ];

        assert_eq!(restored_selection_index(&reordered, Some("BBB"), 1), 0);
    }

    #[test]
    fn selection_falls_back_to_nearest_row_when_selected_ticker_is_filtered_out() {
        let summaries = vec![summary("AAA", "8,00"), summary("CCC", "10,00")];

        assert_eq!(restored_selection_index(&summaries, Some("BBB"), 1), 1);
        assert_eq!(restored_selection_index(&[], Some("BBB"), 1), 0);
    }

    #[test]
    fn detail_popup_uses_the_same_detailed_score_and_rank_as_the_list() {
        let snapshots = vec![
            details("AAA", "8,00"),
            details("BBB", "12,00"),
            details("CCC", "-1,00"),
            details("DDD", "9,00"),
            details("EEE", "10,00"),
        ];
        let summaries = snapshots
            .iter()
            .map(stock_summary_from_details)
            .collect::<Vec<_>>();
        let snapshot_map = snapshots
            .into_iter()
            .map(|snapshot| (snapshot.ticker.clone(), snapshot))
            .collect::<std::collections::BTreeMap<_, _>>();
        let visible_snapshots = visible_detail_snapshots(&summaries, &snapshot_map);
        let stocks = visible_snapshots
            .iter()
            .map(StockFundamentals::from)
            .collect::<Vec<_>>();
        let table_report = scoring::run_strategy(&QarpStrategy::default(), &stocks).unwrap();
        let popup_score = score_stock_details_universe(&summaries, &snapshot_map, "AAA").unwrap();
        let table_evaluation = evaluation_for_ticker(&table_report, "AAA").unwrap();

        assert_eq!(popup_score.evaluation.rank, table_evaluation.rank);
        assert_eq!(
            popup_score
                .evaluation
                .score
                .as_ref()
                .map(|score| score.total_score),
            table_evaluation
                .score
                .as_ref()
                .map(|score| score.total_score)
        );
        assert_eq!(
            popup_score.evaluation.eligibility.eligible,
            table_evaluation.eligibility.eligible
        );
    }

    #[test]
    fn sector_filter_matches_saved_sector_case_insensitively() {
        let summaries = [summary("BANK", "8,00"), summary("ENERGY", "10,00")];
        let sector_by_ticker = std::collections::BTreeMap::from([
            ("BANK".to_string(), "Financials".to_string()),
            ("ENERGY".to_string(), "Utilities".to_string()),
        ]);
        let filtered =
            filter_summaries(&summaries, "", Some("financials"), &sector_by_ticker, None);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].ticker, "BANK");
    }

    #[test]
    fn sector_filter_does_not_show_stocks_without_saved_sector_data() {
        let summaries = [summary("SAVED", "8,00"), summary("UNKNOWN", "10,00")];
        let sector_by_ticker =
            std::collections::BTreeMap::from([("SAVED".to_string(), "Retail".to_string())]);
        let filtered = filter_summaries(&summaries, "", Some("Retail"), &sector_by_ticker, None);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].ticker, "SAVED");
    }

    #[test]
    fn market_ticker_universe_excludes_fundamentus_only_stocks() {
        let tickers = vec!["B3ONLY".to_string(), "SHARED".to_string()];
        let snapshots = std::collections::BTreeMap::from([
            ("SHARED".to_string(), details("SHARED", "8,00")),
            (
                "FUNDAMENTUS_ONLY".to_string(),
                details("FUNDAMENTUS_ONLY", "5,00"),
            ),
        ]);

        let combined = summaries_for_market_tickers(&tickers, &snapshots);

        assert_eq!(
            combined
                .iter()
                .map(|stock| stock.ticker.as_str())
                .collect::<Vec<_>>(),
            vec!["B3ONLY", "SHARED"]
        );
        assert_eq!(combined[0].p_l, "-");
        assert_eq!(combined[1].p_l, "8,00");
        assert_eq!(combined[1].cotacao, "10,00");
    }

    #[test]
    fn detailed_scoring_ignores_market_tickers_without_saved_snapshots() {
        let rows = vec![summary("SAVED", "8,00"), summary("NOT_SAVED", "-")];
        let snapshots =
            std::collections::BTreeMap::from([("SAVED".to_string(), details("SAVED", "8,00"))]);

        let visible = visible_detail_snapshots(&rows, &snapshots);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].ticker, "SAVED");
    }

    #[test]
    fn b3_filter_keeps_only_exact_listed_ticker_matches() {
        let summaries = [
            summary("AAA3", "8,00"),
            summary("AAA4", "9,00"),
            summary("BBB3", "10,00"),
        ];
        let listed = std::collections::BTreeSet::from(["AAA3".to_string()]);
        let filtered = filter_summaries(&summaries, "", None, &Default::default(), Some(&listed));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].ticker, "AAA3");
    }
}
