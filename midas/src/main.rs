use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use dionysus::binance::{BinanceMarket, MarketEvent};
use dionysus::finance::{DiError, Quote, Sample, Token};
use dionysus::historical_data::HistoricalData;
use dionysus::indicators::match_indicator_from_text;
use dionysus::oracles::match_oracle_from_text;
use dionysus::time::{TimeUnit, TimeWindow};
use dionysus::{ERROR, INFO};
use ratatui::{
    layout::{Constraint, Flex, Layout, Position},
    widgets::Clear,
    DefaultTerminal, Frame,
};
use slog::{slog_error, slog_info};
use slog_scope;
use std::io;

mod common;
mod g_common;
mod g_curve;
mod g_element;
mod g_indicators;
mod g_samples;
mod g_strategy;
mod w_command;
mod w_graph;
mod w_interactible;
mod w_log;
mod w_market;
mod w_order_book;
mod w_strategy;
mod w_symbol_tabs;
mod w_wallet;

use w_command::CommandInput;
use w_graph::StockGraph;
use w_interactible::Interactible;
use w_log::LogWindow;
use w_market::MarketWindow;
use w_order_book::OrderBookWindow;
use w_strategy::StrategyWindow;
use w_symbol_tabs::SymbolTabs;
use w_wallet::WalletWindow;

#[derive(Default, Eq, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    Command,
}

#[derive(Default)]
pub struct App {
    exit: bool,
    stock_views: Vec<StockGraph>,
    symbol_tabs: SymbolTabs,
    command_w: CommandInput,
    input_mode: InputMode,
    wallet_w: WalletWindow,
    market_w: MarketWindow,
    market: BinanceMarket,
    order_book_w: OrderBookWindow,
    log_w: LogWindow,
    strategy_w: Vec<StrategyWindow>,
}

impl App {
    fn add_tab(&mut self, symbol: &str, currency: &str) {
        let time_window = TimeWindow {
            resolution: TimeUnit::Hour(1),
            count: 200,
        };
        let pair = Token::pair(
            String::from(symbol).to_uppercase().as_str(),
            String::from(currency).to_uppercase().as_str(),
        );

        match self.market.fetch_last(&pair, &time_window) {
            Ok(samples) => {
                let mut stock_graph = StockGraph::default();
                stock_graph.set_data(samples);
                stock_graph.reset_camera();
                self.stock_views.push(stock_graph);
                self.strategy_w.push(StrategyWindow::default());
                self.symbol_tabs.add(&pair);
                self.market.order_book_service(&pair);
                self.market.kline_service(&pair, &time_window.resolution);
            }
            Err(e) => ERROR!("{:?}", e),
        };
    }

    fn set_history_size(&mut self, n: usize) {
        if let Some((i, pair)) = self.symbol_tabs.current() {
            let mut time_window = self.stock_views[i].time_window.clone();
            time_window.count = n as i64;
            match self.market.fetch_last(&pair, &time_window) {
                Ok(samples) => self.stock_views[i].set_data(samples),
                Err(e) => ERROR!("{:?}", e),
            }
        }
    }

    fn set_resolution(&mut self, resolution_name: &str) {
        if let Some((curr_index, curr_token)) = self.symbol_tabs.current() {
            let mut time_window = self.stock_views[curr_index].time_window.clone();
            time_window.resolution = TimeUnit::from_name(resolution_name);

            match self.market.fetch_last(&curr_token, &time_window) {
                Ok(samples) => {
                    self.stock_views[curr_index].set_data(samples);
                    self.market
                        .kline_service(&curr_token, &time_window.resolution);
                }
                Err(e) => ERROR!("{:?}", e),
            };
        }
    }

    fn update_graph(&mut self, token: &Token) {
        for i in 0..self.symbol_tabs.tab_count() {
            if *token == self.symbol_tabs.tab(i).unwrap() {
                let time_window = self.stock_views[i].time_window.clone();
                match self.market.get_last(token, &time_window) {
                    Ok(samples) => {
                        self.stock_views[i].set_data(samples);
                    }
                    Err(e) => ERROR!("{:?}", e),
                };
            }
        }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.add_tab("kaito", "usdt");
        self.run_command("oracle mean-reversion 10");
        self.run_command("oracle macd-crossover 12 26 9");
        self.run_command("oracle macd-zero-cross 12 26 9");
        self.run_command("oracle ema-cross 50 200");
        self.market.day_ticker_all_service("USDT");

        let tick_rate = std::time::Duration::from_millis(16);
        let mut last_tick = std::time::Instant::now();

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                self.handle_events()?;
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = std::time::Instant::now();

                for event in self.market.get_events() {
                    match event {
                        MarketEvent::KLine((token, sample)) => {
                            if let Err(e) = self.market.append(&token, &sample) {
                                ERROR!("{:?}", e);
                            }
                            self.update_graph(&token);
                        }
                        MarketEvent::Ticks(ticks) => self.market_w.update_with(ticks),
                        MarketEvent::OrderBook(book) => {
                            if let Some((curr_index, current_token)) = self.symbol_tabs.current() {
                                if current_token == book.token {
                                    self.strategy_w[curr_index].run(
                                        &book.quote(),
                                        self.stock_views[curr_index].samples.data.as_slice(),
                                    );
                                    self.order_book_w.update_with(book);
                                }
                            }
                        }
                    };
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        //    0                      1                               2
        //  ------- -----------------------------------------------------------
        // |       |           SYMBOLS                        |                |
        // | BOOK  |------------------------------------------|    WALLET      |   a
        // |       |                                          |----------------|
        // |       |             CHART                        |    MARKET      |
        // |       |                                          |                |
        // |       |                                          |                |   b
        // |------ |                                          |----------------|
        // |       |                                          |                |
        // |ORACLE |------------------------------------------|    LOG         |   c
        // |       |            COMMAND                       |                |
        //  ------- -----------------------------------------------------------
        let layout_012 = Layout::horizontal([
            Constraint::Percentage(18),
            Constraint::Percentage(64),
            Constraint::Percentage(18),
        ]);

        let layout_0_ab =
            Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]);

