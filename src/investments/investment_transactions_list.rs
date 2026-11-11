use crossterm::event::KeyCode;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use ratatui_interact::{
    components::ListPickerState,
    utils::{pad_to_width, truncate_to_width},
};

use crate::{
    investments::InvestmentTransaction,
    tui::TUI,
    widgets::{ActionResult, InteractableWidget},
};

pub struct InvestmentTransactionsList {
    pub rows: Vec<InvestmentTransaction>,
    pub state: ListPickerState,
}

fn column_widths(available_width: usize) -> [usize; 7] {
    let preferred_widths = [20, 24, 14, 12, 16, 16, 14];
    let minimum_columns = preferred_widths.len().min(available_width);
    let remaining_width = available_width.saturating_sub(minimum_columns);
    let total_preferred_width: usize = preferred_widths.iter().sum();
    let mut widths = [0; 7];

    for (index, width) in widths.iter_mut().enumerate().take(minimum_columns) {
        *width = 1 + remaining_width * preferred_widths[index] / total_preferred_width;
    }

    let allocated_width: usize = widths.iter().sum();
    widths[6] += available_width.saturating_sub(allocated_width);
    widths
}

fn optional_amount(value: Option<i64>, width: usize) -> String {
    value
        .map(|cents| TUI::format_amount_for_display(cents, width))
        .unwrap_or_default()
}

fn optional_quantity(value: Option<f64>) -> String {
    value
        .map(|quantity| format!("{quantity:.4}"))
        .unwrap_or_default()
}

impl InvestmentTransactionsList {
    pub fn new(investment_transactions: Vec<InvestmentTransaction>) -> Self {
        let row_count = investment_transactions.len();
        Self {
            rows: investment_transactions,
            state: ListPickerState::new(row_count),
        }
    }

    pub fn selected_transaction(&self) -> Option<&InvestmentTransaction> {
        self.rows.get(self.state.selected_index)
    }
}

impl InteractableWidget for InvestmentTransactionsList {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let widths = column_widths(area.width.saturating_sub(4) as usize);
        let items = self
            .rows
            .iter()
            .map(|transaction| {
                let date = format!("{}", transaction.date.format("%Y-%m-%d %H:%M:%S"));
                let values = [
                    pad_to_width(&truncate_to_width(&date, widths[0]), widths[0]),
                    pad_to_width(
                        &truncate_to_width(&transaction.investment_id, widths[1]),
                        widths[1],
                    ),
                    pad_to_width(
                        &truncate_to_width(&transaction.transaction_type.to_string(), widths[2]),
                        widths[2],
                    ),
                    format!(
                        "{:>width$}",
                        optional_quantity(transaction.quantity),
                        width = widths[3]
                    ),
                    format!(
                        "{:>width$}",
                        optional_amount(transaction.price_per_unit_in_cents, widths[4]),
                        width = widths[4]
                    ),
                    format!(
                        "{:>width$}",
                        optional_amount(transaction.value_in_cents, widths[5]),
                        width = widths[5]
                    ),
                    format!(
                        "{:>width$}",
                        optional_amount(Some(transaction.amount_in_cents), widths[6]),
                        width = widths[6]
                    ),
                ];
                Line::from(vec![
                    Span::styled(values[0].clone(), Style::default().fg(palette.budget)),
                    Span::raw(values[1].clone()),
                    Span::raw(values[2].clone()),
                    Span::raw(values[3].clone()),
                    Span::raw(values[4].clone()),
                    Span::raw(values[5].clone()),
                    Span::styled(values[6].clone(), Style::default().fg(palette.green)),
                ])
            })
            .collect::<Vec<_>>();

        let headers = [
            ("Date", widths[0]),
            ("Investment", widths[1]),
            ("Type", widths[2]),
            ("Quantity", widths[3]),
            ("Price", widths[4]),
            ("Value", widths[5]),
            ("Amount", widths[6]),
        ];
        let header = ListItem::new(Line::from(Span::styled(
            headers
                .iter()
                .map(|(label, width)| {
                    if matches!(*label, "Quantity" | "Price" | "Value" | "Amount") {
                        format!("{:>width$}", label, width = width)
                    } else {
                        pad_to_width(label, *width)
                    }
                })
                .collect::<String>(),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let mut list_items = Vec::with_capacity(items.len() + 1);
        list_items.push(header);
        list_items.extend(items.into_iter().map(ListItem::new));

        let list = List::new(list_items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");
        let selected_index = if self.rows.is_empty() {
            None
        } else {
            Some(self.state.selected_index + 1)
        };
        let mut list_state = ListState::default().with_selected(selected_index);
        frame.render_stateful_widget(list, area, &mut list_state);
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
