use crate::{
    w_command::CommandInput, w_graph::GraphView, w_help::HelpWindow, w_info::InfoWindow,
    w_log::LogWindow, w_market::MarketWindow, w_oracle::OracleWindow, w_order::OrderWindow,
    w_order_book::OrderBookWindow, w_strategy::StrategyWindow, w_symbol_tabs::SymbolTabs,
    w_wallet::WalletWindow, w_window::WindowType,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_prompts::State;

#[derive(Eq, PartialEq, Debug)]
pub enum InteractionEvent {
    None,
    Consumed,
    RunCommand(String),
    Escape,
    SymbolSelect(usize),
    WindowOpen(WindowType),
    UpdateStrategy,
}

impl InteractionEvent {
    pub fn from_bool(consumed: bool) -> Self {
        if consumed {
            InteractionEvent::Consumed
        } else {
            InteractionEvent::None
        }
    }
}

pub trait Interactible {
    fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent;
}

impl Interactible for LogWindow {
    fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent {
        if !global {
            match (key_event.code, key_event.modifiers) {
                (KeyCode::Esc, _) => InteractionEvent::Escape,
                (KeyCode::Char('l'), _) => InteractionEvent::Escape,
                _ => InteractionEvent::None,
            }
        } else {
            InteractionEvent::None
        }
    }
}

impl Interactible for MarketWindow {
    fn handle_key_event(&mut self, _key_event: &KeyEvent, _global: bool) -> InteractionEvent {
        InteractionEvent::None
    }
}

impl Interactible for GraphView {
    fn handle_key_event(&mut self, key_event: &KeyEvent, _global: bool) -> InteractionEvent {
        let mut consumed = true;
        if key_event.kind == KeyEventKind::Press {
            match key_event.modifiers {
                KeyModifiers::CONTROL => self.zooming = true,
                _ => self.zooming = false,
            };
            match key_event.code {
                KeyCode::Left => {
                    if self.zooming {
                        self.zoom(-0.05, 0.0);
                    } else {
                        self.pan(-1.0, 0.0);
                    }
                }
                KeyCode::Right => {
                    if self.zooming {
                        self.zoom(0.05, 0.0);
                    } else {
                        self.pan(1.0, 0.0);
                    }
                }
                KeyCode::Up => {
                    if self.zooming {
                        self.zoom(0.0, 0.05);
                    } else {
                        self.pan(0.0, 1.0);
                    }
                }
                KeyCode::Down => {
                    if self.zooming {
                        self.zoom(0.0, -0.05);
                    } else {
                        self.pan(0.0, -1.0);
                    }
                }
                _ => consumed = false,
            };
        }
        InteractionEvent::from_bool(consumed)
    }
}

impl Interactible for CommandInput {
    fn handle_key_event(
        &mut self,
        key_event: &crossterm::event::KeyEvent,
        global: bool,
    ) -> InteractionEvent {
        if global {
            InteractionEvent::None
        } else {
            let mut consumed = true;
            if key_event.kind == KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Char(to_insert) => self.enter_char(to_insert),
                    KeyCode::Backspace => self.delete_char(),
                    KeyCode::Left => self.move_cursor_left(),
                    KeyCode::Right => self.move_cursor_right(),
                    KeyCode::Enter => {
                        let text = self.text();
                        self.clear();
                        return InteractionEvent::RunCommand(text);
                    }
                    KeyCode::Esc => {
                        self.clear();
                        return InteractionEvent::Escape;
                    }
                    _ => consumed = false,
                };
            }
            InteractionEvent::from_bool(consumed)
        }
    }
}

impl Interactible for SymbolTabs {
    fn handle_key_event(
        &mut self,
        key_event: &crossterm::event::KeyEvent,
        _global: bool,
    ) -> InteractionEvent {
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Char('o') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                        self.previous_item();
                    } else {
                        self.next_item();
                    }
                }
                KeyCode::Char('t') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                        self.previous();
                    } else {
                        self.next();
                    }
                }
                _ => return InteractionEvent::None,
            };
            if let Some(midas_index) = self.current_midas_index() {
                return InteractionEvent::SymbolSelect(midas_index);
            }
        }
        InteractionEvent::None
    }
}

impl<'a> Interactible for OracleWindow<'a> {
    fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent {
        if global {
            InteractionEvent::None
        } else {
            match (key_event.code, key_event.modifiers) {
                (KeyCode::Enter, _) => self.submit(),
                (KeyCode::Tab, KeyModifiers::NONE) => self.focus_next(),
                (KeyCode::BackTab, KeyModifiers::SHIFT) => self.focus_prev(),
                (KeyCode::Esc, _) => {
                    self.close();
                    return InteractionEvent::Escape;
                }
                (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                    return InteractionEvent::UpdateStrategy
                }
                _ => self.current().handle_key_event(key_event.clone()),
            };
            InteractionEvent::None
        }
    }
}

impl Interactible for WalletWindow {
    fn handle_key_event(&mut self, _key_event: &KeyEvent, _global: bool) -> InteractionEvent {
        InteractionEvent::None
    }
}

impl Interactible for StrategyWindow {
    fn handle_key_event(&mut self, _key_event: &KeyEvent, _global: bool) -> InteractionEvent {
        InteractionEvent::None
    }
}

impl Interactible for OrderBookWindow {
    fn handle_key_event(&mut self, _key_event: &KeyEvent, _global: bool) -> InteractionEvent {
        InteractionEvent::None
    }
}

impl Interactible for OrderWindow {
    fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent {
        if !global {
            match (key_event.code, key_event.modifiers) {
                (KeyCode::Esc, _) => InteractionEvent::Escape,
                (KeyCode::Char('O'), _) => InteractionEvent::Escape,
                _ => InteractionEvent::None,
            }
        } else {
            InteractionEvent::None
        }
    }
}

impl Interactible for HelpWindow {
    fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent {
        if !global {
            match (key_event.code, key_event.modifiers) {
                (KeyCode::Esc, _) => InteractionEvent::Escape,
                (KeyCode::Char('?'), _) => InteractionEvent::Escape,
                _ => InteractionEvent::None,
            }
        } else {
            InteractionEvent::None
        }
    }
}

impl Interactible for InfoWindow {
    fn handle_key_event(&mut self, key_event: &KeyEvent, global: bool) -> InteractionEvent {
        if !global {
            match (key_event.code, key_event.modifiers) {
                (KeyCode::Esc, _) => InteractionEvent::Escape,
                (KeyCode::Char('/'), _) => InteractionEvent::Escape,
                _ => InteractionEvent::None,
            }
        } else {
            InteractionEvent::None
        }
    }
}
