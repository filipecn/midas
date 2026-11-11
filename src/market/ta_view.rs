use std::{collections::BTreeMap, sync::Arc};

use market_data::EnhancedMarketSeries;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::Paragraph,
};

use crate::widgets::{
    InteractableWidget,
    input_box::InputBox,
    list::{GenericList, ListColumn},
};
use crate::{
    fundamentus::fetcher::Fundamentus,
    market::{HistoryState, MarketData, TechnicalAnalysisRequest},
    widgets::ActionResult,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketTechnicalAnalysisIndicator {
    Close,
    Sma(usize),
    Ema(usize),
    Rsi(usize),
    Macd(usize, usize, usize),
}

impl MarketTechnicalAnalysisIndicator {
    fn label(&self) -> String {
        match self {
            Self::Close => "Close".to_string(),
            Self::Sma(period) => format!("SMA {period}"),
            Self::Ema(period) => format!("EMA {period}"),
            Self::Rsi(period) => format!("RSI {period}"),
            Self::Macd(fast, slow, signal) => format!("MACD ({fast}, {slow}, {signal})"),
        }
    }

    fn value(&self, stock: &MarketTechnicalAnalysisStock) -> String {
        let Some(series) = stock.series.as_deref() else {
            return "-".to_string();
        };
        let value = match self {
            Self::Close => series.series.last().map(|bar| bar.close),
            Self::Sma(period) => series
                .indicators
                .sma
                .get(&format!("SMA {period}"))
                .and_then(|values| values.back())
                .copied(),
            Self::Ema(period) => series
                .indicators
                .ema
                .get(&format!("EMA {period}"))
                .and_then(|values| values.back())
                .copied(),
            Self::Rsi(period) => series
                .indicators
                .rsi
                .get(&format!("RSI {period}"))
                .and_then(|values| values.back())
                .copied(),
            Self::Macd(fast, slow, signal) => series
                .indicators
                .macd
                .get(&format!("MACD ({fast}, {slow}, {signal})"))
                .and_then(|(macd, _, _)| macd.back())
                .copied(),
        };

        value
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "-".to_string())
    }
}

#[derive(Clone)]
pub struct MarketTechnicalAnalysisStock {
    pub ticker: String,
    pub series: Option<Arc<EnhancedMarketSeries>>,
    loading: bool,
    error: Option<String>,
}

impl MarketTechnicalAnalysisStock {
    fn new(ticker: String) -> Self {
        Self {
            ticker,
            series: None,
            loading: false,
            error: None,
        }
    }

    fn status_icon(&self) -> char {
        if self.loading {
            '⟳'
        } else if self.error.is_some() {
            '!'
        } else if self.series.is_some() {
            '✓'
        } else {
            '·'
        }
    }
}

pub struct MarketTechnicalAnalysisView {
    pub stocks_list: GenericList<MarketTechnicalAnalysisStock>,
    pub indicators: Vec<MarketTechnicalAnalysisIndicator>,
    ticker_filter: InputBox,
    all_stocks: Vec<MarketTechnicalAnalysisStock>,
    tickers_loaded: bool,
    ticker_load_error: Option<String>,
    active_ticker: Option<(String, String)>,
}

impl MarketTechnicalAnalysisView {
    pub fn new() -> Self {
        Self::with_indicators(vec![
            MarketTechnicalAnalysisIndicator::Close,
            MarketTechnicalAnalysisIndicator::Sma(14),
            MarketTechnicalAnalysisIndicator::Ema(20),
            MarketTechnicalAnalysisIndicator::Rsi(14),
        ])
    }

    pub fn with_indicators(indicators: Vec<MarketTechnicalAnalysisIndicator>) -> Self {
        Fundamentus::fetch_tickers();
        let mut ticker_filter = InputBox::new("Search ticker");
        ticker_filter.state.focused = true;
        Self {
            stocks_list: Self::create_stocks_list(Vec::new(), &indicators),
            indicators,
            ticker_filter,
            all_stocks: Vec::new(),
            tickers_loaded: false,
            ticker_load_error: None,
            active_ticker: None,
        }
    }

    pub fn set_indicators(&mut self, indicators: Vec<MarketTechnicalAnalysisIndicator>) {
        let selected_ticker = self
            .stocks_list
            .selected_row()
            .map(|stock| stock.ticker.clone());
        for stock in &mut self.all_stocks {
            stock.series = None;
            stock.loading = false;
            stock.error = None;
        }
        self.active_ticker = None;
        self.indicators = indicators;
        self.refresh_filtered_stocks(selected_ticker.as_deref());
    }

