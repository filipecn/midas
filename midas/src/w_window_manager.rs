use crate::{
    common::popup_area,
    w_graph::GraphView,
    w_info::InfoWindow,
    w_interactible::InteractionEvent,
    w_market::MarketWindow,
    w_oracle::OracleWindow,
    w_order_book::OrderBookWindow,
    w_strategy::StrategyWindow,
    w_symbol_tabs::SymbolTabs,
    w_wallet::WalletWindow,
    w_window::{MidasWindow, WindowType},
};
use crossterm::event::{KeyCode, KeyEvent};
use dionysus::strategy::Strategy;
use ratatui::{layout::Rect, widgets::Clear, Frame};
use std::collections::HashMap;

pub struct WindowManager {
    pub windows: Vec<MidasWindow>,
    pub selected_window: Option<usize>,
    pub chart_id: HashMap<usize, usize>,
    key_codes: HashMap<KeyCode, (WindowType, bool)>,
    float_window: Option<usize>,
}

impl WindowManager {
    pub fn new() -> Self {
        let mut wm = WindowManager {
            windows: Vec::new(),
            chart_id: HashMap::new(),
            selected_window: None,
            key_codes: HashMap::new(),
            float_window: None,
        };
        wm.key_codes
            .insert(KeyCode::Char('a'), (WindowType::INPUT, false));
        wm.key_codes
            .insert(KeyCode::Char('l'), (WindowType::LOG, true));
        wm.key_codes
            .insert(KeyCode::Char('o'), (WindowType::ORACLE, true));
        wm.key_codes
            .insert(KeyCode::Char('?'), (WindowType::HELP, true));
        wm.key_codes
            .insert(KeyCode::Char('/'), (WindowType::INFO, true));

        wm.open(WindowType::LOG);
        wm.open(WindowType::STRATEGY);
        wm.open(WindowType::INPUT);
        wm.open(WindowType::WALLET);
        wm.open(WindowType::MARKET);
        wm.open(WindowType::ORACLE);
        wm.open(WindowType::ORDERBOOK);
        wm.open(WindowType::TABS);
        wm.open(WindowType::HELP);
        wm.open(WindowType::INFO);
        wm
    }

    pub fn open(&mut self, window_type: WindowType) {
        self.windows.push(MidasWindow::new(window_type));
    }

    pub fn open_chart(&mut self, midas_index: usize, graph: GraphView) {
        self.chart_id.insert(midas_index, self.windows.len());
        self.windows.push(MidasWindow::from_graph(graph));
        self.select_chart(midas_index);
    }

    pub fn select_chart(&mut self, midas_index: usize) {
        for (mi, i) in &self.chart_id {
            self.windows[*i].active = *mi == midas_index;
        }
    }

    pub fn set_area(&mut self, window_type: WindowType, area: Rect) {
        match window_type {
            WindowType::CHART => {
                for (_, i) in &self.chart_id {
                    self.windows[*i].area = area.clone();
                }
            }
            _ => self.windows[window_type as usize].area = area,
        }
    }

    pub fn tabs(&mut self) -> &mut SymbolTabs {
        self.windows[WindowType::TABS as usize]
            .content
            .downcast_mut::<SymbolTabs>()
            .unwrap()
    }

    pub fn wallet(&mut self) -> &mut WalletWindow {
        self.windows[WindowType::WALLET as usize]
            .content
            .downcast_mut::<WalletWindow>()
            .unwrap()
    }

    pub fn market(&mut self) -> &mut MarketWindow {
        self.windows[WindowType::MARKET as usize]
            .content
            .downcast_mut::<MarketWindow>()
            .unwrap()
    }

    pub fn strategy(&mut self) -> &mut StrategyWindow {
        self.windows[WindowType::STRATEGY as usize]
            .content
            .downcast_mut::<StrategyWindow>()
            .unwrap()
    }

    pub fn book(&mut self) -> &mut OrderBookWindow {
        self.windows[WindowType::ORDERBOOK as usize]
            .content
            .downcast_mut::<OrderBookWindow>()
            .unwrap()
    }

    pub fn info(&mut self) -> &mut InfoWindow {
        self.windows[WindowType::INFO as usize]
            .content
            .downcast_mut::<InfoWindow>()
            .unwrap()
    }

    pub fn open_oracle(&mut self, strategy: &Strategy) {
        self.windows[WindowType::ORACLE as usize]
            .content
            .downcast_mut::<OracleWindow>()
            .unwrap()
            .open(&strategy);
    }

    pub fn get_oracle(&self) -> Strategy {
        self.windows[WindowType::ORACLE as usize]
            .content
            .downcast_ref::<OracleWindow>()
            .unwrap()
            .strategy
            .clone()
    }

    pub fn chart(&mut self, midas_index: usize) -> Option<&mut GraphView> {
        if let Some(i) = self.chart_id.get(&midas_index) {
            self.windows[*i].content.downcast_mut::<GraphView>()
        } else {
            None
        }
    }

    pub fn select_window(&mut self, window_type: WindowType) {
        match window_type {
            WindowType::CHART => (),
            _ => self.selected_window = Some(window_type as usize),
        }
    }

    pub fn window_index(&self, window_type: WindowType) -> usize {
        match window_type {
            WindowType::CHART => 0,
            _ => window_type as usize,
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        for (i, window) in self.windows.iter_mut().enumerate() {
            let mut focus = false;
            if let Some(s) = self.selected_window {
                if s == i {
                    focus = true;
                }
            }
            if let Some(fw) = self.float_window {
                if fw == i {
                    continue;
                }
            }
            window.render(frame, focus, None);
        }
        if let Some(fw) = self.float_window {
            let area = popup_area(frame.area().clone(), 60, 80);
            frame.render_widget(Clear, area); //this clears out the background
            self.windows[fw].render(frame, true, Some(area));
        }
    }

    pub fn handle_key_event(&mut self, key_event: &KeyEvent) -> InteractionEvent {
        if let Some(i) = self.selected_window {
            let event = self.windows[i].handle_key_event(key_event, false);
            if event == InteractionEvent::Escape {
                self.selected_window = None;
                if let Some(fw) = self.float_window {
                    if fw == i {
                        self.float_window = None;
                    }
                }
                return InteractionEvent::Consumed;
            }
            return event;
        } else {
            for window in self.windows.iter_mut() {
                let event = window.handle_key_event(key_event, true);
                if event != InteractionEvent::None {
                    return event;
                }
            }
            if let Some((window_type, is_float)) = self.key_codes.get(&key_event.code) {
                let wt = window_type.clone();
                if *is_float {
                    self.float_window = Some(self.window_index(wt.clone()));
                }
                self.select_window(wt.clone());
                return InteractionEvent::WindowOpen(wt);
            }
        }
        match key_event.code {
            KeyCode::Char('q') => InteractionEvent::Escape,
            _ => InteractionEvent::None,
        }
    }
}
