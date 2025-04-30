use clap::Parser;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use dionysus::backtest::Backtest;
use dionysus::finance::Token;
use dionysus::historical_data::HistoricalData;
use dionysus::indicators::match_indicator_from_text;
use dionysus::time::TimeUnit;
use dionysus::ERROR;
use ratatui::{
    layout::{Constraint, Flex, Layout, Position},
    widgets::Clear,
    DefaultTerminal, Frame,
};
use slog::slog_error;
use slog_scope;
use std::collections::HashMap;
use std::io;

mod common;
mod g_book;
mod g_common;
mod g_curve;
mod g_element;
mod g_indicators;
mod g_samples;
mod g_strategy;
mod midas;
mod w_command;
mod w_graph;
mod w_interactible;
mod w_log;
mod w_market;
mod w_order_book;
mod w_strategy;
mod w_symbol_tabs;
mod w_wallet;

use common::popup_area;
use midas::{Midas, MidasEvent};
use w_command::CommandInput;
use w_graph::GraphView;
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

pub struct App {
    midas: Midas,
    exit: bool,
    graph_views: HashMap<usize, GraphView>,
    symbol_tabs: SymbolTabs,
    command_w: CommandInput,
    input_mode: InputMode,
    wallet_w: WalletWindow,
    market_w: MarketWindow,
    order_book_w: OrderBookWindow,
    log_w: LogWindow,
    strategy_w: StrategyWindow,
    show_log: bool,
    state_file: String,
    backtests: HashMap<usize, Backtest>,
}

impl App {
    pub fn new(keys_file: &str, use_test_api: bool) -> App {
        App {
            midas: Midas::new(keys_file, use_test_api),
            exit: false,
            graph_views: HashMap::new(),
            symbol_tabs: SymbolTabs::default(),
            command_w: CommandInput::default(),
            input_mode: InputMode::default(),
            wallet_w: WalletWindow::default(),
            market_w: MarketWindow::default(),
            order_book_w: OrderBookWindow::default(),
            log_w: LogWindow::default(),
            strategy_w: StrategyWindow::default(),
            show_log: false,
            state_file: String::from("state.json"),
            backtests: HashMap::new(),
        }
    }

    fn open_tab(&mut self, midas_index: usize) {
        if let Some(c) = self.midas.get(midas_index) {
            if c.token.is_pair() {
                if let Some(samples) = self.midas.get_history(midas_index) {
                    let mut graph = GraphView::default();
                    graph.set_strategy(&c.strategy);
                    graph.set_data(samples);
                    graph.reset_camera();
                    self.graph_views.insert(midas_index, graph);
                    self.symbol_tabs.add(&c.token, midas_index);
                }
            }
        }
    }

    fn add_tab(&mut self, symbol: &str, currency: &str) {
        let pair = Token::pair(
            String::from(symbol).to_uppercase().as_str(),
            String::from(currency).to_uppercase().as_str(),
        );

        if let Some(index) = self.midas.add_token(&pair) {
            self.open_tab(index);
        }
    }

    fn set_history_size(&mut self, n: usize) {
        if let Some((midas_index, pair)) = self.symbol_tabs.current() {
            if let Some(graph_view) = self.graph_views.get_mut(&midas_index) {
                let mut time_window = graph_view.time_window.clone();
                time_window.count = n as i64;
                match self.midas.market.fetch_last(&pair, &time_window) {
                    Ok(samples) => graph_view.set_data(samples),
                    Err(e) => ERROR!("{:?}", e),
                }
            }
        }
    }

    fn set_resolution(&mut self, resolution_name: &str) {
        if let Some((midas_index, curr_token)) = self.symbol_tabs.current() {
            if let Some(c) = self.midas.get(midas_index) {
                let mut s = c.strategy.clone();
                s.duration.resolution = TimeUnit::from_name(resolution_name);
                self.midas.set_strategy(midas_index, &s);
                if let Some(graph_view) = self.graph_views.get_mut(&midas_index) {
                    match self.midas.market.get_last(&curr_token, &s.duration) {
                        Ok(samples) => graph_view.set_data(samples),
                        Err(e) => ERROR!("{:?}", e),
                    }
                }
            }
        }
    }

    fn update_graph(&mut self, midas_index: usize) {
        if let Some((index, curr_token)) = self.symbol_tabs.current() {
            if index == midas_index {
                if let Some(graph_view) = self.graph_views.get_mut(&midas_index) {
                    let time_window = graph_view.time_window.clone();
                    match self.midas.market.get_last(&curr_token, &time_window) {
                        Ok(samples) => {
                            graph_view.set_data(samples);
                        }
                        Err(e) => ERROR!("{:?}", e),
                    };
                }
            }
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.midas.init(&self.state_file);
        for midas_index in 0..self.midas.hesperides.len() {
            self.open_tab(midas_index);
        }
        //self.run_command("oracle mean-reversion 10");
        //self.run_command("oracle macd-crossover 12 26 9");
        //self.run_command("oracle macd-zero-cross 12 26 9");
        //self.run_command("oracle ema-cross 50 200");

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

                for event in self.midas.touch() {
                    match event {
                        MidasEvent::KLineUpdate(midas_index) => {
                            self.update_graph(midas_index);
                        }
                        MidasEvent::BookUpdate(token) => {
                            if let Some((midas_index, current_token)) = self.symbol_tabs.current() {
                                if current_token == token {
                                    if let Some(book) = self.midas.get_book(&token) {
                                        if let Some(graph_view) =
                                            self.graph_views.get_mut(&midas_index)
                                        {
                                            graph_view.book_w.set_book(&book);
                                        }
                                        self.order_book_w.update_with(book);
                                    }
                                }
                            }
                        }
                    };
                }

                self.wallet_w
                    .update(self.midas.get_balance(), &self.midas.ticks);

                self.market_w.update_with(self.midas.ticks.clone());

                self.strategy_w.update(
                    &self.midas,
                    &self.backtests,
                    self.symbol_tabs.current_midas_index(),
                );
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        //     0                     1                               2
        //  -------------------------------------------------------------------
        // |                   SYMBOLS                                         |
        // |-------------------------------------------------------------------|
        // |       |                                          |    WALLET      |   a
        // |ORACLES|             CHART                        |----------------|
        // |       |                                          |    MARKET      |
        // |       |                                          |                |   b
        // |------ |                                          |----------------|
        // |       |                                          |                |
        // |BOOK   |------------------------------------------|    LOG         |   c
        // |       |            COMMAND                       |                |
        //  ------- -----------------------------------------------------------

        // a-SYMBOLS  b-rest
        let layout_ab = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);

        // 0-book 1-chart 2-wallet
        let layout_b_012 = Layout::horizontal([
            Constraint::Percentage(18),
            Constraint::Percentage(64),
            Constraint::Percentage(18),
        ]);

        // a-book b-oracle
        let layout_b_0_ab =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);

