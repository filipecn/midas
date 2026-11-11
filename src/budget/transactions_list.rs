use chrono::{Datelike, Weekday};
use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use ratatui_interact::{
    components::ListPickerState,
    utils::{pad_to_width, truncate_to_width},
};

use crate::{
    budget::{Transaction, data::Categories},
    core::Accounts,
    tui::TUI,
    widgets::{ActionResult, InteractableWidget},
};

#[derive(Clone)]
pub struct TransactionListRow {
    pub id: usize,
    pub transaction: Transaction,
}

pub struct TransactionsList {
    pub rows: Vec<TransactionListRow>,
    pub state: ListPickerState,
    pub show_date: bool,
    pub show_account: bool,
    pub show_account_type: bool,
    pub show_description: bool,
    pub show_merchant: bool,
    pub show_full_category: bool,
    pub show_category: bool,
    pub show_subcategory: bool,
    pub show_amount: bool,
}

fn column_widths(list: &TransactionsList, available_width: usize) -> Vec<usize> {
    let preferred_widths = [
        (list.show_date, 20),
        (list.show_account, 30),
        (list.show_account_type, 15),
        (list.show_description, 30),
        (list.show_merchant, 30),
        (list.show_full_category, 40),
        (list.show_category, 20),
        (list.show_subcategory, 20),
        (list.show_amount, 10),
    ];
    let total_preferred_width: usize = preferred_widths
        .iter()
        .filter(|(visible, _)| *visible)
        .map(|(_, width)| width)
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
    let remaining_width = available_width - minimum_columns;
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

impl TransactionListRow {
    pub fn new(id: usize, t: &Transaction) -> Self {
        TransactionListRow {
            id,
            transaction: t.clone(),
        }
    }
}

impl TransactionsList {
    pub fn new(
        transactions: Vec<(usize, Transaction)>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut rows = Vec::new();
        for t in transactions.iter() {
            rows.push(TransactionListRow::new(t.0, &t.1));
        }
        Ok(TransactionsList {
            state: ListPickerState::new(transactions.len()),
            rows,
            show_date: true,
            show_account: true,
            show_account_type: true,
            show_description: true,
            show_merchant: true,
            show_full_category: true,
            show_category: false,
            show_subcategory: false,
            show_amount: true,
        })
    }

    pub fn selected_transaction(&self) -> Option<Transaction> {
        if let Some(row) = self.rows.get(self.state.selected_index) {
            Some(row.transaction.clone())
        } else {
            None
        }
    }
}

impl InteractableWidget for TransactionsList {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let widths = column_widths(self, area.width.saturating_sub(4) as usize);
        let items: Vec<Line> = self
            .rows
            .iter()
            .map(|t| {
                let mut width_index = 0;
                let (category, subcategory) = t.transaction.category.inverse_pair();
                let category_color = Categories::get_top_category_color(category.as_str());
                let subcategory_color = if subcategory.is_empty() {
                    category_color
                } else {
                    Categories::get_color(subcategory.as_str())
                };
                let date_color = match t.transaction.date.weekday() {
                    Weekday::Mon => Color::Rgb(70, 130, 180),
                    Weekday::Tue => Color::Rgb(0, 160, 180),
                    Weekday::Wed => Color::Rgb(46, 139, 87),
                    Weekday::Thu => Color::Rgb(218, 165, 32),
                    Weekday::Fri => Color::Rgb(186, 85, 211),
                    Weekday::Sat => Color::Rgb(230, 126, 34),
                    Weekday::Sun => Color::Rgb(205, 92, 92),
                };
                let mut cols: Vec<Span> = Vec::new();
                if self.show_date {
                    let width = widths[width_index];
                    width_index += 1;
                    let date_text = format!("{}", t.transaction.date.format("%Y-%m-%d %H:%M:%S"));
                    let date = pad_to_width(&truncate_to_width(&date_text, width), width);
                    cols.push(Span::styled(date, Style::default().fg(date_color)));
                }
                if self.show_account {
                    let width = widths[width_index];
                    width_index += 1;
                    let account_color = Accounts::get_account_color(&t.transaction.account_id)
                        .unwrap_or(category_color);
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&t.transaction.account_id, width), width),
                        Style::default().fg(account_color),
                    ));
                }
                if self.show_account_type {
                    let width = widths[width_index];
                    width_index += 1;
                    let account_type =
                        Accounts::get_type(&t.transaction.account_id).unwrap_or_default();
                    let account_type_color = if account_type.eq_ignore_ascii_case("credit") {
                        palette.spent
                    } else if account_type.eq_ignore_ascii_case("checking") {
                        palette.green
                    } else if account_type.eq_ignore_ascii_case("savings") {
                        palette.budget
                    } else {
                        Color::Gray
                    };
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&account_type, width), width),
                        Style::default().fg(account_type_color),
                    ));
                }
                if self.show_description {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&t.transaction.description, width), width),
                        Style::default().fg(category_color),
                    ));
                }
                if self.show_merchant {
                    let width = widths[width_index];
                    width_index += 1;
                    cols.push(Span::styled(
                        pad_to_width(&truncate_to_width(&t.transaction.merchant, width), width),
                        Style::default().fg(category_color),
                    ));
                }
                if self.show_full_category {
                    let width = widths[width_index];
                    width_index += 1;
                    let combined = format!("{} - {}", category, subcategory);
                    let text = pad_to_width(truncate_to_width(&combined, width).as_str(), width);
                    cols.push(Span::styled(text, Style::default().fg(subcategory_color)));
                }
                if self.show_category {
                    let width = widths[width_index];
                    width_index += 1;
                    let text = pad_to_width(truncate_to_width(&category, width).as_str(), width);
                    cols.push(Span::styled(text, Style::default().fg(category_color)));
                }
                if self.show_subcategory {
                    let width = widths[width_index];
                    width_index += 1;
                    let text = pad_to_width(truncate_to_width(&subcategory, width).as_str(), width);
                    cols.push(Span::styled(text, Style::default().fg(subcategory_color)));
                }
                if self.show_amount {
                    let width = widths[width_index];
                    let amount_str =
                        TUI::format_amount_for_display(t.transaction.amount_in_cents, width);
                    if t.transaction.amount_in_cents < 0 {
                        cols.push(Span::styled(
                            amount_str,
                            ratatui::style::Style::default().fg(palette.spent),
                        ));
                    } else {
                        cols.push(Span::styled(
                            amount_str,
                            ratatui::style::Style::default().fg(palette.green),
                        ));
                    }
                }
                Line::from(cols)
            })
            .collect();

        let mut header_cols: Vec<String> = Vec::new();
        let mut width_index = 0;
        if self.show_date {
            header_cols.push(pad_to_width("Date", widths[width_index]));
            width_index += 1;
        }
        if self.show_account {
            header_cols.push(pad_to_width("Account", widths[width_index]));
            width_index += 1;
        }
        if self.show_account_type {
            header_cols.push(pad_to_width("Account Type", widths[width_index]));
            width_index += 1;
        }
        if self.show_description {
            header_cols.push(pad_to_width("Description", widths[width_index]));
            width_index += 1;
        }
        if self.show_merchant {
            header_cols.push(pad_to_width("Merchant", widths[width_index]));
            width_index += 1;
        }
        if self.show_full_category {
            header_cols.push(pad_to_width("Category - Subcategory", widths[width_index]));
            width_index += 1;
        }
        if self.show_category {
            header_cols.push(pad_to_width("Category", widths[width_index]));
            width_index += 1;
        }
        if self.show_subcategory {
            header_cols.push(pad_to_width("Subcategory", widths[width_index]));
            width_index += 1;
        }
        if self.show_amount {
            header_cols.push(format!("{:>width$}", "Amount", width = widths[width_index]));
        }

        let header = ListItem::new(Line::from(Span::styled(
            header_cols.join(""),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let mut list_items: Vec<ListItem> = Vec::with_capacity(items.len() + 1);
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
        let mut result = ActionResult::Consumed;
        match key.code {
            KeyCode::Enter => {
                if let Some(t) = self.selected_transaction() {
                    result = ActionResult::SelectTransaction(t);
                }
            }
            KeyCode::Up => {
                self.state.select_prev();
                self.state.ensure_visible(10);
            }
            KeyCode::Down => {
                self.state.select_next();
                self.state.ensure_visible(10);
            }
            _ => result = ActionResult::None,
        };
        result
    }
}
