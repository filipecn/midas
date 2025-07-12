use crate::w_graph::GraphView;
use crate::w_interactible::{Interactible, InteractionEvent};
use crate::w_log::LogWindow;
use crate::w_market::MarketWindow;
use crate::w_order_book::OrderBookWindow;
use crate::w_strategy::StrategyWindow;
use crate::w_symbol_tabs::SymbolTabs;
use crate::w_wallet::WalletWindow;
use crate::{w_command::CommandInput, w_oracle::OracleWindow};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::Clear,
    Frame,
};

pub trait WindowContent {
    fn render(&mut self, frame: &mut Frame, area: Rect, focus: bool);
}

impl WindowContent for LogWindow {
    fn render(&mut self, frame: &mut Frame, area: Rect, focus: bool) {
        self.draw(area, frame.buffer_mut(), focus);
    }
}

impl<'a> WindowContent for OracleWindow<'a> {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.render(frame, area);
    }
}

impl WindowContent for StrategyWindow {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.render(area, frame.buffer_mut());
    }
}

impl WindowContent for WalletWindow {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.render(area, frame.buffer_mut());
    }
}

impl WindowContent for CommandInput {
    fn render(&mut self, frame: &mut Frame, area: Rect, focus: bool) {
        self.draw(area, frame.buffer_mut(), focus);
        frame.set_cursor_position(ratatui::layout::Position::new(
            // Draw the cursor at the current position in the input field.
            // This position is can be controlled via the left and right arrow key
            area.x + self.cursor_position() + 1,
            // Move one line down, from the border to the input line
            area.y + 1,
        ));
    }
}

impl WindowContent for MarketWindow {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.render(area, frame.buffer_mut());
    }
}

impl WindowContent for OrderBookWindow {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.render(area, frame.buffer_mut());
    }
}

impl WindowContent for SymbolTabs {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.draw(area, frame.buffer_mut());
    }
}

impl WindowContent for GraphView {
    fn render(&mut self, frame: &mut Frame, area: Rect, _focus: bool) {
        self.draw(area.clone(), frame.buffer_mut());
        let legend_area = area.clone();
        let vertical = Layout::vertical([Constraint::Percentage(15)]).flex(Flex::Start);
        let horizontal = Layout::horizontal([Constraint::Percentage(30)]).flex(Flex::End);
        let [legend_area] = vertical.areas(legend_area);
        let [legend_area] = horizontal.areas(legend_area);

        frame.render_widget(Clear, legend_area);
        self.draw_legend(legend_area, frame.buffer_mut());
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum WindowType {
    LOG = 0,
    STRATEGY = 1,
    INPUT = 2,
    WALLET = 3,
    MARKET = 4,
    ORACLE = 5,
    ORDERBOOK = 6,
    TABS = 7,
    CHART = 8,
}

pub struct MidasWindow {
    pub active: bool,
    pub window_type: WindowType,
    pub area: Rect,
    pub content: Box<dyn std::any::Any>,
}

macro_rules! create_window {
    ($t:expr, $s:tt) => {
        MidasWindow {
            active: true,
            window_type: $t,
            area: Rect::default(),
            content: Box::new($s::default()),
        }
    };
}

pub fn draw_content<T: WindowContent + std::any::Any>(
    content: &mut T,
    frame: &mut Frame,
    area: Rect,
    focus: bool,
) {
    content.render(frame, area, focus);
}

macro_rules! render {
    ($self:expr, $frame:tt, $t:tt, $focus:expr, $area:expr) => {
        draw_content::<$t>(
            $self.content.downcast_mut::<$t>().unwrap(),
            $frame,
            $area,
            $focus,
        )
    };
}

pub fn handle_key_event<T: Interactible + std::any::Any>(
    content: &mut T,
    key_event: &KeyEvent,
    global: bool,
) -> InteractionEvent {
    content.handle_key_event(key_event, global)
}

macro_rules! handle_key_event {
    ($self:expr, $key_event:tt, $t:tt, $global:tt) => {
        handle_key_event::<$t>(
            $self.content.downcast_mut::<$t>().unwrap(),
            $key_event,
            $global,
        )
    };
}

impl MidasWindow {
    pub fn from_graph(graph: GraphView) -> Self {
        Self {
            active: true,
            window_type: WindowType::CHART,
            area: Rect::default(),
            content: Box::new(graph),
        }
    }

    pub fn new(window_type: WindowType) -> Self {
        match window_type {
            WindowType::LOG => create_window!(window_type, LogWindow),
            WindowType::STRATEGY => create_window!(window_type, StrategyWindow),
            WindowType::INPUT => create_window!(window_type, CommandInput),
            WindowType::WALLET => create_window!(window_type, WalletWindow),
            WindowType::MARKET => create_window!(window_type, MarketWindow),
            WindowType::ORACLE => create_window!(window_type, OracleWindow),
            WindowType::ORDERBOOK => create_window!(window_type, OrderBookWindow),
            WindowType::TABS => create_window!(window_type, SymbolTabs),
            WindowType::CHART => create_window!(window_type, GraphView),
        }
    }

    pub fn render(&mut self, frame: &mut Frame, focus: bool, override_area: Option<Rect>) {
        if self.active {
            let area = if let Some(oa) = override_area {
                oa
            } else {
                self.area.clone()
            };
            match self.window_type {
                WindowType::LOG => render!(self, frame, LogWindow, focus, area),
                WindowType::MARKET => render!(self, frame, MarketWindow, focus, area),
                WindowType::TABS => render!(self, frame, SymbolTabs, focus, area),
                WindowType::INPUT => render!(self, frame, CommandInput, focus, area),
                WindowType::WALLET => render!(self, frame, WalletWindow, focus, area),
                WindowType::STRATEGY => render!(self, frame, StrategyWindow, focus, area),
                WindowType::ORDERBOOK => render!(self, frame, OrderBookWindow, focus, area),
                WindowType::ORACLE => render!(self, frame, OracleWindow, focus, area),
                WindowType::CHART => render!(self, frame, GraphView, focus, area),
            }
        }
    }

    pub fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent {
        if self.active {
            match self.window_type {
                WindowType::LOG => return handle_key_event!(self, key_event, LogWindow, global),
                WindowType::MARKET => {
                    return handle_key_event!(self, key_event, MarketWindow, global)
                }
                WindowType::TABS => return handle_key_event!(self, key_event, SymbolTabs, global),
                WindowType::INPUT => {
                    return handle_key_event!(self, key_event, CommandInput, global)
                }
                WindowType::WALLET => {
                    return handle_key_event!(self, key_event, WalletWindow, global)
                }
                WindowType::STRATEGY => {
                    return handle_key_event!(self, key_event, StrategyWindow, global)
                }
                WindowType::ORDERBOOK => {
                    return handle_key_event!(self, key_event, OrderBookWindow, global)
                }
                WindowType::ORACLE => {
                    return handle_key_event!(self, key_event, OracleWindow, global)
                }
                WindowType::CHART => return handle_key_event!(self, key_event, GraphView, global),
            };
        }
        InteractionEvent::None
    }
}
