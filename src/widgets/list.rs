use std::collections::BTreeMap;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use ratatui_interact::{
    components::ListPickerState,
    utils::{pad_to_width, truncate_to_width},
};

use crate::widgets::{ActionResult, InteractableWidget};

pub struct ListColumn<T> {
    pub width: usize,
    value: Box<dyn Fn(&T) -> String>,
    style: Box<dyn Fn(&T) -> Style>,
}

impl<T> ListColumn<T> {
    pub fn new(width: usize, value: impl Fn(&T) -> String + 'static) -> Self {
        Self {
            width,
            value: Box::new(value),
            style: Box::new(|_| Style::default()),
        }
    }

    pub fn with_style(mut self, style: impl Fn(&T) -> Style + 'static) -> Self {
        self.style = Box::new(style);
        self
    }

    fn value(&self, row: &T) -> String {
        (self.value)(row)
    }

    fn style(&self, row: &T) -> Style {
        (self.style)(row)
    }
}

pub struct GenericList<T> {
    pub rows: Vec<T>,
    pub columns: BTreeMap<String, ListColumn<T>>,
    pub state: ListPickerState,
    title: Option<String>,
    column_order: Option<Vec<String>>,
    sticky_header: bool,
}

impl<T> GenericList<T> {
    pub fn new(rows: Vec<T>, columns: BTreeMap<String, ListColumn<T>>) -> Self {
        Self {
            state: ListPickerState::new(rows.len()),
            rows,
            columns,
            title: None,
            column_order: None,
            sticky_header: false,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_column_order<I, S>(mut self, order: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.column_order = Some(order.into_iter().map(Into::into).collect());
        self
    }

    pub fn with_sticky_header(mut self) -> Self {
        self.sticky_header = true;
        self
    }

    pub fn selected_row(&self) -> Option<&T> {
        self.rows.get(self.state.selected_index)
    }

    fn ordered_columns(&self) -> Vec<(&String, &ListColumn<T>)> {
        let Some(order) = &self.column_order else {
            return self.columns.iter().collect();
        };

        let mut columns = order
            .iter()
            .filter_map(|name| self.columns.get_key_value(name))
            .collect::<Vec<_>>();
        columns.extend(
            self.columns
                .iter()
                .filter(|(name, _)| !order.contains(name))
                .collect::<Vec<_>>(),
        );
        columns
    }

    fn column_widths(
        &self,
        available_width: usize,
        columns: &[(&String, &ListColumn<T>)],
    ) -> Vec<usize> {
        let column_count = columns.len();
        if column_count == 0 || available_width == 0 {
            return vec![0; column_count];
        }

        let preferred_widths: Vec<usize> = columns
            .iter()
            .map(|(_, column)| column.width.max(1))
            .collect();
        let total_preferred_width: usize = preferred_widths.iter().sum();
        let minimum_columns = column_count.min(available_width);
        let remaining_width = available_width - minimum_columns;
        let mut widths = vec![0; column_count];

        for width in widths.iter_mut().take(minimum_columns) {
            *width = 1;
        }
        for (width, preferred) in widths.iter_mut().zip(preferred_widths) {
            *width += remaining_width * preferred / total_preferred_width;
        }

        let allocated_width: usize = widths.iter().sum();
        if let Some(last_width) = widths.last_mut() {
            *last_width += available_width.saturating_sub(allocated_width);
        }
        widths
    }
}

impl<T> InteractableWidget for GenericList<T> {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let columns = self.ordered_columns();
        let widths = self.column_widths(area.width.saturating_sub(4) as usize, &columns);
        let header_text = columns
            .iter()
            .zip(&widths)
            .map(|((name, _), width)| pad_to_width(name, *width))
            .collect::<String>();
        let header_line = Line::from(Span::styled(
            header_text.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        let header = ListItem::new(header_line.clone());

        let rows = self.rows.iter().map(|row| {
            let spans = columns
                .iter()
                .zip(&widths)
                .map(|((_, column), width)| {
                    let value = column.value(row);
                    Span::styled(
                        pad_to_width(&truncate_to_width(&value, *width), *width),
                        column.style(row),
                    )
                })
                .collect::<Vec<_>>();
            ListItem::new(Line::from(spans))
        });

        let mut block = Block::default().borders(Borders::ALL);
        if let Some(title) = &self.title {
            block = block.title(title.as_str());
        }
        let selected_index = if self.rows.is_empty() {
            None
        } else {
            Some(self.state.selected_index + usize::from(!self.sticky_header))
        };
        if self.sticky_header {
            let inner_area = block.inner(area);
            frame.render_widget(block, area);
            let [header_area, rows_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner_area);
            let header_line = Line::from(vec![
                Span::raw("  "),
                Span::styled(header_text, Style::default().add_modifier(Modifier::BOLD)),
            ]);
            frame.render_widget(Paragraph::new(header_line), header_area);
            let list = List::new(rows)
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ");
            let mut list_state = ListState::default().with_selected(selected_index);
            frame.render_stateful_widget(list, rows_area, &mut list_state);
        } else {
            let mut items = Vec::with_capacity(self.rows.len() + 1);
            items.push(header);
            items.extend(rows);
            let list = List::new(items)
                .block(block)
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ");
            let mut list_state = ListState::default().with_selected(selected_index);
            frame.render_stateful_widget(list, area, &mut list_state);
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        match key.code {
            KeyCode::Up => {
                self.state.select_prev();
                self.state.ensure_visible(10);
                ActionResult::Consumed
            }
            KeyCode::Down => {
                self.state.select_next();
                self.state.ensure_visible(10);
                ActionResult::Consumed
            }
            _ => ActionResult::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn custom_column_order_is_rendered_before_unlisted_columns() {
        let columns = BTreeMap::from([
            ("Amount".to_string(), ListColumn::new(1, |_| String::new())),
            ("Date".to_string(), ListColumn::new(1, |_| String::new())),
            (
                "Quantity".to_string(),
                ListColumn::new(1, |_| String::new()),
            ),
            ("Type".to_string(), ListColumn::new(1, |_| String::new())),
        ]);
        let list = GenericList::new(Vec::<()>::new(), columns)
            .with_column_order(["Date", "Quantity", "Amount"]);

        let names = list
            .ordered_columns()
            .into_iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Date", "Quantity", "Amount", "Type"]);
    }

    #[test]
    fn sticky_header_remains_visible_when_scrolling_rows() {
        let columns = BTreeMap::from([(
            "Ticker".to_string(),
            ListColumn::new(12, |ticker: &usize| format!("STOCK{ticker}")),
        )]);
        let mut list = GenericList::new((0..20).collect(), columns).with_sticky_header();
        list.state.selected_index = 15;
        let backend = TestBackend::new(30, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| list.render(frame.area(), frame))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let header = (0..buffer.area.width)
            .map(|x| buffer.get(x, 1).symbol())
            .collect::<String>();
        let visible_rows = (2..5)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol())
                    .collect::<String>()
            })
            .collect::<String>();

        assert!(header.contains("Ticker"));
        assert!(visible_rows.contains("STOCK15"));
    }
}
