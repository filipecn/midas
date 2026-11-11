use std::collections::HashMap;

use chrono::{Datelike, Months, NaiveDate};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, Block, Borders, Paragraph},
};
use ratatui_interact::state::FocusManager;

use crate::{
    budget::{
        data::Categories, db::Transactions, expense_group_tree::ExpenseGroupTree,
        transactions_list::TransactionsList, Transaction,
    },
    tui::TUI,
    widgets::InteractableWidget,
};

/// Which panel is focused
#[derive(Clone, Eq, PartialEq, Hash)]
enum FocusedItem {
    VariableGroupsTable,
    FixedGroupsTable,
    TransactionsTable,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GroupTreeType {
    Variable,
    Fixed,
}

struct MonthData {
    transactions: Vec<Transaction>,
    pub amounts: HashMap<String, i64>, // category name -> total amount
    pub planned_amounts: HashMap<String, i64>, // category name -> planned amount
    total_spent: u64,
}

fn format_budget_spent_percentage(budget: i64, spent: i64) -> String {
    if budget == 0 {
        "       -".to_string()
    } else {
        format!(
            "{:>7.2}%",
            spent.unsigned_abs() as f64 / budget.unsigned_abs() as f64 * 100.0
        )
    }
}

impl MonthData {
    pub fn new(transactions: Vec<Transaction>) -> Self {
        let amounts: HashMap<String, i64> =
            transactions.iter().fold(HashMap::new(), |mut acc, t| {
                *acc.entry(t.category.name.clone()).or_insert(0) += t.amount_in_cents;
                if let Some(parent) = &t.category.parent {
                    *acc.entry(parent.clone()).or_insert(0) += t.amount_in_cents;
                }
                acc
            });
        let expense_groups = Categories::get_expense_groups();
        let planned_amounts: HashMap<String, i64> = expense_groups
            .iter()
            .map(|(group_name, _)| {
                (group_name.clone(), 0) // Initialize with 0, you might want to calculate this based on your data
            })
            .collect();
        let total_spent = transactions
            .iter()
            .filter(|transaction| {
                transaction.amount_in_cents < 0 && transaction.category.category_type == "expense"
            })
            .fold(0_u64, |total, transaction| {
                total.saturating_add(transaction.amount_in_cents.unsigned_abs())
            });
        Self {
            transactions,
            amounts,
            planned_amounts,
            total_spent,
        }
    }
}

pub struct ExpenseGroupsView {
    current_year: u32,
    current_month: u32,
    // (year, month) -> MonthData
    cache: HashMap<(u32, u32), MonthData>,
    focus: FocusManager<FocusedItem>,
    group_tree: ExpenseGroupTree,
    fixed_group_tree: ExpenseGroupTree,
    active_group_tree: GroupTreeType,
    transactions_table: TransactionsList,
}

impl ExpenseGroupsView {
    pub fn new(current_year: u32, current_month: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut focus: FocusManager<FocusedItem> = FocusManager::new();
        focus.register(FocusedItem::VariableGroupsTable);
        focus.register(FocusedItem::FixedGroupsTable);
        focus.register(FocusedItem::TransactionsTable);
        let mut this = Self {
            current_year,
            current_month,
            cache: HashMap::new(),
            focus,
            group_tree: ExpenseGroupTree::new(),
            fixed_group_tree: ExpenseGroupTree::new(),
            active_group_tree: GroupTreeType::Variable,
            transactions_table: TransactionsList::new(Vec::new())?,
        };
        // load all months available in the database into the cache
        let available_months = Transactions::available_months()?;
        for (year, month) in available_months {
            this.load_month_data(year, month)?;
        }
        this.ensure_current_month_has_data();
        // Estimate the planned amounts for each group based on the last 3 months of data
        this.estimate_planned_amounts();
        Ok(this)
    }

