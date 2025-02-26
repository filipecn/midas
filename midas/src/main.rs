use std::io;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use dionysus::ta::match_indicator_from_text;
use dionysus::time::TimeUnit;
use market::MarketWindow;
use ratatui::{
    layout::{Constraint, Layout, Position},
    DefaultTerminal, Frame,
};

mod command_input;
mod common;
mod market;
mod stock_graph;
mod symbol_tabs;
mod wallet;

use command_input::CommandInput;
use common::Interactible;
use stock_graph::StockGraph;
use symbol_tabs::SymbolTabs;
use wallet::WalletWindow;

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
    market: MarketWindow,
}

impl App {
    pub fn new() -> Self {
        let mut s = App::default();
        s.add_tab("brownian");
        s.add_tab("BTCUSDT");
        s
    }
    fn add_tab(&mut self, symbol: &str) {
        let mut stock_graph = StockGraph::default();
        match stock_graph.load(symbol, &TimeUnit::Hour(1), 100) {
            Ok(()) => {
                self.stock_views.push(stock_graph);
                self.symbol_tabs.add(symbol);
            }
            Err(_) => (),
        }
    }

    fn set_resolution(&mut self, resolution_name: &str) {
        let resolution = TimeUnit::from_name(resolution_name);
        match self.stock_views[self.symbol_tabs.current()].set_resolution(&resolution) {
            Ok(()) => (),
            Err(_) => (), // TODO
        };
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        //  -----------------------------------------------------------
        // |           SYMBOLS                        |    BOOK        |
        // |------------------------------------------|                |
        // |                                          |----------------|
        // |             CHART                        |    SYMBOLS     |
        // |                                          |                |
        // |                                          |----------------|
        // |                                          |                |
        // |                                          |     WALLET     |
        // |------------------------------------------|                |
        // |            COMMAND                       |                |
        //  -----------------------------------------------------------
        let layout_0 = Layout::horizontal([Constraint::Percentage(80), Constraint::Min(0)]);

        let layout_00 = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(4),
        ]);

        let layout_01 = Layout::vertical([Constraint::Percentage(30), Constraint::Min(0)]);

        let [l00_area, l01_area] = layout_0.areas(frame.area());

        let [symbol_tabs_area, chart_area, command_area] = layout_00.areas(l00_area);
        let [wallet_area, market_area] = layout_01.areas(l01_area);

        frame.render_widget(&self.symbol_tabs, symbol_tabs_area);
        if self.stock_views.len() > self.symbol_tabs.current() {
            frame.render_widget(&self.stock_views[self.symbol_tabs.current()], chart_area);
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
        self.market.render(market_area, frame.buffer_mut());
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
                        if self.stock_views.len() > self.symbol_tabs.current() {
                            event_consumed &= self.stock_views[self.symbol_tabs.current()]
                                .handle_key_event(&key_event);
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
        let words: Vec<&str> = command.split(' ').collect();
        match words[0].to_uppercase().as_str() {
            "LOAD" => self.add_tab(words[1]),
            "GRAPH" => self.add_indicator(&words[1..]),
            "RES" => self.set_resolution(&words[1]),
            _ => (),
        };
    }

    fn add_indicator(&mut self, words: &[&str]) {
        match match_indicator_from_text(&words) {
            Some(indicator) => {
                self.stock_views[self.symbol_tabs.current()].add_indicator(indicator)
            }
            None => (),
        };
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let app_result = App::new().run(&mut terminal);
    ratatui::restore();
    Ok(app_result?)
}
