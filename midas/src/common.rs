use crossterm::event::KeyEvent;
use ratatui::style::{
    palette::tailwind::{BLUE, GREEN, RED, SLATE},
    Color, Modifier, Style,
};

pub const NORMAL_BG: Color = SLATE.c950;
pub const NORMAL_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
pub const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
pub const PROFIT_COLOR: Color = GREEN.c500;
pub const LOSS_COLOR: Color = RED.c500;

pub trait Interactible {
    fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool;
    fn set_focus(&mut self, focus: bool);
}