    fn ensure_current_month_has_data(&mut self) {
        if self
            .cache
            .contains_key(&(self.current_year, self.current_month))
        {
            return;
        }

        if let Some(&(year, month)) = self.cache.keys().max() {
            self.current_year = year;
            self.current_month = month;
        }
    }

    fn load_month_data(&mut self, year: u32, month: u32) -> Result<(), Box<dyn std::error::Error>> {
        if !self.cache.contains_key(&(year, month)) {
            let transactions = Transactions::read_month(year, month)?;
            self.cache
                .insert((year, month), MonthData::new(transactions));
        }

        if let Some(month_data) = self.cache.get(&(year, month)) {
            self.group_tree.update(
                &month_data.amounts,
                &month_data.planned_amounts,
                "variable",
                month_data.total_spent,
            );
            self.fixed_group_tree.update(
                &month_data.amounts,
                &month_data.planned_amounts,
                "fixed",
                month_data.total_spent,
            );
            self.update_transactions_table();
        }

        Ok(())
    }

    fn estimate_planned_amounts(&mut self) {
        let expense_groups = Categories::get_expense_groups();
        let month_keys: Vec<(u32, u32)> = self.cache.keys().copied().collect();
        let mut estimates = Vec::new();

        for (year, month) in month_keys {
            let mut planned_amounts = HashMap::new();
            for (group_name, subcategories) in &expense_groups {
                let mut category_names = vec![group_name.clone()];
                category_names.extend(
                    subcategories
                        .iter()
                        .map(|subcategory| subcategory.name.clone()),
                );
                for category_name in category_names {
                    let mut history_total = 0;
                    let mut history_count = 0;
                    let mut history_year = year;
                    let mut history_month = month;

                    for _ in 0..3 {
                        if history_month == 1 {
                            if history_year == 0 {
                                break;
                            }
                            history_year -= 1;
                            history_month = 12;
                        } else {
                            history_month -= 1;
                        }

                        if let Some(previous_month) = self.cache.get(&(history_year, history_month))
                        {
                            history_total += previous_month
                                .amounts
                                .get(&category_name)
                                .copied()
                                .unwrap_or(0)
                                .abs();
                            history_count += 1;
                        }
                    }

                    let planned_amount = if history_count == 0 {
                        0
                    } else {
                        history_total / history_count
                    };
                    planned_amounts.insert(category_name, planned_amount);
                }
            }
            estimates.push(((year, month), planned_amounts));
        }

        for (month_key, planned_amounts) in estimates {
            if let Some(month_data) = self.cache.get_mut(&month_key) {
                month_data.planned_amounts = planned_amounts;
            }
        }

        if let Some(month_data) = self.cache.get(&(self.current_year, self.current_month)) {
            self.group_tree.update(
                &month_data.amounts,
                &month_data.planned_amounts,
                "variable",
                month_data.total_spent,
            );
            self.fixed_group_tree.update(
                &month_data.amounts,
                &month_data.planned_amounts,
                "fixed",
                month_data.total_spent,
            );
        }
    }

    fn next_month(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (year, month) = if self.current_month == 12 {
            (self.current_year + 1, 1)
        } else {
            (self.current_year, self.current_month + 1)
        };
        self.load_month_data(year, month)?;
        self.current_year = year;
        self.current_month = month;
        self.estimate_planned_amounts();
        Ok(())
    }

    fn previous_month(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (year, month) = if self.current_month == 1 {
            (self.current_year - 1, 12)
        } else {
            (self.current_year, self.current_month - 1)
        };
        self.load_month_data(year, month)?;
        self.current_year = year;
        self.current_month = month;
        self.estimate_planned_amounts();
        Ok(())
    }

    fn current_month_data(&self) -> Option<&MonthData> {
        self.cache.get(&(self.current_year, self.current_month))
    }

