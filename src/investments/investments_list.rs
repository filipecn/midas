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
    investments::Investment,
    tui::TUI,
    widgets::{ActionResult, InteractableWidget},
};

#[derive(Clone)]
pub struct InvestmentListRow {
    pub investment: Investment,
    pub total_invested_in_cents: i64,
}

impl InvestmentListRow {
    fn percent_change(&self) -> f64 {
        if self.total_invested_in_cents == 0 {
            0.0
        } else {
            ((self.investment.amount_in_cents as f64 - self.total_invested_in_cents as f64)
                / self.total_invested_in_cents as f64)
                * 100.0
        }
    }
}

pub struct InvestmentsList {
    pub rows: Vec<InvestmentListRow>,
    pub state: ListPickerState,
    pub title: Option<String>,
    pub show_date: bool,
    pub show_due_date: bool,
    pub show_name: bool,
    pub show_type: bool,
    pub show_subtype: bool,
    pub show_status: bool,
    pub show_amount: bool,
    pub show_percent_change: bool,
    pub show_time: bool,
    pub show_rate: bool,
    pub show_rate_type: bool,
    pub show_fixed_annual_rate: bool,
    pub focused: bool,
}

fn column_widths(list: &InvestmentsList, available_width: usize) -> Vec<usize> {
    let preferred_widths = [
        (list.show_date, 12),
        (list.show_due_date, 12),
        (list.show_name, 18),
        (list.show_type, 12),
        (list.show_subtype, 12),
        (list.show_status, 12),
        (list.show_amount, 12),
        (list.show_percent_change, 14),
        (list.show_time, 12),
        (list.show_rate, 12),
        (list.show_rate_type, 14),
        (list.show_fixed_annual_rate, 16),
    ];
    let total_preferred_width: usize = preferred_widths
        .iter()
        .filter(|(visible, _)| *visible)
        .map(|(_, width)| *width)
        .sum();
    let visible_columns = preferred_widths
        .iter()
        .filter(|(visible, _)| *visible)
        .count();

    if visible_columns == 0 || available_width == 0 {
        return preferred_widths
            .iter()
            .filter(|(visible, _)| *visible)
            .map(|_| 0)
            .collect();
    }

    let visible_preferred_widths: Vec<usize> = preferred_widths
        .iter()
        .filter(|(visible, _)| *visible)
        .map(|(_, preferred)| *preferred)
        .collect();
    let minimum_columns = visible_columns.min(available_width);
    let remaining_width = available_width.saturating_sub(minimum_columns);
    let mut widths = vec![0; visible_columns];
    for width in widths.iter_mut().take(minimum_columns) {
        *width = 1;
    }
    for (width, preferred) in widths.iter_mut().zip(visible_preferred_widths) {
        *width += remaining_width * preferred / total_preferred_width;
    }
    let allocated_width: usize = widths.iter().sum();
    if let Some(last_width) = widths.last_mut() {
        *last_width += available_width.saturating_sub(allocated_width);
    }
    widths
}

