use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use dionysus::binance::{BinanceMarket, MarketEvent};
use dionysus::finance::Token;
use dionysus::ta::match_indicator_from_text;
use dionysus::time::TimeUnit;
use dionysus::INFO;
use ratatui::{
    layout::{Constraint, Layout, Position},
    widgets::Clear,
    DefaultTerminal, Frame,
};
use slog::slog_info;
use slog_scope;
use std::io;

mod common;
mod w_command;
mod w_graph;
mod w_log;
mod w_market;
mod w_order_book;
mod w_symbol_tabs;
mod w_wallet;

use common::Interactible;
use w_command::CommandInput;
use w_graph::StockGraph;
use w_log::LogWindow;
use w_market::MarketWindow;
use w_order_book::OrderBookWindow;
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
    command: CommandInput,
    input_mode: InputMode,
    wallet: WalletWindow,
    market_window: MarketWindow,
    market: BinanceMarket,
    order_book_w: OrderBookWindow,
    log_w: LogWindow,
}

impl App {
    pub fn new() -> Self {
        let mut s = App::default();
        s.add_tab("BTC", "usdt");
        s
    }
    fn add_tab(&mut self, symbol: &str, currency: &str) {
        let mut stock_graph = StockGraph::default();
        let resolution = TimeUnit::Hour(1);
        let pair = Token::pair(
            String::from(symbol).to_uppercase().as_str(),
            String::from(currency).to_uppercase().as_str(),
        );
        match stock_graph.load(&pair, &resolution, 100) {
            Ok(()) => {
                self.market.order_book_service(&pair);
                self.market.kline_service(&pair, &resolution);
                self.stock_views.push(stock_graph);
                self.symbol_tabs.add(&pair);
            }
            Err(_) => (),
        }
    }

    fn set_resolution(&mut self, resolution_name: &str) {
        let resolution = TimeUnit::from_name(resolution_name);
        if let Some((curr_index, curr_token)) = self.symbol_tabs.current() {
            match self.stock_views[curr_index].set_resolution(&resolution) {
                Ok(()) => self.market.kline_service(&curr_token, &resolution),
                Err(_) => (),
            };
        }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
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
                            for view in &mut self.stock_views {
                                view.update_with(&token, &sample);
                            }
                        }
                        MarketEvent::Ticks(ticks) => self.market_window.update_with(ticks),
                        MarketEvent::OrderBook(book) => {
                            if let Some((_, current_token)) = self.symbol_tabs.current() {
                                if current_token == book.token {
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
        // |       |                                          |----------------|
        // |       |                                          |                |
        // |       |------------------------------------------|    LOG         |   c
        // |       |            COMMAND                       |                |
        //  ------- -----------------------------------------------------------
        let layout_012 = Layout::horizontal([
            Constraint::Percentage(18),
            Constraint::Percentage(64),
            Constraint::Percentage(18),
        ]);

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

        let [symbol_tabs_area, chart_area, command_area] = layout_1_abc.areas(l1_area);
        let [wallet_area, market_area, log_area] = layout_2_abc.areas(l2_area);

        frame.render_widget(&self.symbol_tabs, symbol_tabs_area);
        if let Some((curr_index, _)) = self.symbol_tabs.current() {
            frame.render_widget(&self.stock_views[curr_index], chart_area);
            if let Some(legend_area) = self.stock_views[curr_index].legend_area(chart_area) {
                frame.render_widget(Clear, legend_area);
                self.stock_views[curr_index].draw_legend(legend_area, frame.buffer_mut());
            }
        }

        frame.render_widget(&self.command, command_area);
        frame.set_cursor_position(Position::new(
            // Draw the cursor at the current position in the input field.
            // This position is can be controlled via the left and right arrow key
            command_area.x + self.command.cursor_position() + 1,
            // Move one line down, from the border to the input line
            command_area.y + 1,
        ));
        self.wallet.render(wallet_area, frame.buffer_mut());
        self.market_window.render(market_area, frame.buffer_mut());
        self.order_book_w.render(l0_area, frame.buffer_mut());
        frame.render_widget(&self.log_w, log_area);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) => match self.input_mode {
                InputMode::Normal => {
                    if key_event.code == KeyCode::Char('a') {
                        self.input_mode = InputMode::Command;
                        self.command.set_focus(true);
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
                    self.command.set_focus(false);
                    if key_event.code == KeyCode::Enter {
                        self.run_command(self.command.text().as_str());
                        self.command.clear();
                        self.input_mode = InputMode::Normal;
                    } else if key_event.code == KeyCode::Esc {
                        self.input_mode = InputMode::Normal;
                        self.command.clear();
                    } else {
                        self.command.set_focus(true);
                        self.command.handle_key_event(&key_event);
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
        INFO!("cmd: {:?}", command);
        let words: Vec<&str> = command.split(' ').collect();
        match words[0].to_uppercase().as_str() {
            "LOAD" => self.add_tab(words[1], if words.len() > 2 { words[2] } else { "usdt" }),
            "GRAPH" => self.add_indicator(&words[1..]),
            "RES" => self.set_resolution(&words[1]),
            _ => (),
        };
    }

    fn add_indicator(&mut self, words: &[&str]) {
        if let Some((curr_index, _)) = self.symbol_tabs.current() {
            match match_indicator_from_text(&words) {
                Some(indicator) => self.stock_views[curr_index].add_indicator(indicator),
                None => (),
            };
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

fn main() -> Result<()> {
    let _guard = w_log::init();
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let app_result = App::new().run(&mut terminal);
    ratatui::restore();
    Ok(app_result?)
}