    pub fn update(&mut self) {
        if !self.tickers_loaded && self.ticker_load_error.is_none() {
            match Fundamentus::ticker_list_state() {
                Ok(Some(tickers)) => {
                    self.set_tickers(tickers);
                    self.tickers_loaded = true;
                }
                Ok(None) => {}
                Err(error) => self.ticker_load_error = Some(error),
            }
        }

        let Some((ticker, request_ticker)) = self.active_ticker.clone() else {
            return;
        };
        match MarketData::history(&request_ticker) {
            Some(HistoryState::Loaded(series)) => {
                if let Some(stock) = self.all_stocks.iter_mut().find(|s| s.ticker == ticker) {
                    stock.series = Some(series);
                    stock.loading = false;
                }
                self.active_ticker = None;
                self.refresh_filtered_stocks(Some(&ticker));
            }
            Some(HistoryState::Error(error)) => {
                if let Some(stock) = self.all_stocks.iter_mut().find(|s| s.ticker == ticker) {
                    stock.error = Some(error);
                    stock.loading = false;
                }
                self.active_ticker = None;
                self.refresh_filtered_stocks(Some(&ticker));
            }
            Some(HistoryState::Loading) | None => {}
        }
    }

    fn set_tickers(&mut self, tickers: Vec<String>) {
        self.all_stocks = tickers
            .into_iter()
            .map(MarketTechnicalAnalysisStock::new)
            .collect();
        self.refresh_filtered_stocks(None);
    }

    fn refresh_filtered_stocks(&mut self, selected_ticker: Option<&str>) {
        let selected_index = self.stocks_list.state.selected_index;
        let filter = self.ticker_filter.state.text.to_uppercase();
        let stocks = self
            .all_stocks
            .iter()
            .filter(|stock| stock.ticker.to_uppercase().contains(&filter))
            .cloned()
            .collect::<Vec<_>>();
        self.stocks_list = Self::create_stocks_list(stocks, &self.indicators);

        if let Some(index) = selected_ticker.and_then(|ticker| {
            self.stocks_list
                .rows
                .iter()
                .position(|stock| stock.ticker == ticker)
        }) {
            self.stocks_list.state.selected_index = index;
        } else if !self.stocks_list.rows.is_empty() {
            self.stocks_list.state.selected_index =
                selected_index.min(self.stocks_list.rows.len() - 1);
        }
    }

    fn fetch_selected_stock(&mut self) {
        let selected_index = self.stocks_list.state.selected_index;
        let Some(stock) = self.stocks_list.rows.get(selected_index) else {
            return;
        };
        if stock.loading || stock.series.is_some() {
            return;
        }

        let ticker = stock.ticker.clone();
        if let Some((previous_ticker, _)) = self.active_ticker.take() {
            if let Some(previous_stock) = self
                .all_stocks
                .iter_mut()
                .find(|stock| stock.ticker == previous_ticker)
            {
                previous_stock.loading = false;
            }
        }
        let Some(stock) = self
            .all_stocks
            .iter_mut()
            .find(|stock| stock.ticker == ticker)
        else {
            return;
        };
        stock.loading = true;
        stock.error = None;
        let request = self.create_request(ticker.clone());
        self.active_ticker = Some((ticker.clone(), request.ticker.clone()));
        MarketData::fetch_history(request);
        self.refresh_filtered_stocks(Some(&ticker));
    }

    fn create_request(&self, ticker: String) -> TechnicalAnalysisRequest {
        let provider_ticker = if ticker.ends_with(".SA") {
            ticker
        } else {
            format!("{ticker}.SA")
        };
        let mut request = TechnicalAnalysisRequest {
            ticker: provider_ticker,
            ..TechnicalAnalysisRequest::default()
        };
        request.sma_period = self
            .indicators
            .iter()
            .find_map(|indicator| match indicator {
                MarketTechnicalAnalysisIndicator::Sma(period) => Some(*period),
                _ => None,
            });
        request.ema_period = self
            .indicators
            .iter()
            .find_map(|indicator| match indicator {
                MarketTechnicalAnalysisIndicator::Ema(period) => Some(*period),
                _ => None,
            });
        request.rsi_period = self
            .indicators
            .iter()
            .find_map(|indicator| match indicator {
                MarketTechnicalAnalysisIndicator::Rsi(period) => Some(*period),
                _ => None,
            });
        request.macd_params = self
            .indicators
            .iter()
            .find_map(|indicator| match indicator {
                MarketTechnicalAnalysisIndicator::Macd(fast, slow, signal) => {
                    Some((*fast, *slow, *signal))
                }
                _ => None,
            });
        request
    }

