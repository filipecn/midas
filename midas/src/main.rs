use clap::Parser;
use color_eyre::Result;
use crossterm::event::{self, Event};
use dionysus::backtest::Backtest;
use dionysus::finance::{Order, OrderType, Side, TimeInForce, Token};
use dionysus::historical_data::HistoricalData;
use dionysus::indicators::match_indicator_from_text;
use dionysus::strategy::Strategy;
use dionysus::time::{Date, TimeUnit};
use dionysus::trader::Trader;
use dionysus::ERROR;
use ratatui::{
    layout::{Constraint, Layout},
    DefaultTerminal, Frame,
};
use slog::slog_error;
use slog_scope;
use std::collections::HashMap;
use std::io;
use w_window::WindowType;
use w_window_manager::WindowManager;

mod common;
mod g_book;
mod g_common;
mod g_curve;
mod g_element;
mod g_indicators;
mod g_samples;
mod g_strategy;
mod midas;
mod w_backtest;
mod w_command;
mod w_graph;
mod w_help;
mod w_info;
mod w_interactible;
mod w_log;
mod w_market;
mod w_oracle;
mod w_order;
mod w_order_book;
mod w_strategy;
mod w_symbol_tabs;
mod w_wallet;
mod w_window;
mod w_window_manager;

use midas::{Midas, MidasEvent};
use w_graph::GraphView;
use w_interactible::InteractionEvent;

pub struct App {
    midas: Midas,
    exit: bool,
    state_file: String,
    backtests: HashMap<usize, Backtest>,
    window_manager: WindowManager,
}

