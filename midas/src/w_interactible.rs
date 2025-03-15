use crate::{w_command::CommandInput, w_graph::StockGraph, w_symbol_tabs::SymbolTabs};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub trait Interactible {
    fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool;
    fn set_focus(&mut self, focus: bool);
}

impl Interactible for StockGraph {
    fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool {
        let mut consumed = true;
        if key_event.kind == KeyEventKind::Press {
            match key_event.modifiers {
                KeyModifiers::CONTROL => self.zooming = true,
                _ => self.zooming = false,
            };
            match key_event.code {
                KeyCode::Char('i') => {
                    self.selected_indicator_set =
                        (self.selected_indicator_set + 1) % self.indicators.len();
                }
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
        consumed
    }
    fn set_focus(&mut self, focus: bool) {
        self.focus = focus;
    }
}

impl Interactible for CommandInput {
    fn handle_key_event(&mut self, key_event: &crossterm::event::KeyEvent) -> bool {
        let mut consumed = true;
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                //KeyCode::Enter => self.submit_message(),
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Backspace => self.delete_char(),
                KeyCode::Left => self.move_cursor_left(),
                KeyCode::Right => self.move_cursor_right(),
                _ => consumed = false,
            };
        }
        consumed
    }
    fn set_focus(&mut self, focus: bool) {
        self.focus = focus;
    }
}

impl Interactible for SymbolTabs {
    fn handle_key_event(&mut self, key_event: &crossterm::event::KeyEvent) -> bool {
        let mut consumed = true;
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Char('t') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                        self.previous();
                    } else {
                        self.next();
                    }
                }
                _ => consumed = false,
            };
        }
        consumed
    }
    fn set_focus(&mut self, _focus: bool) {}
}