    fn spending_by_month(&self) -> Vec<(String, u64)> {
        let Some(current_date) =
            NaiveDate::from_ymd_opt(self.current_year as i32, self.current_month, 1)
        else {
            return Vec::new();
        };

        (0..6)
            .filter_map(|months_ago| current_date.checked_sub_months(Months::new(months_ago)))
            .map(|date| {
                let amount = self
                    .cache
                    .get(&(date.year() as u32, date.month()))
                    .map(|month_data| {
                        month_data
                            .transactions
                            .iter()
                            .filter(|transaction| transaction.amount_in_cents < 0)
                            .fold(0_u64, |total, transaction| {
                                total.saturating_add(transaction.amount_in_cents.unsigned_abs())
                            })
                    })
                    .unwrap_or(0);
                (date.format("%b").to_string(), amount)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn update_transactions_table(&mut self) {
        self.ensure_current_month_has_data();

        let selected_category = match self.active_group_tree {
            GroupTreeType::Variable => self.group_tree.selected_category(),
            GroupTreeType::Fixed => self.fixed_group_tree.selected_category(),
        };
        let month_key = (self.current_year, self.current_month);
        let transactions = self
            .cache
            .get(&month_key)
            .map(|month_data| {
                month_data
                    .transactions
                    .iter()
                    .enumerate()
                    .filter(|(_, transaction)| match &selected_category {
                        Some(category) => {
                            Categories::overlap(category.clone(), transaction.category.clone())
                        }
                        None => true,
                    })
                    .map(|(idx, transaction)| (idx, transaction.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        self.transactions_table = TransactionsList::new(transactions).unwrap();
        self.transactions_table.show_amount = true;
        self.transactions_table.show_full_category = false;
        self.transactions_table.show_category = false;
        self.transactions_table.show_subcategory = true;
        self.transactions_table.show_account_type = false;
        self.transactions_table.show_description = true;
        self.transactions_table.show_merchant = false;
        self.transactions_table.show_account = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{format_budget_spent_percentage, ExpenseGroupsView, MonthData};
    use crate::budget::{Category, Transaction};
    use std::collections::HashMap;

    fn transaction(amount_in_cents: i64) -> Transaction {
        Transaction {
            db_id: None,
            identifier: String::new(),
            date: chrono::Utc::now(),
            amount_in_cents,
            account_id: String::new(),
            category: Category::uncategorized(),
            description: String::new(),
            merchant: String::new(),
        }
    }

    #[test]
    fn budget_spent_percentage_uses_budget_as_denominator() {
        assert_eq!(format_budget_spent_percentage(20_000, -15_000), "  75.00%");
        assert_eq!(format_budget_spent_percentage(0, -15_000), "       -");
    }

    #[test]
    fn falls_back_to_latest_available_month_when_current_month_has_no_transactions() {
        let mut view = ExpenseGroupsView {
            current_year: 2026,
            current_month: 10,
            cache: HashMap::from([
                ((2025, 12), super::MonthData::new(vec![])),
                ((2026, 3), super::MonthData::new(vec![])),
            ]),
            focus: ratatui_interact::state::FocusManager::new(),
            group_tree: crate::budget::expense_group_tree::ExpenseGroupTree::new(),
            fixed_group_tree: crate::budget::expense_group_tree::ExpenseGroupTree::new(),
            active_group_tree: super::GroupTreeType::Variable,
            transactions_table: crate::budget::transactions_list::TransactionsList::new(Vec::new())
                .unwrap(),
        };
        view.ensure_current_month_has_data();
        assert_eq!(view.current_year, 2026);
        assert_eq!(view.current_month, 3);
    }

    #[test]
    fn spending_by_month_includes_six_calendar_months_and_only_expenses() {
        let view = ExpenseGroupsView {
            current_year: 2026,
            current_month: 2,
            cache: HashMap::from([
                ((2025, 9), MonthData::new(vec![transaction(-200)])),
                (
                    (2025, 12),
                    MonthData::new(vec![transaction(-250), transaction(500)]),
                ),
                ((2026, 2), MonthData::new(vec![transaction(-350)])),
            ]),
            focus: ratatui_interact::state::FocusManager::new(),
            group_tree: crate::budget::expense_group_tree::ExpenseGroupTree::new(),
            fixed_group_tree: crate::budget::expense_group_tree::ExpenseGroupTree::new(),
            active_group_tree: super::GroupTreeType::Variable,
            transactions_table: crate::budget::transactions_list::TransactionsList::new(Vec::new())
                .unwrap(),
        };

        assert_eq!(
            view.spending_by_month(),
            vec![
                ("Sep".to_string(), 200),
                ("Oct".to_string(), 0),
                ("Nov".to_string(), 0),
                ("Dec".to_string(), 250),
                ("Jan".to_string(), 0),
                ("Feb".to_string(), 350),
            ]
        );
    }
}

impl InteractableWidget for ExpenseGroupsView {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let main_layout = Layout::horizontal([Constraint::Percentage(50), Constraint::Min(1)]);
        let [left_area, right_area] = area.layout(&main_layout);
        // Main layout: Title at the top, Table filling the rest
        let left_layout = Layout::vertical([
            Constraint::Length(2), // Title height
            Constraint::Min(1),    // Table area
            Constraint::Length(4), // Transfers, column headers, and expense totals
            Constraint::Length(2), // Footer height
        ]);

        let [title_area, table_area, totals_area, footer] = left_area.layout(&left_layout);

        // 1. Render Title matching the orange accent from the screenshot
        let month_name = NaiveDate::from_ymd_opt(self.current_year as i32, self.current_month, 1)
            .map(|date| date.format("%B").to_string())
            .unwrap_or_else(|| self.current_month.to_string());
        let title = Line::from(vec![Span::styled(
            format!("Group Expenses - {}/{}", month_name, self.current_year,),
            Style::default()
                .fg(Color::Rgb(235, 100, 50))
                .add_modifier(Modifier::BOLD),
        )]);
        frame.render_widget(title, title_area);

        let [variable_area, fixed_area] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Min(1)]).areas(table_area);
        let [variable_title_area, variable_tree_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(variable_area);
        let [fixed_title_area, fixed_tree_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(fixed_area);
        let variable_title_style = if self.active_group_tree == GroupTreeType::Variable {
            Style::default()
                .fg(Color::Rgb(235, 100, 50))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let fixed_title_style = if self.active_group_tree == GroupTreeType::Fixed {
            Style::default()
                .fg(Color::Rgb(235, 100, 50))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new("Variable Expenses").style(variable_title_style),
            variable_title_area,
        );
        frame.render_widget(
            Paragraph::new("Fixed Expenses").style(fixed_title_style),
            fixed_title_area,
        );
        self.group_tree.render(variable_tree_area, frame);
        self.fixed_group_tree.render(fixed_tree_area, frame);
        let [transactions_area, spending_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(10)]).areas(right_area);
        self.transactions_table.render(transactions_area, frame);

        let palette = TUI::color_palette();
        let spending = self.spending_by_month();
        let max_spending = spending
            .iter()
            .map(|(_, amount)| *amount)
            .max()
            .unwrap_or(0)
            .max(1);
        let bars = spending
            .iter()
            .map(|(month, amount)| {
                let display_amount = i64::try_from(*amount).unwrap_or(i64::MAX);
                Bar::with_label(month.as_str(), *amount)
                    .text_value(TUI::format_amount_for_display(display_amount, 8))
            })
            .collect::<Vec<_>>();
        let chart = BarChart::new(bars)
            .block(
                Block::default()
                    .title("Spent per month (last 6 months)")
                    .borders(Borders::ALL),
            )
            .bar_width(8)
            .bar_gap(1)
            .max(max_spending)
            .bar_style(Style::default().fg(palette.spent))
            .value_style(Style::default().fg(Color::White))
            .label_style(Style::default().fg(Color::Gray));
        frame.render_widget(chart, spending_area);

        let format_total = |amount| {
            if TUI::sensitive_values_visible() {
                format!("{:>8.2}", amount as f64 / 100.0)
            } else {
                TUI::format_amount_for_display(amount, 8)
            }
        };
        let transfer_total = self
            .current_month_data()
            .and_then(|month_data| month_data.amounts.get("Transfers & Capital").copied())
            .unwrap_or(0);
        let [transfers_area, totals_header_area, variable_totals_area, fixed_totals_area] =
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .areas(totals_area);
        let transfers_line = Line::from(vec![
            Span::styled(
                format!("{:>14}: ", "Transfers"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format_total(transfer_total),
                Style::default().fg(palette.spent),
            ),
        ]);
        frame.render_widget(Paragraph::new(transfers_line), transfers_area);

        let totals_header = Line::from(format!(
            "{:>16}{:>8} {:>8} {:>8} {:>8}",
            "", "Budget", "Spent", "Net", "%"
        ));
        frame.render_widget(Paragraph::new(totals_header), totals_header_area);

        for (tree, label, totals_area) in [
            (&self.group_tree, "Variable total", variable_totals_area),
            (&self.fixed_group_tree, "Fixed total", fixed_totals_area),
        ] {
            let (budget, spent) = tree.totals();
            let net = budget + spent;
            let budget_spent_percentage = format_budget_spent_percentage(budget, spent);
            let totals_line = Line::from(vec![
                Span::styled(
                    format!("{label:>14}: "),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format_total(budget), Style::default().fg(palette.budget)),
                Span::raw(" "),
                Span::styled(format_total(spent), Style::default().fg(palette.spent)),
                Span::raw(" "),
                Span::styled(
                    format_total(net),
                    Style::default().fg(if net < 0 { Color::Red } else { palette.green }),
                ),
                Span::raw(" "),
                Span::raw(budget_spent_percentage),
            ]);
            frame.render_widget(Paragraph::new(totals_line), totals_area);
        }

        // get selected group and get its category description
        let selected_category = match self.active_group_tree {
            GroupTreeType::Variable => self.group_tree.selected_category(),
            GroupTreeType::Fixed => self.fixed_group_tree.selected_category(),
        };
        if let Some(category) = selected_category {
            let description = category
                .description
                .unwrap_or_else(|| "No description available".to_string());
            let footer_text = Line::from(vec![Span::styled(
                description,
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )]);
            frame.render_widget(footer_text, footer);
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> super::ActionResult {
        let mut result = super::ActionResult::Consumed;
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Left => {
                    if let Err(e) = self.previous_month() {
                        eprintln!("Error loading previous month: {}", e);
                    }
                    self.update_transactions_table();
                }
                KeyCode::Right => {
                    if let Err(e) = self.next_month() {
                        eprintln!("Error loading next month: {}", e);
                    }
                    self.update_transactions_table();
                }
                _ => {}
            }
            return result;
        }
        match key.code {
            KeyCode::Tab => {
                self.focus.next();
                match self.focus.current().unwrap() {
                    FocusedItem::VariableGroupsTable => {
                        self.active_group_tree = GroupTreeType::Variable;
                    }
                    FocusedItem::FixedGroupsTable => {
                        self.active_group_tree = GroupTreeType::Fixed;
                    }
                    FocusedItem::TransactionsTable => {}
                }
                self.update_transactions_table();
            }
            _ => match self.focus.current().unwrap() {
                FocusedItem::VariableGroupsTable => {
                    result = self.group_tree.handle_key(key);
                    self.active_group_tree = GroupTreeType::Variable;
                    self.update_transactions_table();
                }
                FocusedItem::FixedGroupsTable => {
                    result = self.fixed_group_tree.handle_key(key);
                    self.active_group_tree = GroupTreeType::Fixed;
                    self.update_transactions_table();
                }
                FocusedItem::TransactionsTable => {
                    result = self.transactions_table.handle_key(key);
                }
            },
        }
        result
    }
}