impl App {
    pub fn new(keys_file: &str, use_test_api: bool) -> App {
        App {
            midas: Midas::new(keys_file, use_test_api),
            exit: false,
            state_file: String::from("state.json"),
            backtests: HashMap::new(),
            window_manager: WindowManager::new(),
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
                    self.window_manager.tabs().add(&c.token, midas_index);
                    self.window_manager.open_chart(midas_index, graph);
                    self.run_backtest();
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
        if let Some((midas_index, pair)) = self.window_manager.tabs().current() {
            if let Some(graph_view) = self.window_manager.chart(midas_index) {
                let mut time_window = graph_view.time_window.clone();
                time_window.count = n as i64;
                match self.midas.market.fetch_last(&pair, &time_window) {
                    Ok(samples) => {
                        graph_view.set_data(samples);
                        self.run_backtest();
                    }
                    Err(e) => ERROR!("{:?}", e),
                }
            }
        }
    }

    fn set_resolution(&mut self, resolution_name: &str) {
        if let Some((midas_index, curr_token)) = self.window_manager.tabs().current() {
            if let Some(c) = self.midas.get(midas_index) {
                let mut s = c.strategy.clone();
                s.duration.resolution = TimeUnit::from_name(resolution_name);
                self.midas.set_strategy(midas_index, &s);
                if let Some(graph_view) = self.window_manager.chart(midas_index) {
                    match self.midas.market.get_last(&curr_token, &s.duration) {
                        Ok(samples) => {
                            graph_view.set_data(samples);
                            self.run_backtest();
                        }
                        Err(e) => ERROR!("{:?}", e),
                    }
                }
            }
        }
    }

    fn update_strategy(&mut self, strategy: &Strategy) {
        if let Some((midas_index, token)) = self.window_manager.tabs().current() {
            self.midas.set_strategy(midas_index, strategy);
            if let Some(graph_view) = self.window_manager.chart(midas_index) {
                match self.midas.market.get_last(&token, &strategy.duration) {
                    Ok(samples) => {
                        graph_view.set_data(samples);
                        self.run_backtest();
                    }
                    Err(e) => ERROR!("{:?}", e),
                }
            }
        }
    }

    fn open_oracle(&mut self) {
        if let Some(midas_index) = self.window_manager.tabs().current_midas_index() {
            if let Some(c) = self.midas.get(midas_index) {
                self.window_manager.open_oracle(&c.strategy);
            }
        }
    }

    fn open_info(&mut self) {
        let mut token: Option<Token> = None;
        if let Some(midas_index) = self.window_manager.tabs().current_midas_index() {
            if let Some(c) = self.midas.get(midas_index) {
                token = Some(c.token.clone());
            }
        }
        if let Some(t) = token {
            self.window_manager
                .info()
                .update(&mut self.midas.exchange, &t);
        }
    }

    fn open_order(&mut self) {
        self.window_manager.order().update(&self.midas.wallet);
    }

    fn update_graph(&mut self, midas_index: usize) {
        if let Some(token) = self.midas.get_token(midas_index) {
            if let Some(graph_view) = self.window_manager.chart(midas_index) {
                let time_window = graph_view.time_window.clone();
                match self.midas.market.get_last(&token, &time_window) {
                    Ok(samples) => {
                        graph_view.set_data(samples);
                    }
                    Err(e) => ERROR!("{:?}", e),
                };
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
                            if let Some((midas_index, current_token)) =
                                self.window_manager.tabs().current()
                            {
                                if current_token == token {
                                    if let Some(book) = self.midas.get_book(&token) {
                                        if let Some(graph_view) =
                                            self.window_manager.chart(midas_index)
                                        {
                                            graph_view.book_w.set_book(&book);
                                        }
                                        self.window_manager.book().update_with(book);
                                    }
                                }
                            }
                        }
                    };
                }

                self.window_manager
                    .wallet()
                    .update(self.midas.get_balance(), &self.midas.ticks);

                self.window_manager
                    .market()
                    .update_with(self.midas.ticks.clone());

                let midas_index = self.window_manager.tabs().current_midas_index();
                self.window_manager
                    .strategy()
                    .update(&self.midas, &self.backtests, midas_index);
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
            Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)]);

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

        self.window_manager.set_area(WindowType::LOG, log_area);
        self.window_manager.set_area(WindowType::CHART, chart_area);
        self.window_manager
            .set_area(WindowType::INPUT, command_area);
        self.window_manager
            .set_area(WindowType::ORDERBOOK, book_area);
        self.window_manager
            .set_area(WindowType::TABS, symbol_tabs_area);
        self.window_manager
            .set_area(WindowType::STRATEGY, strategy_area);
        self.window_manager
            .set_area(WindowType::MARKET, market_area);
        self.window_manager
            .set_area(WindowType::WALLET, wallet_area);

        let midas_index = self.window_manager.tabs().current_midas_index().unwrap();
        self.window_manager.select_chart(midas_index);

        self.window_manager.render(frame);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) => match self.window_manager.handle_key_event(&key_event) {
                InteractionEvent::Escape => self.exit(),
                InteractionEvent::RunCommand(command) => self.run_command(command.as_str()),
                InteractionEvent::SymbolSelect(midas_index) => {
                    self.window_manager.select_chart(midas_index)
                }
                InteractionEvent::UpdateStrategy => {
                    self.update_strategy(&self.window_manager.get_oracle())
                }
                InteractionEvent::WindowOpen(window_type) => match window_type {
                    WindowType::ORACLE => self.open_oracle(),
                    WindowType::INFO => self.open_info(),
                    WindowType::ORDER => self.open_order(),
                    _ => (),
                },
                _ => (),
            },
            _ => (),
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
            "BACKTEST" => self.run_backtest(),
            "BUY" => self.create_order(Side::Buy),
            "SELL" => self.create_order(Side::Sell),
            _ => (),
        };
    }

    fn add_indicator(&mut self, words: &[&str]) {
        if let Some((midas_index, _)) = self.window_manager.tabs().current() {
            if let Some(graph_view) = self.window_manager.chart(midas_index) {
                ERROR!("add indicator {:?}", midas_index);
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

    fn run_backtest(&mut self) {
        for (midas_index, _) in self.midas.hesperides.iter().enumerate() {
            if let Some(graph_view) = self.window_manager.chart(midas_index) {
                let bt = self
                    .midas
                    .run_backtest(midas_index, &graph_view.time_window);
                graph_view.set_backtest(&bt);
                self.backtests.insert(midas_index, bt.clone());
            }
        }
    }

    fn create_order(&mut self, signal: Side) {
        if let Some((_, token)) = self.window_manager.tabs().current() {
            // get token info
            let token_info = self.midas.exchange.get(&token);
            if let Some(book) = self.midas.get_book(&token) {
                if let Some(quote) = book.quote() {
                    // get symbol info
                    let price = quote.ask.unwrap_or(0.0);
                    // consider 1 dollar
                    let shares = 10.0 / price;
                    if shares < token_info.lot_min_qty {
                        let cost = price * token_info.lot_min_qty;
                        ERROR!("min cost is: {}", cost);
                    }
                    match signal {
                        Side::Buy => {
                            let order = Order {
                                index: 0,
                                position_index: None,
                                id: None,
                                token: quote.token.clone(),
                                date: Date::now(),
                                quantity: (shares * 100.0).round() / 100.0,
                                side: Side::Buy,
                                price,
                                stop_price: None,
                                order_type: OrderType::Limit,
                                tif: TimeInForce::default(),
                            };
                            ERROR!("{:?}", order);
                            ERROR!("{:?}", self.midas.wallet.buy_order(&order));
                        }
                        Side::Sell => (),
                        _ => (),
                    }
                }
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