        // a-chart b-command
        let layout_b_1_ab = Layout::vertical([Constraint::Min(0), Constraint::Length(4)]);

        // a-wallet b-market c-log
        let layout_b_2_abc = Layout::vertical([
            Constraint::Percentage(30),
            Constraint::Min(0),
            Constraint::Percentage(20),
        ]);

        let [symbol_tabs_area, b_area] = layout_ab.areas(frame.area());

        let [l0_area, l1_area, l2_area] = layout_b_012.areas(b_area);

        let [strategy_area, book_area] = layout_b_0_ab.areas(l0_area);

        let [chart_area, command_area] = layout_b_1_ab.areas(l1_area);

        let [wallet_area, market_area, log_area] = layout_b_2_abc.areas(l2_area);

        frame.render_widget(&self.symbol_tabs, symbol_tabs_area);
        if let Some((midas_index, _)) = self.symbol_tabs.current() {
            if let Some(graph_view) = self.graph_views.get(&midas_index) {
                frame.render_widget(graph_view, chart_area);

                let legend_area = chart_area.clone();
                let vertical = Layout::vertical([Constraint::Percentage(15)]).flex(Flex::Start);
                let horizontal = Layout::horizontal([Constraint::Percentage(30)]).flex(Flex::End);
                let [legend_area] = vertical.areas(legend_area);
                let [legend_area] = horizontal.areas(legend_area);

                frame.render_widget(Clear, legend_area);
                graph_view.draw_legend(legend_area, frame.buffer_mut());
            }
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

        self.strategy_w.render(strategy_area, frame.buffer_mut());

        self.order_book_w.render(book_area, frame.buffer_mut());

        if self.show_log {
            let area = popup_area(frame.area().clone(), 60, 80);
            frame.render_widget(Clear, area); //this clears out the background
            frame.render_widget(&self.log_w, area);
        } else {
            frame.render_widget(&self.log_w, log_area);
        }
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
                        if let Some((midas_index, _)) = self.symbol_tabs.current() {
                            if let Some(graph_view) = self.graph_views.get_mut(&midas_index) {
                                event_consumed &= graph_view.handle_key_event(&key_event);
                                if event_consumed {
                                    graph_view.set_focus(true);
                                }
                            }
                        }
                        if !event_consumed {
                            match key_event.code {
                                KeyCode::Char('q') => self.exit(),
                                KeyCode::Char('l') => self.show_log = !self.show_log,
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
            "SAVE" => self.midas.save_state(&self.state_file),
            "HIST" => {
                if let Ok(n) = words[1].parse::<usize>() {
                    self.set_history_size(n);
                }
            }
            "BACKTEST" => self.run_backtest(&words[1..]),
            _ => (),
        };
    }

    fn add_indicator(&mut self, words: &[&str]) {
        if let Some((midas_index, _)) = self.symbol_tabs.current() {
            if let Some(graph_view) = self.graph_views.get_mut(&midas_index) {
                match match_indicator_from_text(&words) {
                    Some(indicator) => graph_view.add_indicator(&indicator),
                    None => (),
                };
            }
        }
    }

    fn add_oracle(&mut self, words: &[&str]) {
        //match match_oracle_from_text(&words) {
        //    Some(oracle) => {
        //        for s in &mut self.strategy_w {
        //            s.add_oracle(&oracle);
        //        }
        //        for w in &mut self.graph_views {
        //            w.add_oracle(&oracle);
        //        }
        //    }
        //    None => (),
        //};
    }

    fn run_backtest(&mut self, _words: &[&str]) {
        if let Some((midas_index, _)) = self.symbol_tabs.current() {
            if let Some(graph_view) = self.graph_views.get_mut(&midas_index) {
                let bt = self
                    .midas
                    .run_backtest(midas_index, &graph_view.time_window);
                graph_view.set_backtest(&bt);
                self.backtests.insert(midas_index, bt.clone());
            }
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File containing the secret and API keys
    #[arg(short, long)]
    keys: String,

    /// Number of times to greet
    #[arg(short, long, default_value_t = false)]
    test: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let _guard = w_log::init();
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let app_result = App::new(args.keys.as_str(), args.test).run(&mut terminal);
    ratatui::restore();
    Ok(app_result?)
}