        let layout_1_abc = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(4),
        ]);

        let layout_2_abc = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Min(0),
            Constraint::Percentage(20),
        ]);

        let [l0_area, l1_area, l2_area] = layout_012.areas(frame.area());

        let [book_area, strategy_area] = layout_0_ab.areas(l0_area);
        let [symbol_tabs_area, chart_area, command_area] = layout_1_abc.areas(l1_area);
        let [wallet_area, market_area, log_area] = layout_2_abc.areas(l2_area);

        frame.render_widget(&self.symbol_tabs, symbol_tabs_area);
        if let Some((curr_index, _)) = self.symbol_tabs.current() {
            frame.render_widget(&self.stock_views[curr_index], chart_area);
            self.strategy_w[curr_index].render(strategy_area, frame.buffer_mut());

            let legend_area = chart_area.clone();
            let vertical = Layout::vertical([Constraint::Percentage(15)]).flex(Flex::Start);
            let horizontal = Layout::horizontal([Constraint::Percentage(30)]).flex(Flex::End);
            let [legend_area] = vertical.areas(legend_area);
            let [legend_area] = horizontal.areas(legend_area);

            frame.render_widget(Clear, legend_area);
            self.stock_views[curr_index].draw_legend(legend_area, frame.buffer_mut());
        }

        frame.render_widget(&self.command_w, command_area);
        frame.set_cursor_position(Position::new(
            // Draw the cursor at the current position in the input field.
            // This position is can be controlled via the left and right arrow key
            command_area.x + self.command_w.cursor_position() + 1,
            // Move one line down, from the border to the input line
            command_area.y + 1,
        ));
        self.wallet_w.render(wallet_area, frame.buffer_mut());
        self.market_w.render(market_area, frame.buffer_mut());
        frame.render_widget(&self.log_w, log_area);

        self.order_book_w.render(book_area, frame.buffer_mut());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) => match self.input_mode {
                InputMode::Normal => {
                    if key_event.code == KeyCode::Char('a') {
                        self.input_mode = InputMode::Command;
                        self.command_w.set_focus(true);
                    } else {
                        let mut event_consumed = false;
                        event_consumed &= self.symbol_tabs.handle_key_event(&key_event);
                        if let Some((curr_index, _)) = self.symbol_tabs.current() {
                            event_consumed &=
                                self.stock_views[curr_index].handle_key_event(&key_event);
                            if event_consumed {
                                self.stock_views[curr_index].set_focus(true);
                            }
                        }
                        if !event_consumed {
                            match key_event.code {
                                KeyCode::Char('q') => self.exit(),
                                _ => {}
                            }
                        }
                    }
                }
                InputMode::Command => {
                    self.command_w.set_focus(false);
                    if key_event.code == KeyCode::Enter {
                        self.run_command(self.command_w.text().as_str());
                        self.command_w.clear();
                        self.input_mode = InputMode::Normal;
                    } else if key_event.code == KeyCode::Esc {
                        self.input_mode = InputMode::Normal;
                        self.command_w.clear();
                    } else {
                        self.command_w.set_focus(true);
                        self.command_w.handle_key_event(&key_event);
                    }
                }
            },
            _ => {}
        };
        Ok(())
    }

    fn run_command(&mut self, command: &str) {
        if command.is_empty() {
            return;
        }
        let words: Vec<&str> = command.split(' ').collect();
        match words[0].to_uppercase().as_str() {
            "LOAD" => self.add_tab(words[1], if words.len() > 2 { words[2] } else { "usdt" }),
            "GRAPH" => self.add_indicator(&words[1..]),
            "RES" => self.set_resolution(&words[1]),
            "ORACLE" => self.add_oracle(&words[1..]),
            "HIST" => {
                if let Ok(n) = words[1].parse::<usize>() {
                    self.set_history_size(n);
                }
            }
            _ => (),
        };
    }

    fn add_indicator(&mut self, words: &[&str]) {
        if let Some((curr_index, _)) = self.symbol_tabs.current() {
            match match_indicator_from_text(&words) {
                Some(indicator) => self.stock_views[curr_index].add_indicator(&indicator),
                None => (),
            };
        }
    }

    fn add_oracle(&mut self, words: &[&str]) {
        match match_oracle_from_text(&words) {
            Some(oracle) => {
                for s in &mut self.strategy_w {
                    s.add_oracle(&oracle);
                }
                for w in &mut self.stock_views {
                    w.add_oracle(&oracle);
                }
            }
            None => (),
        };
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

fn main() -> Result<()> {
    let _guard = w_log::init();
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let app_result = App::default().run(&mut terminal);
    ratatui::restore();
    Ok(app_result?)
}
