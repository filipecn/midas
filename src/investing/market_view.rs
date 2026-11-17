use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
};

use market_data::EnhancedMarketSeries;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table},
};
use ratatui_interact::components::{Spinner, SpinnerFrames, SpinnerState, SpinnerStyle};

use crate::{
    investing::StockDetails,
    investing::{HistoryState, MarketData, TechnicalAnalysisRequest},
    source_data::fundamentus::{self as fetcher, StockDetailsState},
    source_data::news::sentiment::{SentimentEngine, SentimentLabel, SentimentScore},
    source_data::news::{NewsArticle, NewsClient},
    widgets::{
        ActionResult, InteractableWidget,
        input_box::InputBox,
        list::{GenericList, ListColumn},
    },
};

const SMA_PERIOD: usize = 14;
const EMA_PERIOD: usize = 20;
const RSI_PERIOD: usize = 14;
const MACD_PARAMS: (usize, usize, usize) = (12, 26, 9);

#[derive(Clone)]
struct MarketStock {
    ticker: String,
    details: Option<StockDetails>,
    series: Option<Arc<EnhancedMarketSeries>>,
}

impl MarketStock {
    fn sector(&self) -> Option<&str> {
        self.details
            .as_ref()
            .and_then(|details| sector_name(&details.setor))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailsTab {
    Technical,
    Fundamentus,
    MaterialFact,
    News,
}

struct DetailsWindow {
    ticker: String,
    tab: DetailsTab,
    fundamentus_error: Option<String>,
    technical_error: Option<String>,
    fundamentus_loaded: bool,
    technical_loaded: bool,
    material_facts: Option<Vec<crate::source_data::cvm::companies::IpeDocument>>,
    material_fact_error: Option<String>,
    material_fact_requested: bool,
    material_fact_rx:
        Option<Receiver<Result<Vec<crate::source_data::cvm::companies::IpeDocument>, String>>>,
    news: Option<Vec<NewsArticle>>,
    news_error: Option<String>,
    news_requested: bool,
    news_rx: Option<Receiver<Result<Vec<NewsArticle>, String>>>,
    spinner_state: SpinnerState,
}

pub struct MarketView {
    stocks_list: GenericList<MarketStock>,
    all_stocks: Vec<MarketStock>,
    ticker_filter: InputBox,
    tickers_loaded: bool,
    ticker_load_error: Option<String>,
    sectors: Vec<String>,
    selected_sector_index: usize,
    sector_panel_focused: bool,
    status_message: String,
    details_window: Option<DetailsWindow>,
    sentiment_engine: SentimentEngine,
}

impl MarketView {
    pub fn new() -> Self {
        fetcher::Fundamentus::fetch_tickers();
        let mut ticker_filter = InputBox::new("Search ticker");
        ticker_filter.state.focused = true;
        let mut view = Self {
            stocks_list: create_stocks_list(Vec::new()),
            all_stocks: Vec::new(),
            ticker_filter,
            tickers_loaded: false,
            ticker_load_error: None,
            sectors: Vec::new(),
            selected_sector_index: 0,
            sector_panel_focused: false,
            status_message: String::new(),
            details_window: None,
            sentiment_engine: SentimentEngine::default(),
        };
        view.load_saved_stocks();
        view
    }

    pub fn update(&mut self) {
        if !self.tickers_loaded && self.ticker_load_error.is_none() {
            match fetcher::Fundamentus::ticker_list_state() {
                Ok(Some(tickers)) => {
                    self.set_tickers(tickers);
                    self.tickers_loaded = true;
                }
                Ok(None) => {}
                Err(error) => {
                    log::error!(target: "midas", "Could not load market tickers: {error}");
                    self.ticker_load_error = Some(error);
                }
            }
        }

        let Some(ticker) = self
            .details_window
            .as_ref()
            .map(|window| window.ticker.clone())
        else {
            return;
        };

        let mut refresh_stocks = false;
        match fetcher::Fundamentus::stock_details(&ticker) {
            Some(StockDetailsState::Loaded(details)) => {
                let should_store = self
                    .details_window
                    .as_ref()
                    .is_some_and(|window| !window.fundamentus_loaded);
                if should_store {
                    if let Err(error) = crate::db::fundamental::Stocks::write_stock_to_db(&details)
                    {
                        log::error!(
                            target: "midas",
                            "Failed to save Fundamentus details for {}: {error}",
                            details.ticker
                        );
                        self.status_message = format!("Could not save details: {error}");
                    }
                    self.update_stock_details(details);
                    refresh_stocks = true;
                }
                if let Some(window) = &mut self.details_window {
                    window.fundamentus_loaded = true;
                    window.fundamentus_error = None;
                }
            }
            Some(StockDetailsState::Error(error)) => {
                if let Some(window) = &mut self.details_window {
                    window.fundamentus_error = Some(error);
                }
            }
            Some(StockDetailsState::Loading) | None => {}
        }

        let provider_ticker = provider_ticker(&ticker);
        match MarketData::history(&provider_ticker) {
            Some(HistoryState::Loaded(series)) => {
                let should_store = self
                    .details_window
                    .as_ref()
                    .is_some_and(|window| !window.technical_loaded);
                if should_store {
                    self.update_stock_series(&ticker, series);
                    refresh_stocks = true;
                }
                if let Some(window) = &mut self.details_window {
                    window.technical_loaded = true;
                    window.technical_error = None;
                }
            }
            Some(HistoryState::Error(error)) => {
                if let Some(window) = &mut self.details_window {
                    window.technical_error = Some(error);
                }
            }
            Some(HistoryState::Loading) | None => {
                if let Some(window) = &mut self.details_window {
                    window
                        .spinner_state
                        .tick_with_frames(SpinnerFrames::Dots.frames().len());
                }
            }
        }

        if refresh_stocks {
            self.refresh_sector_options();
            self.refresh_filtered_stocks(None);
        }

        self.start_material_fact_load_if_ready();
        self.poll_material_fact_load();
        self.start_news_load_if_ready();
        self.poll_news_load();
    }

    pub fn close_details_window(&mut self) -> bool {
        self.details_window.take().is_some()
    }

    fn load_saved_stocks(&mut self) {
        match crate::db::fundamental::Stocks::read_all() {
            Ok(stocks) => {
                self.all_stocks = stocks
                    .into_iter()
                    .map(|details| MarketStock {
                        ticker: details.ticker.trim().to_ascii_uppercase(),
                        details: Some(details),
                        series: None,
                    })
                    .collect();
                self.refresh_sector_options();
            }
            Err(error) => {
                log::error!(target: "midas", "Could not load saved stock details: {error}");
                self.status_message = format!("Could not load saved stock details: {error}");
            }
        }
    }

    fn set_tickers(&mut self, tickers: Vec<String>) {
        let mut details_by_ticker = self
            .all_stocks
            .drain(..)
            .filter_map(|stock| stock.details.map(|details| (stock.ticker, details)))
            .collect::<BTreeMap<_, _>>();
        self.all_stocks = tickers
            .into_iter()
            .map(|ticker| {
                let ticker = ticker.trim().to_ascii_uppercase();
                MarketStock {
                    details: details_by_ticker.remove(&ticker),
                    ticker,
                    series: None,
                }
            })
            .collect();
        self.refresh_sector_options();
        self.refresh_filtered_stocks(None);
    }

    fn update_stock_details(&mut self, details: StockDetails) {
        let ticker = details.ticker.trim().to_ascii_uppercase();
        if let Some(stock) = self
            .all_stocks
            .iter_mut()
            .find(|stock| stock.ticker.eq_ignore_ascii_case(&ticker))
        {
            stock.details = Some(details);
        }
        self.refresh_sector_options();
    }

    fn update_stock_series(&mut self, ticker: &str, series: Arc<EnhancedMarketSeries>) {
        if let Some(stock) = self
            .all_stocks
            .iter_mut()
            .find(|stock| stock.ticker.eq_ignore_ascii_case(ticker))
        {
            stock.series = Some(series);
        }
    }

    fn refresh_sector_options(&mut self) {
        let selected = self.selected_sector().map(str::to_string);
        self.sectors = self
            .all_stocks
            .iter()
            .filter_map(MarketStock::sector)
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
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
        self.refresh_filtered_stocks(None);
    }

    fn refresh_filtered_stocks(&mut self, selected_ticker: Option<&str>) {
        let previous_index = self.stocks_list.state.selected_index;
        let selected_ticker = selected_ticker.map(str::to_string).or_else(|| {
            self.stocks_list
                .selected_row()
                .map(|stock| stock.ticker.clone())
        });
        let filter = self.ticker_filter.state.text.to_ascii_uppercase();
        let sector = self.selected_sector().map(str::to_string);
        let rows = self
            .all_stocks
            .iter()
            .filter(|stock| stock.ticker.contains(&filter))
            .filter(|stock| {
                sector.as_deref().is_none_or(|sector| {
                    stock
                        .sector()
                        .is_some_and(|stock_sector| stock_sector.eq_ignore_ascii_case(sector))
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        self.stocks_list = create_stocks_list(rows);
        self.stocks_list.state.selected_index = selected_ticker
            .as_deref()
            .and_then(|ticker| {
                self.stocks_list
                    .rows
                    .iter()
                    .position(|stock| stock.ticker.eq_ignore_ascii_case(ticker))
            })
            .unwrap_or_else(|| previous_index.min(self.stocks_list.rows.len().saturating_sub(1)));
    }

    fn open_selected_stock(&mut self) {
        let Some(stock) = self.stocks_list.selected_row() else {
            return;
        };
        let ticker = stock.ticker.clone();
        let series = stock.series.clone();
        fetcher::Fundamentus::fetch_stock_details(ticker.clone());
        let request = create_request(&ticker);
        MarketData::fetch_history(request);

        let has_series = series.is_some();
        self.details_window = Some(DetailsWindow {
            ticker: ticker.clone(),
            tab: DetailsTab::Technical,
            fundamentus_error: None,
            technical_error: None,
            fundamentus_loaded: false,
            technical_loaded: has_series,
            material_facts: None,
            material_fact_error: None,
            material_fact_requested: false,
            material_fact_rx: None,
            news: None,
            news_error: None,
            news_requested: false,
            news_rx: None,
            spinner_state: SpinnerState::for_frames(SpinnerFrames::Dots),
        });

        if let Some(series) = series {
            self.update_stock_series(&ticker, series);
        }
    }

    fn retry_details(&mut self) {
        let Some(window) = &mut self.details_window else {
            return;
        };
        if window.fundamentus_error.is_some() {
            window.fundamentus_error = None;
            window.fundamentus_loaded = false;
            fetcher::Fundamentus::fetch_stock_details(window.ticker.clone());
        }
        if window.technical_error.is_some() {
            window.technical_error = None;
            window.technical_loaded = false;
            MarketData::fetch_history(create_request(&window.ticker));
        }
        if window.material_fact_error.is_some() {
            window.material_fact_error = None;
            window.material_fact_requested = false;
        }
        if window.news_error.is_some() {
            window.news_error = None;
            window.news_requested = false;
        }
    }

    fn handle_details_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        match key.code {
            crossterm::event::KeyCode::Tab => {
                if let Some(window) = &mut self.details_window {
                    window.tab = match window.tab {
                        DetailsTab::Technical => DetailsTab::Fundamentus,
                        DetailsTab::Fundamentus => DetailsTab::MaterialFact,
                        DetailsTab::MaterialFact => DetailsTab::News,
                        DetailsTab::News => DetailsTab::Technical,
                    };
                }
                ActionResult::Consumed
            }
            crossterm::event::KeyCode::BackTab => {
                if let Some(window) = &mut self.details_window {
                    window.tab = match window.tab {
                        DetailsTab::Technical => DetailsTab::News,
                        DetailsTab::Fundamentus => DetailsTab::Technical,
                        DetailsTab::MaterialFact => DetailsTab::Fundamentus,
                        DetailsTab::News => DetailsTab::MaterialFact,
                    };
                }
                ActionResult::Consumed
            }
            crossterm::event::KeyCode::Char('r') => {
                self.retry_details();
                ActionResult::Consumed
            }
            _ => ActionResult::Consumed,
        }
    }

    fn start_material_fact_load_if_ready(&mut self) {
        let Some(window) = self.details_window.as_mut() else {
            return;
        };
        if window.material_fact_requested {
            return;
        }
        let ticker = window.ticker.clone();
        let (tx, rx) = mpsc::channel();
        window.material_fact_requested = true;
        window.material_fact_rx = Some(rx);
        std::thread::spawn(move || {
            let result = load_material_facts(&ticker);
            let _ = tx.send(result);
        });
    }

    fn poll_material_fact_load(&mut self) {
        let Some(window) = self.details_window.as_mut() else {
            return;
        };
        let Some(rx) = window.material_fact_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(rows)) => {
                window.material_facts = Some(rows);
                window.material_fact_error = None;
                window.material_fact_rx = None;
            }
            Ok(Err(error)) => {
                window.material_fact_error = Some(error);
                window.material_fact_rx = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                window.material_fact_error =
                    Some("Material fact lookup stopped unexpectedly".into());
                window.material_fact_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn start_news_load_if_ready(&mut self) {
        let Some(window) = self.details_window.as_mut() else {
            return;
        };
        if window.news_requested {
            return;
        }
        let ticker = window.ticker.clone();
        let company_name = self
            .all_stocks
            .iter()
            .find(|stock| stock.ticker.eq_ignore_ascii_case(&ticker))
            .and_then(|stock| stock.details.as_ref())
            .map(|details| details.nome.clone())
            .filter(|name| !name.trim().is_empty() && name != "-");
        let query = match company_name {
            Some(name) => format!("\"{name}\" OR {ticker}"),
            None => ticker.clone(),
        };
        let (tx, rx) = mpsc::channel();
        window.news_requested = true;
        window.news_rx = Some(rx);
        std::thread::spawn(move || {
            let result = NewsClient::default()
                .google_news_search(&query)
                .map_err(|error| error.to_string());
            let _ = tx.send(result);
        });
    }

    fn poll_news_load(&mut self) {
        let Some(window) = self.details_window.as_mut() else {
            return;
        };
        let Some(rx) = window.news_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(articles)) => {
                window.news = Some(articles);
                window.news_error = None;
                window.news_rx = None;
            }
            Ok(Err(error)) => {
                window.news_error = Some(error);
                window.news_rx = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                window.news_error = Some("News lookup stopped unexpectedly".into());
                window.news_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
}

impl Default for MarketView {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractableWidget for MarketView {
    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        if self.details_window.is_some() {
            return self.handle_details_key(key);
        }

        if key.code == crossterm::event::KeyCode::Tab
            || key.code == crossterm::event::KeyCode::BackTab
        {
            self.sector_panel_focused = !self.sector_panel_focused;
            self.ticker_filter.state.focused = !self.sector_panel_focused;
            return ActionResult::Consumed;
        }

        if self.sector_panel_focused {
            match key.code {
                crossterm::event::KeyCode::Up => {
                    self.move_sector_selection(false);
                    return ActionResult::Consumed;
                }
                crossterm::event::KeyCode::Down => {
                    self.move_sector_selection(true);
                    return ActionResult::Consumed;
                }
                crossterm::event::KeyCode::Left
                | crossterm::event::KeyCode::Right
                | crossterm::event::KeyCode::Enter => {
                    self.sector_panel_focused = false;
                    self.ticker_filter.state.focused = true;
                    return ActionResult::Consumed;
                }
                _ => return ActionResult::Consumed,
            }
        }

        if key.code == crossterm::event::KeyCode::Char('r')
            && !self.tickers_loaded
            && self.ticker_load_error.is_some()
        {
            self.ticker_load_error = None;
            fetcher::Fundamentus::fetch_tickers();
            return ActionResult::Consumed;
        }

        if key.code == crossterm::event::KeyCode::Enter {
            self.open_selected_stock();
            return ActionResult::Consumed;
        }

        match self.ticker_filter.handle_key(key) {
            ActionResult::PatternChanged(_) => {
                self.refresh_filtered_stocks(None);
                ActionResult::Consumed
            }
            ActionResult::None => self.stocks_list.handle_key(key),
            result => result,
        }
    }

    fn render(&self, area: Rect, frame: &mut ratatui::Frame) {
        let [filter_area, content_area, status_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .areas(area);
        self.ticker_filter.render(filter_area, frame);

        let [sector_area, stock_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(26), Constraint::Min(0)])
            .areas(content_area);
        render_sector_panel(
            sector_area,
            frame,
            &self.sectors,
            self.selected_sector_index,
            &self.all_stocks,
            self.sector_panel_focused,
            self.tickers_loaded,
        );

        if self.tickers_loaded {
            self.stocks_list.render(stock_area, frame);
        } else {
            let message = self
                .ticker_load_error
                .as_deref()
                .map(|error| format!("Could not load market tickers: {error} (press r to retry)"))
                .unwrap_or_else(|| "Loading market tickers...".to_string());
            frame.render_widget(
                Paragraph::new(message).alignment(Alignment::Center),
                stock_area,
            );
        }

        let status = if self.status_message.is_empty() {
            "Tab sectors | ↑/↓ select | Enter details".to_string()
        } else {
            format!("{} | Tab sectors | Enter details", self.status_message)
        };
        frame.render_widget(Paragraph::new(status), status_area);
        self.render_details_window(area, frame);
    }
}

impl MarketView {
    fn render_details_window(&self, area: Rect, frame: &mut ratatui::Frame) {
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
        let block = Block::default().borders(Borders::ALL).title(format!(
            " {} Market details | Tab to switch | Esc to close ",
            window.ticker
        ));
        let content_area = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let [tabs_area, details_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .areas(content_area);
        let tabs = Line::from(vec![
            tab_span(" Technical ", window.tab == DetailsTab::Technical),
            Span::raw("  "),
            tab_span(" Fundamentus ", window.tab == DetailsTab::Fundamentus),
            Span::raw("  "),
            tab_span(" Material Fact ", window.tab == DetailsTab::MaterialFact),
            Span::raw("  "),
            tab_span(" News ", window.tab == DetailsTab::News),
        ]);
        frame.render_widget(Paragraph::new(tabs), tabs_area);

        match window.tab {
            DetailsTab::Technical => self.render_technical_details(window, details_area, frame),
            DetailsTab::Fundamentus => {
                let details = self
                    .all_stocks
                    .iter()
                    .find(|stock| stock.ticker.eq_ignore_ascii_case(&window.ticker))
                    .and_then(|stock| stock.details.as_ref());
                if let Some(details) = details {
                    crate::investing::stock_view::FundamentusWidget::new(details)
                        .render(frame, details_area);
                } else if let Some(error) = &window.fundamentus_error {
                    frame.render_widget(
                        Paragraph::new(format!("{error} | Press r to retry"))
                            .alignment(Alignment::Center),
                        details_area,
                    );
                } else {
                    render_loading(
                        window,
                        "Loading Fundamentus details...",
                        details_area,
                        frame,
                    );
                }
            }
            DetailsTab::MaterialFact => self.render_material_facts(window, details_area, frame),
            DetailsTab::News => self.render_news(window, details_area, frame),
        }
    }

    fn render_news(&self, window: &DetailsWindow, area: Rect, frame: &mut ratatui::Frame) {
        if let Some(error) = &window.news_error {
            frame.render_widget(
                Paragraph::new(format!("{error} | Press r to retry")).alignment(Alignment::Center),
                area,
            );
            return;
        }
        let Some(articles) = &window.news else {
            render_loading(window, "Searching Google News...", area, frame);
            return;
        };
        if articles.is_empty() {
            frame.render_widget(
                Paragraph::new("No news found for this company or ticker.")
                    .alignment(Alignment::Center),
                area,
            );
            return;
        }
        let rows = articles.iter().take(100).map(|article| {
            let date = article
                .published
                .map(|date| date.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "Date unknown".into());
            let link = article.link.as_deref().unwrap_or("");
            let title = if link.is_empty() {
                article.title.clone()
            } else {
                format!("{}  {link}", article.title)
            };
            let sentiment = self.sentiment_engine.classify_article(article);
            ListItem::new(Line::from(vec![
                sentiment_icon(sentiment),
                Span::raw(format!("{date}  {}  {title}", article.source)),
            ]))
        });
        frame.render_widget(
            List::new(rows).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Google News "),
            ),
            area,
        );
    }

    fn render_material_facts(
        &self,
        window: &DetailsWindow,
        area: Rect,
        frame: &mut ratatui::Frame,
    ) {
        if let Some(error) = &window.material_fact_error {
            frame.render_widget(
                Paragraph::new(format!("{error} | Press r to retry")).alignment(Alignment::Center),
                area,
            );
            return;
        }
        let Some(facts) = &window.material_facts else {
            render_loading(window, "Loading CVM material facts...", area, frame);
            return;
        };
        if facts.is_empty() {
            frame.render_widget(
                Paragraph::new(
                    "No material facts found for this company in the current year's IPE data.",
                )
                .alignment(Alignment::Center),
                area,
            );
            return;
        }
        let rows = facts.iter().take(100).map(|fact| {
            let date = fact
                .submission_date
                .or(fact.reference_date)
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Date unknown".into());
            let subject = fact
                .subject
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .or(fact.species.as_deref())
                .or(fact.document_type.as_deref())
                .unwrap_or("Material Fact");
            let link = if fact
                .download_url
                .as_deref()
                .is_some_and(|url| !url.is_empty())
            {
                " [document available]"
            } else {
                ""
            };
            let sentiment = self.sentiment_engine.classify(subject);
            ListItem::new(Line::from(vec![
                sentiment_icon(sentiment),
                Span::raw(format!("{date}  {subject}{link}")),
            ]))
        });
        frame.render_widget(
            List::new(rows).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" CVM IPE material facts "),
            ),
            area,
        );
    }

    fn render_technical_details(
        &self,
        window: &DetailsWindow,
        area: Rect,
        frame: &mut ratatui::Frame,
    ) {
        let stock = self
            .all_stocks
            .iter()
            .find(|stock| stock.ticker.eq_ignore_ascii_case(&window.ticker));
        let Some(series) = stock.and_then(|stock| stock.series.as_deref()) else {
            if let Some(error) = &window.technical_error {
                frame.render_widget(
                    Paragraph::new(format!("{error} | Press r to retry"))
                        .alignment(Alignment::Center),
                    area,
                );
            } else {
                render_loading(window, "Loading technical analysis...", area, frame);
            }
            return;
        };

        let [metrics_area, history_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(9), Constraint::Min(0)])
            .areas(area);
        let last = series.series.last();
        let mut metrics = vec![
            ("Symbol", series.symbol.clone()),
            ("Interval", format!("{:?}", series.interval)),
            (
                "Last updated",
                last.map(|bar| bar.datetime.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            (
                "Open",
                last.map_or_else(|| "-".to_string(), |bar| format!("{:.2}", bar.open)),
            ),
            (
                "High",
                last.map_or_else(|| "-".to_string(), |bar| format!("{:.2}", bar.high)),
            ),
            (
                "Low",
                last.map_or_else(|| "-".to_string(), |bar| format!("{:.2}", bar.low)),
            ),
            (
                "Close",
                last.map_or_else(|| "-".to_string(), |bar| format!("{:.2}", bar.close)),
            ),
            (
                "Volume",
                last.map_or_else(|| "-".to_string(), |bar| format!("{:.0}", bar.volume)),
            ),
            (
                "SMA 14 / EMA 20 / RSI 14",
                format!(
                    "{} / {} / {}",
                    latest_indicator(&series.indicators.sma, &format!("SMA {SMA_PERIOD}")),
                    latest_indicator(&series.indicators.ema, &format!("EMA {EMA_PERIOD}")),
                    latest_indicator(&series.indicators.rsi, &format!("RSI {RSI_PERIOD}")),
                ),
            ),
        ];
        if let Some((macd, signal, histogram)) = series.indicators.macd.get(&format!(
            "MACD ({}, {}, {})",
            MACD_PARAMS.0, MACD_PARAMS.1, MACD_PARAMS.2
        )) {
            metrics.push((
                "MACD / Signal / Histogram",
                format!(
                    "{} / {} / {}",
                    latest_value(macd),
                    latest_value(signal),
                    latest_value(histogram)
                ),
            ));
        }
        let metric_rows = metrics
            .into_iter()
            .map(|(name, value)| Row::new(vec![name.to_string(), value]))
            .collect::<Vec<_>>();
        frame.render_widget(
            Table::new(
                metric_rows,
                [Constraint::Percentage(35), Constraint::Percentage(65)],
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Technical indicators "),
            ),
            metrics_area,
        );

        let history_rows = series
            .series
            .iter()
            .rev()
            .take(8)
            .map(|bar| {
                Row::new(vec![
                    bar.datetime.format("%Y-%m-%d %H:%M").to_string(),
                    format!("{:.2}", bar.open),
                    format!("{:.2}", bar.high),
                    format!("{:.2}", bar.low),
                    format!("{:.2}", bar.close),
                    format!("{:.0}", bar.volume),
                ])
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Table::new(
                history_rows,
                [
                    Constraint::Percentage(24),
                    Constraint::Percentage(14),
                    Constraint::Percentage(14),
                    Constraint::Percentage(14),
                    Constraint::Percentage(14),
                    Constraint::Percentage(20),
                ],
            )
            .header(Row::new(["Date", "Open", "High", "Low", "Close", "Volume"]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Recent history "),
            ),
            history_area,
        );
    }
}

fn create_request(ticker: &str) -> TechnicalAnalysisRequest {
    TechnicalAnalysisRequest {
        ticker: provider_ticker(ticker),
        sma_period: Some(SMA_PERIOD),
        ema_period: Some(EMA_PERIOD),
        rsi_period: Some(RSI_PERIOD),
        macd_params: Some(MACD_PARAMS),
        ..TechnicalAnalysisRequest::default()
    }
}

fn provider_ticker(ticker: &str) -> String {
    if ticker.ends_with(".SA") {
        ticker.to_string()
    } else {
        format!("{ticker}.SA")
    }
}

fn sector_name(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty() && value != "-").then_some(value)
}

fn latest_value(values: &std::collections::VecDeque<f32>) -> String {
    values
        .back()
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_string())
}

fn latest_indicator(
    indicators: &std::collections::HashMap<String, std::collections::VecDeque<f32>>,
    name: &str,
) -> String {
    indicators
        .get(name)
        .map(latest_value)
        .unwrap_or_else(|| "-".to_string())
}

fn load_material_facts(
    ticker: &str,
) -> Result<Vec<crate::source_data::cvm::companies::IpeDocument>, String> {
    use crate::source_data::cvm::{
        CkanClient, CsvEncoding, CsvOptions, ResourceDownloader,
        companies::{
            CompanyDataSource, CompanyDisclosureSource, CorporateDocumentKind, parse_ipe_rows,
            parse_registration_rows,
        },
        extract_zip,
    };
    use chrono::Datelike;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "midas-material-facts-{}-{stamp}",
        std::process::id()
    ));
    let result = (|| {
        let ckan = CkanClient::with_defaults().map_err(|error| error.to_string())?;
        let downloader = ResourceDownloader::default();
        let companies = CompanyDataSource::new(&ckan, &downloader);
        let options = CsvOptions {
            encoding: CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };
        let registration = companies
            .download_registration(dir.join("cad.csv"), false)
            .map_err(|error| error.to_string())?;
        let issuer_catalog = crate::source_data::b3::security_catalog()?;
        let issuer = issuer_catalog
            .issuer_for_ticker(ticker)
            .ok_or_else(|| format!("B3 has no issuer mapping for ticker {ticker}"))?;
        let mut identity = crate::source_data::company::CompanyIdentity::from_b3(issuer);
        let cvm_code_override = issuer_catalog.cvm_code_override(&issuer.code);
        let company = parse_registration_rows(
            std::fs::File::open(registration.path).map_err(|error| error.to_string())?,
            options,
        )
        .filter_map(Result::ok)
        .find(|company| {
            if let Some(cvm_code) = cvm_code_override {
                if company.cvm_code.trim_start_matches('0') == cvm_code {
                    return true;
                }
            }
            identity.matches_name(&company.legal_name)
                || company
                    .trading_name
                    .as_deref()
                    .is_some_and(|name| identity.matches_name(name))
        })
        .ok_or_else(|| {
            format!(
                "Could not match B3 issuer '{}' to a CVM company registration",
                issuer.name
            )
        })?;
        identity.add_registration(&company);
        let company_cnpj = identity.cnpj.as_deref().unwrap_or(&company.cnpj).to_owned();

        let year = chrono::Local::now().year();
        let disclosures = CompanyDisclosureSource::new(&ckan, &downloader);
        let archive = disclosures
            .download_ipe(year, dir.join(format!("ipe-{year}.zip")), false)
            .map_err(|error| error.to_string())?;
        let extracted =
            extract_zip(archive.path, dir.join("ipe")).map_err(|error| error.to_string())?;
        let csv = extracted
            .files
            .into_iter()
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .to_ascii_lowercase()
                        .starts_with("ipe_cia_aberta_")
                        && name
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .ends_with(".csv")
                })
            })
            .ok_or_else(|| "IPE archive did not contain its annual CSV".to_string())?;
        let mut facts = parse_ipe_rows(
            std::fs::File::open(csv).map_err(|error| error.to_string())?,
            options,
        )
        .filter_map(Result::ok)
        .filter(|fact| {
            fact.cnpj == company_cnpj && fact.kind() == CorporateDocumentKind::MaterialFact
        })
        .collect::<Vec<_>>();
        facts.sort_by_key(|fact| std::cmp::Reverse(fact.submission_date.or(fact.reference_date)));
        Ok(facts)
    })();
    let _ = std::fs::remove_dir_all(dir);
    result
}