    fn create_stocks_list(
        stocks: Vec<MarketTechnicalAnalysisStock>,
        indicators: &[MarketTechnicalAnalysisIndicator],
    ) -> GenericList<MarketTechnicalAnalysisStock> {
        let mut columns = BTreeMap::from([(
            "Ticker".to_string(),
            ListColumn::new(14, |stock: &MarketTechnicalAnalysisStock| {
                format!("{} {}", stock.status_icon(), stock.ticker)
            }),
        )]);

        for indicator in indicators {
            let column_indicator = indicator.clone();
            columns.insert(
                indicator.label(),
                ListColumn::new(12, move |stock: &MarketTechnicalAnalysisStock| {
                    column_indicator.value(stock)
                }),
            );
        }

        let column_order = std::iter::once("Ticker".to_string())
            .chain(
                indicators
                    .iter()
                    .map(MarketTechnicalAnalysisIndicator::label),
            )
            .collect::<Vec<_>>();
        GenericList::new(stocks, columns)
            .with_column_order(column_order)
            .with_sticky_header()
    }
}

impl Default for MarketTechnicalAnalysisView {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractableWidget for MarketTechnicalAnalysisView {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let [filter_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .areas(area);
        self.ticker_filter.render(filter_area, frame);

        if !self.tickers_loaded {
            let message = self
                .ticker_load_error
                .as_deref()
                .map(|error| format!("Failed to load B3 tickers: {error} (press r to retry)"))
                .unwrap_or_else(|| "Loading B3 tickers...".to_string());
            frame.render_widget(
                Paragraph::new(message).alignment(Alignment::Center),
                list_area,
            );
        } else {
            if self.stocks_list.rows.is_empty() {
                let message = if self.ticker_filter.state.text.is_empty() {
                    "No B3 tickers returned".to_string()
                } else {
                    "No tickers match the search".to_string()
                };
                frame.render_widget(
                    Paragraph::new(message).alignment(Alignment::Center),
                    list_area,
                );
            } else {
                self.stocks_list.render(list_area, frame);
            }
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        if key.code == crossterm::event::KeyCode::Tab {
            self.ticker_filter.state.focused = !self.ticker_filter.state.focused;
            return ActionResult::Consumed;
        }

        if !self.tickers_loaded {
            if !self.ticker_filter.state.focused
                && key.code == crossterm::event::KeyCode::Char('r')
                && self.ticker_load_error.is_some()
            {
                self.ticker_load_error = None;
                Fundamentus::fetch_tickers();
                return ActionResult::Consumed;
            }
        }

        if self.ticker_filter.state.focused {
            match self.ticker_filter.handle_key(key) {
                ActionResult::PatternChanged(_) => {
                    let selected_ticker = self
                        .stocks_list
                        .selected_row()
                        .map(|stock| stock.ticker.clone());
                    self.refresh_filtered_stocks(selected_ticker.as_deref());
                    return ActionResult::Consumed;
                }
                ActionResult::None => {}
                result => return result,
            }
        }

        if key.code == crossterm::event::KeyCode::Enter {
            self.fetch_selected_stock();
            return ActionResult::Consumed;
        }
        if self.tickers_loaded {
            self.stocks_list.handle_key(key)
        } else {
            ActionResult::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_column_is_present_without_a_status_column() {
        let list = MarketTechnicalAnalysisView::create_stocks_list(
            Vec::new(),
            &[MarketTechnicalAnalysisIndicator::Close],
        );
        assert!(list.columns.contains_key("Ticker"));
        assert!(!list.columns.contains_key("Status"));
    }

    #[test]
    fn ticker_status_is_represented_by_one_icon() {
        let mut stock = MarketTechnicalAnalysisStock::new("TEST".to_string());
        assert_eq!(stock.status_icon(), '·');

        stock.loading = true;
        assert_eq!(stock.status_icon(), '⟳');

        stock.loading = false;
        stock.error = Some("request failed".to_string());
        assert_eq!(stock.status_icon(), '!');
    }
}