impl InvestmentsList {
    pub fn new(investments: Vec<Investment>) -> Self {
        let rows = investments
            .into_iter()
            .map(|investment| InvestmentListRow {
                investment,
                total_invested_in_cents: 0,
            })
            .collect::<Vec<_>>();
        let row_count = rows.len();
        Self {
            rows,
            state: ListPickerState::new(row_count),
            title: None,
            show_date: true,
            show_due_date: true,
            show_name: true,
            show_type: true,
            show_subtype: true,
            show_status: true,
            show_amount: true,
            show_percent_change: false,
            show_time: true,
            show_rate: true,
            show_rate_type: true,
            show_fixed_annual_rate: true,
            focused: false,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn selected_investment(&self) -> Option<&Investment> {
        self.rows
            .get(self.state.selected_index)
            .map(|row| &row.investment)
    }
}

impl InteractableWidget for InvestmentsList {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let widths = column_widths(self, area.width.saturating_sub(4) as usize);
        let items = self
            .rows
            .iter()
            .map(|row| {
                let investment = &row.investment;
                let date = format!("{}", investment.date.format("%Y-%m-%d %H:%M:%S"));
                let due_date = investment
                    .due_date
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                let current_amount = TUI::format_amount_for_display(investment.amount_in_cents, 12);
                let percent_change = row.percent_change();
                let percent_change_color = if percent_change >= 0.0 {
                    palette.green
                } else {
                    palette.spent
                };
                let elapsed_days = chrono::Utc::now()
                    .signed_duration_since(investment.date)
                    .num_days();
                let time = format!("{elapsed_days}");
                let rate = investment
                    .rate
                    .map(|value| format!("{value:.2}%"))
                    .unwrap_or_default();
                let fixed_annual_rate = investment
                    .fixed_annual_rate
                    .map(|value| format!("{value:.2}%"))
                    .unwrap_or_default();

                let mut width_index = 0;
                let mut cols: Vec<Span> = Vec::new();

                if self.show_date {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&date, width), width),
                        Style::default().fg(palette.budget),
                    ));
                }
                if self.show_due_date {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&due_date, width), width),
                        Style::default().fg(palette.budget),
                    ));
                }
                if self.show_name {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&investment.name, width), width),
                        Style::default(),
                    ));
                }
                if self.show_type {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(
                            &truncate_to_width(&investment.investment_type, width),
                            width,
                        ),
                        Style::default(),
                    ));
                }
                if self.show_subtype {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&investment.subtype, width), width),
                        Style::default(),
                    ));
                }
                if self.show_status {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&investment.status, width), width),
                        Style::default(),
                    ));
                }
                if self.show_amount {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        format!("{:>width$}", current_amount, width = width),
                        Style::default().fg(palette.green),
                    ));
                }
                if self.show_percent_change {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        format!(
                            "{:>width$}",
                            format!("{percent_change:+.2}%"),
                            width = width
                        ),
                        Style::default().fg(percent_change_color),
                    ));
                }
                if self.show_time {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        format!("{:>width$}", time, width = width),
                        Style::default(),
                    ));
                }
                if self.show_rate {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        format!("{:>width$}", rate, width = width),
                        Style::default(),
                    ));
                }
                if self.show_rate_type {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(
                            &truncate_to_width(
                                investment.rate_type.as_deref().unwrap_or_default(),
                                width,
                            ),
                            width,
                        ),
                        Style::default(),
                    ));
                }
                if self.show_fixed_annual_rate {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        format!("{:>width$}", fixed_annual_rate, width = width),
                        Style::default(),
                    ));
                }

                Line::from(cols)
            })
            .collect::<Vec<_>>();

        let mut header_parts = Vec::new();
        let mut width_index = 0;
        if self.show_date {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Date", width));
        }
        if self.show_due_date {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Due Date", width));
        }
        if self.show_name {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Name", width));
        }
        if self.show_type {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Type", width));
        }
        if self.show_subtype {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Subtype", width));
        }
        if self.show_status {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Status", width));
        }
        if self.show_amount {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(format!("{:>width$}", "Amount", width = width));
        }
        if self.show_percent_change {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(format!("{:>width$}", "Change %", width = width));
        }
        if self.show_time {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(format!("{:>width$}", "Time", width = width));
        }
        if self.show_rate {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(format!("{:>width$}", "Rate", width = width));
        }
        if self.show_rate_type {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(pad_to_width("Rate Type", width));
        }
        if self.show_fixed_annual_rate {
            let width = widths[width_index];
            width_index += 1;
            header_parts.push(format!("{:>width$}", "Fixed Annual Rate", width = width));
        }

        let header_text = header_parts.join("");
        let header = ListItem::new(Line::from(Span::styled(
            header_text,
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let mut list_items = Vec::with_capacity(items.len() + 1);
        list_items.push(header);
        list_items.extend(items.into_iter().map(ListItem::new));

        let block = {
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(if self.focused {
                    Style::default().fg(palette.budget)
                } else {
                    Style::default()
                });
            if let Some(title) = &self.title {
                block = block.title(format!(" {title} "));
            }
            block
        };

        let list = List::new(list_items)
            .block(block)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_row(amount_in_cents: i64, total_invested_in_cents: i64) -> InvestmentListRow {
        InvestmentListRow {
            investment: Investment {
                db_id: None,
                identifier: "investment".to_string(),
                date: Utc::now(),
                due_date: None,
                name: "Investment".to_string(),
                amount_in_cents,
                amount_original_in_cents: 0,
                investment_type: "type".to_string(),
                subtype: "subtype".to_string(),
                status: "active".to_string(),
                rate: None,
                rate_type: None,
                rate_periodicity: None,
                fixed_annual_rate: None,
            },
            total_invested_in_cents,
        }
    }

    #[test]
    fn percent_change_uses_amount_relative_to_invested_total() {
        assert_eq!(make_row(125, 100).percent_change(), 25.0);
        assert_eq!(make_row(75, 100).percent_change(), -25.0);
        assert_eq!(make_row(100, 0).percent_change(), 0.0);
    }
}