fn render_loading(window: &DetailsWindow, label: &str, area: Rect, frame: &mut ratatui::Frame) {
    let spinner = Spinner::new(&window.spinner_state)
        .style(SpinnerStyle::new(SpinnerFrames::Dots).color(Color::Cyan))
        .label(label);
    frame.render_widget(spinner, area);
}

fn tab_span(label: &'static str, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(
            label,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(label)
    }
}

fn sentiment_icon(score: SentimentScore) -> Span<'static> {
    let (icon, color) = match score.label {
        SentimentLabel::Positive => ("▲ ", Color::Green),
        SentimentLabel::Neutral => ("● ", Color::Gray),
        SentimentLabel::Negative => ("▼ ", Color::Red),
    };
    Span::styled(
        icon,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn create_stocks_list(stocks: Vec<MarketStock>) -> GenericList<MarketStock> {
    let columns = BTreeMap::from([
        (
            "Ticker".to_string(),
            ListColumn::new(12, |stock: &MarketStock| stock.ticker.clone()),
        ),
        (
            "Sector".to_string(),
            ListColumn::new(20, |stock: &MarketStock| {
                stock.sector().unwrap_or("-").to_string()
            }),
        ),
        (
            "Quote".to_string(),
            ListColumn::new(12, |stock: &MarketStock| {
                stock
                    .details
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |details| details.cotacao.clone())
            }),
        ),
        (
            "P/L".to_string(),
            ListColumn::new(9, |stock: &MarketStock| {
                stock
                    .details
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |details| details.p_l.clone())
            }),
        ),
        (
            "P/VP".to_string(),
            ListColumn::new(9, |stock: &MarketStock| {
                stock
                    .details
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |details| details.p_vp.clone())
            }),
        ),
        (
            "Div Yield".to_string(),
            ListColumn::new(12, |stock: &MarketStock| {
                stock
                    .details
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |details| details.div_yield.clone())
            }),
        ),
        (
            "ROE".to_string(),
            ListColumn::new(9, |stock: &MarketStock| {
                stock
                    .details
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |details| details.roe.clone())
            }),
        ),
        (
            "Close".to_string(),
            ListColumn::new(10, |stock: &MarketStock| {
                stock
                    .series
                    .as_deref()
                    .and_then(|series| series.series.last())
                    .map_or_else(|| "-".to_string(), |bar| format!("{:.2}", bar.close))
            }),
        ),
        (
            "SMA 14".to_string(),
            ListColumn::new(10, |stock: &MarketStock| {
                stock.series.as_deref().map_or_else(
                    || "-".to_string(),
                    |series| latest_indicator(&series.indicators.sma, &format!("SMA {SMA_PERIOD}")),
                )
            }),
        ),
        (
            "EMA 20".to_string(),
            ListColumn::new(10, |stock: &MarketStock| {
                stock.series.as_deref().map_or_else(
                    || "-".to_string(),
                    |series| latest_indicator(&series.indicators.ema, &format!("EMA {EMA_PERIOD}")),
                )
            }),
        ),
        (
            "RSI 14".to_string(),
            ListColumn::new(9, |stock: &MarketStock| {
                stock.series.as_deref().map_or_else(
                    || "-".to_string(),
                    |series| latest_indicator(&series.indicators.rsi, &format!("RSI {RSI_PERIOD}")),
                )
            }),
        ),
    ]);
    GenericList::new(stocks, columns)
        .with_title(" Market by sector ")
        .with_sticky_header()
}

