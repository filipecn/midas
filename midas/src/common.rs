use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{
        palette::tailwind::{BLUE, GREEN, RED, SLATE, YELLOW},
        Color, Modifier, Style, Stylize,
    },
    text::{self, Line},
    widgets::{
        Block, BorderType, Borders, HighlightSpacing, List, ListItem, ListState, StatefulWidget,
    },
};

use dionysus::counselor::Signal;

pub const NORMAL_FG: Color = BLUE.c50;
pub const NORMAL_BG: Color = SLATE.c950;
pub const _NORMAL_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
pub const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
pub const PROFIT_COLOR: Color = GREEN.c500;
pub const LOSS_COLOR: Color = RED.c500;
pub const WINDOW_BORDER: Color = RED.c50;
pub const SELECTED_WINDOW_BORDER: Color = YELLOW.c50;

pub fn color_from_signal(signal: &Signal) -> Color {
    match signal {
        Signal::Buy => RED.c200,
        Signal::Sell => GREEN.c200,
        Signal::None => BLUE.c50,
    }
}

pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn block(title: &str) -> Block {
    Block::new()
        .title(text::Line::raw(title)) //.centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
    //.border_set(symbols::border::EMPTY)
    //.border_style(NORMAL_HEADER_STYLE)
    //.bg(NORMAL_BG)
    //Block::bordered().title(text::Line::from(title).cyan().bold().centered())
}

pub fn focus_style(focus: bool) -> Style {
    Style::default().fg(if focus { Color::Yellow } else { Color::White })
}

pub struct ListWindow<T> {
    state: ListState,
    pub items: Vec<T>,
}

impl<T> Default for ListWindow<T> {
    fn default() -> Self {
        Self {
            state: ListState::default(),
            items: Vec::default(),
        }
    }
}

impl<T> ListWindow<T> {
    pub fn render<F>(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        block: Block,
        f: F,
    ) where
        Self: Sized,
        F: Fn(&T) -> Line,
    {
        let mut list_items: Vec<ListItem> = Vec::new();
        for (_, item) in self.items.iter().enumerate() {
            list_items.push(ListItem::new(f(item)));
        }

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(list_items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}