fn render_sector_panel(
    area: Rect,
    frame: &mut ratatui::Frame,
    sectors: &[String],
    selected_index: usize,
    stocks: &[MarketStock],
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
        stocks.len()
    )));
    items.extend(sectors.iter().enumerate().map(|(index, sector)| {
        let count = stocks
            .iter()
            .filter(|stock| {
                stock
                    .sector()
                    .is_some_and(|stock_sector| stock_sector.eq_ignore_ascii_case(sector))
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
        items.push(ListItem::new(if loaded {
            "No saved sectors"
        } else {
            "Loading sectors..."
        }));
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
                .title(" Sectors "),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(panel, area, &mut list_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_uses_market_ticker_suffix_and_indicator_defaults() {
        let request = create_request("PETR4");
        assert_eq!(request.ticker, "PETR4.SA");
        assert_eq!(request.sma_period, Some(14));
        assert_eq!(request.ema_period, Some(20));
        assert_eq!(request.rsi_period, Some(14));
        assert_eq!(request.macd_params, Some((12, 26, 9)));
    }

    #[test]
    fn sector_name_ignores_empty_and_missing_values() {
        assert_eq!(sector_name(" Banks "), Some("Banks"));
        assert_eq!(sector_name(""), None);
        assert_eq!(sector_name("-"), None);
    }

    #[test]
    fn market_list_combines_fundamentus_and_technical_columns() {
        let list = create_stocks_list(Vec::new());
        for column in [
            "Quote",
            "P/L",
            "P/VP",
            "Div Yield",
            "ROE",
            "Close",
            "SMA 14",
            "EMA 20",
            "RSI 14",
        ] {
            assert!(list.columns.contains_key(column), "missing {column} column");
        }
    }
}
