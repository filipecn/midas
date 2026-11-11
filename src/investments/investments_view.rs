use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::collections::{BTreeMap, HashMap};

use crate::{
    investments::{
        Investment, InvestmentTransaction, InvestmentTransactionType,
        db::Investments as InvestmentDatabase, investments_list::InvestmentsList,
        investments_tree::InvestmentsTree,
    },
    tui::TUI,
    widgets::{
        ActionResult, InteractableWidget,
        input_box::InputBox,
        list::{GenericList, ListColumn},
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum InvestmentFocus {
    Tree,
    List,
    Transactions,
}

enum OriginalAmountEditState {
    Inactive,
    Editing {
        input: InputBox,
        error: Option<String>,
    },
}

pub struct InvestmentsView {
    all_investments: Vec<Investment>,
    all_investment_transactions: Vec<InvestmentTransaction>,
    pub tree: InvestmentsTree,
    investments_list: InvestmentsList,
    transactions_list: GenericList<InvestmentTransaction>,
    focus: InvestmentFocus,
    original_amount_edit: OriginalAmountEditState,
}

impl InvestmentsView {
    fn parse_amount_in_cents(input: &str) -> Result<i64, &'static str> {
        let input = input.trim();
        let (negative, amount) = match input.strip_prefix('-') {
            Some(amount) => (true, amount),
            None => (false, input.strip_prefix('+').unwrap_or(input)),
        };
        let mut parts = amount.split('.');
        let whole = parts.next().unwrap_or_default();
        let fraction = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.len() > 2
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("Enter an amount with up to two decimal places.");
        }

        let whole = whole
            .parse::<u64>()
            .map_err(|_| "Amount is out of range.")?;
        let fraction = match fraction.len() {
            0 => 0_u64,
            1 => fraction.parse::<u64>().map_err(|_| "Invalid amount.")? * 10,
            _ => fraction.parse::<u64>().map_err(|_| "Invalid amount.")?,
        };
        let cents = whole
            .checked_mul(100)
            .and_then(|value| value.checked_add(fraction))
            .ok_or("Amount is out of range.")?;
        if negative {
            if cents == i64::MAX as u64 + 1 {
                Ok(i64::MIN)
            } else {
                i64::try_from(cents)
                    .map(|amount| -amount)
                    .map_err(|_| "Amount is out of range.")
            }
        } else {
            i64::try_from(cents).map_err(|_| "Amount is out of range.")
        }
    }

    fn total_invested_by_investment(
        investments: &[Investment],
        investment_transactions: &[InvestmentTransaction],
    ) -> HashMap<String, i64> {
        let mut totals = HashMap::new();
        for investment in investments {
            if !investment_transactions
                .iter()
                .any(|transaction| transaction.investment_id == investment.identifier)
            {
                totals.insert(
                    investment.identifier.clone(),
                    investment.amount_original_in_cents,
                );
            }
        }
        for transaction in investment_transactions {
            let total = totals.entry(transaction.investment_id.clone()).or_insert(0);
            match &transaction.transaction_type {
                InvestmentTransactionType::Buy => *total += transaction.amount_in_cents,
                InvestmentTransactionType::Sell => *total -= transaction.amount_in_cents,
                _ => {}
            }
        }
        totals
    }

    fn create_transactions_list(
        transactions: Vec<InvestmentTransaction>,
    ) -> GenericList<InvestmentTransaction> {
        let columns = BTreeMap::from([
            (
                "Date".to_string(),
                ListColumn::new(20, |transaction: &InvestmentTransaction| {
                    transaction.date.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .with_style(|transaction| match &transaction.transaction_type {
                    InvestmentTransactionType::Buy => Style::default().fg(Color::Red),
                    InvestmentTransactionType::Sell => Style::default().fg(Color::Green),
                    _ => Style::default(),
                }),
            ),
            (
                "Quantity".to_string(),
                ListColumn::new(12, |transaction: &InvestmentTransaction| {
                    transaction
                        .quantity
                        .map(|quantity| format!("{quantity:.4}"))
                        .unwrap_or_else(|| "-".to_string())
                })
                .with_style(|transaction| match &transaction.transaction_type {
                    InvestmentTransactionType::Buy => Style::default().fg(Color::Red),
                    InvestmentTransactionType::Sell => Style::default().fg(Color::Green),
                    _ => Style::default(),
                }),
            ),
            (
                "Amount".to_string(),
                ListColumn::new(14, |transaction: &InvestmentTransaction| {
                    TUI::format_amount_for_display(transaction.amount_in_cents, 0)
                })
                .with_style(|transaction| match &transaction.transaction_type {
                    InvestmentTransactionType::Buy => Style::default().fg(Color::Red),
                    InvestmentTransactionType::Sell => Style::default().fg(Color::Green),
                    _ => Style::default(),
                }),
            ),
        ]);
        GenericList::new(transactions, columns)
            .with_title("Transactions")
            .with_column_order(["Date", "Quantity", "Amount"])
    }

    fn update_transactions_list(&mut self) {
        let transactions = self
            .investments_list
            .selected_investment()
            .map(|investment| {
                self.all_investment_transactions
                    .iter()
                    .filter(|transaction| transaction.investment_id == investment.identifier)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        self.transactions_list = Self::create_transactions_list(transactions);
    }

    fn update_investments_list(&mut self) {
        let filtered = match self.tree.selected_entry() {
            Some(entry) => {
                let type_name = entry.parent_name.as_deref().unwrap_or(&entry.name);
                let subtype_name = entry.parent_name.as_ref().map(|_| entry.name.clone());
                self.all_investments
                    .iter()
                    .filter(|investment| {
                        investment.status.eq_ignore_ascii_case("active")
                            && investment.investment_type.eq_ignore_ascii_case(type_name)
                            && subtype_name
                                .as_ref()
                                .map(|name| investment.subtype.eq_ignore_ascii_case(name))
                                .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            }
            None => self
                .all_investments
                .iter()
                .filter(|investment| investment.status.eq_ignore_ascii_case("active"))
                .cloned()
                .collect(),
        };

        self.investments_list = InvestmentsList::new(filtered);
        self.investments_list.show_date = false;
        self.investments_list.show_due_date = false;
        self.investments_list.show_name = true;
        self.investments_list.show_type = false;
        self.investments_list.show_subtype = false;
        self.investments_list.show_status = false;
        self.investments_list.show_amount = true;
        self.investments_list.show_percent_change = true;
        self.investments_list.show_time = false;
        self.investments_list.show_rate = false;
        self.investments_list.show_rate_type = false;
        self.investments_list.show_fixed_annual_rate = false;
        self.investments_list.title = Some("Investments".to_string());
        self.investments_list.focused = self.focus == InvestmentFocus::List;
        let invested_amounts = Self::total_invested_by_investment(
            &self.all_investments,
            &self.all_investment_transactions,
        );
        for row in &mut self.investments_list.rows {
            row.total_invested_in_cents = invested_amounts
                .get(&row.investment.identifier)
                .copied()
                .unwrap_or_default();
        }
        self.update_transactions_list();
    }

    pub fn new(
        investments: Vec<Investment>,
        investment_transactions: Vec<InvestmentTransaction>,
    ) -> Self {
        let invested_amounts =
            Self::total_invested_by_investment(&investments, &investment_transactions);
        let mut tree = InvestmentsTree::new();
        tree.update(&investments, &invested_amounts);
        let mut view = Self {
            all_investments: investments,
            all_investment_transactions: investment_transactions,
            tree,
            investments_list: InvestmentsList::new(Vec::new()),
            transactions_list: Self::create_transactions_list(Vec::new()),
            focus: InvestmentFocus::Tree,
            original_amount_edit: OriginalAmountEditState::Inactive,
        };
        view.update_investments_list();
        view
    }

    fn render_details(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Investment details");

        let lines = self
            .investments_list
            .selected_investment()
            .map(|investment| {
                let fields = [
                    (
                        "Database ID",
                        investment
                            .db_id
                            .map_or_else(|| "-".to_string(), |id| id.to_string()),
                    ),
                    ("Identifier", investment.identifier.clone()),
                    (
                        "Date",
                        investment.date.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    ),
                    (
                        "Due date",
                        investment
                            .due_date
                            .map(|date| date.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    ("Name", investment.name.clone()),
                    (
                        "Amount",
                        TUI::format_amount_for_display(investment.amount_in_cents, 0),
                    ),
                    (
                        "Original amount (e to edit)",
                        TUI::format_amount_for_display(investment.amount_original_in_cents, 0),
                    ),
                    ("Type", investment.investment_type.clone()),
                    ("Subtype", investment.subtype.clone()),
                    ("Status", investment.status.clone()),
                    (
                        "Rate",
                        investment
                            .rate
                            .map(|rate| format!("{rate:.2}%"))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Rate type",
                        investment
                            .rate_type
                            .clone()
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Rate periodicity",
                        investment
                            .rate_periodicity
                            .clone()
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Fixed annual rate",
                        investment
                            .fixed_annual_rate
                            .map(|rate| format!("{rate:.2}%"))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                ];
                fields
                    .into_iter()
                    .map(|(label, value)| {
                        Line::from(vec![
                            Span::styled(
                                format!("{label}: "),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(value, Style::default().fg(palette.budget)),
                        ])
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![Line::from("Select an investment from the list.")]);

        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
            area,
        );
    }
}

impl InteractableWidget for InvestmentsView {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let editing_amount = matches!(
            &self.original_amount_edit,
            OriginalAmountEditState::Editing { .. }
        );
        let edit_height = if editing_amount { 4 } else { 0 };
        let [content_area, edit_area, help_area] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(edit_height),
            Constraint::Length(2),
        ])
        .areas(area);
        let [panels_area, transactions_area] =
            Layout::vertical([Constraint::Percentage(65), Constraint::Min(1)]).areas(content_area);
        let [tree_area, list_area, details_area] = Layout::horizontal([
            Constraint::Percentage(45),
            Constraint::Percentage(30),
            Constraint::Min(1),
        ])
        .areas(panels_area);

        let palette = TUI::color_palette();
        let tree_block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.focus == InvestmentFocus::Tree {
                Style::default().fg(palette.budget)
            } else {
                Style::default()
            })
            .title("Types");
        let tree_inner = tree_block.inner(tree_area);
        frame.render_widget(tree_block, tree_area);
        self.tree.render(tree_inner, frame);
        self.investments_list.render(list_area, frame);
        self.render_details(details_area, frame);
        self.transactions_list.render(transactions_area, frame);
        if let OriginalAmountEditState::Editing { input, error } = &self.original_amount_edit {
            let [input_area, message_area] =
                Layout::vertical([Constraint::Length(3), Constraint::Length(1)]).areas(edit_area);
            input.render(input_area, frame);
            let message = error
                .as_deref()
                .unwrap_or("Enter saves; Esc cancels. Amount is in currency units.");
            frame.render_widget(Paragraph::new(message), message_area);
        }

        let help = if editing_amount {
            vec![
                Line::from("Edit amount: currency units, up to 2 decimals"),
                Line::from("Enter: save | Esc: cancel"),
            ]
        } else {
            vec![
                Line::from(
                    "Tab/Shift+Tab: panels | Up/Down: navigate | Tree: Left/Right collapse, Enter toggle",
                ),
                Line::from(
                    "Investment list: e edit original amount | Transactions: Up/Down | v values | l logger | q quit",
                ),
            ]
        };
        frame.render_widget(Paragraph::new(help).wrap(Wrap { trim: true }), help_area);
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        if let OriginalAmountEditState::Editing { input, error } = &mut self.original_amount_edit {
            match key.code {
                KeyCode::Esc => {
                    self.original_amount_edit = OriginalAmountEditState::Inactive;
                }
                KeyCode::Enter => {
                    let new_amount = match Self::parse_amount_in_cents(&input.state.text) {
                        Ok(amount) => amount,
                        Err(message) => {
                            *error = Some(message.to_string());
                            return ActionResult::Consumed;
                        }
                    };
                    let Some(identifier) = self
                        .investments_list
                        .selected_investment()
                        .map(|investment| investment.identifier.clone())
                    else {
                        self.original_amount_edit = OriginalAmountEditState::Inactive;
                        return ActionResult::Consumed;
                    };
                    if let Err(save_error) =
                        InvestmentDatabase::update_original_amount(&identifier, new_amount)
                    {
                        log::error!(
                            target: "midas",
                            "Failed to update original amount for investment {identifier}: {save_error}"
                        );
                        *error = Some(format!("Could not save amount: {save_error}"));
                        return ActionResult::Consumed;
                    }

                    if let Some(investment) = self
                        .all_investments
                        .iter_mut()
                        .find(|investment| investment.identifier == identifier)
                    {
                        investment.amount_original_in_cents = new_amount;
                    }
                    let selected_index = self.investments_list.state.selected_index;
                    let collapsed = self.tree.tree_state.collapsed.clone();
                    let invested_amounts = Self::total_invested_by_investment(
                        &self.all_investments,
                        &self.all_investment_transactions,
                    );
                    self.tree.update(&self.all_investments, &invested_amounts);
                    self.tree.tree_state.collapsed = collapsed;
                    self.update_investments_list();
                    self.investments_list.state.selected_index = selected_index;
                    self.update_transactions_list();
                    self.original_amount_edit = OriginalAmountEditState::Inactive;
                }
                _ => {
                    input.handle_key(key);
                    *error = None;
                }
            }
            return ActionResult::Consumed;
        }

        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    InvestmentFocus::Tree => InvestmentFocus::List,
                    InvestmentFocus::List => InvestmentFocus::Transactions,
                    InvestmentFocus::Transactions => InvestmentFocus::Tree,
                };
                self.investments_list.focused = self.focus == InvestmentFocus::List;
                ActionResult::Consumed
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    InvestmentFocus::Tree => InvestmentFocus::Transactions,
                    InvestmentFocus::List => InvestmentFocus::Tree,
                    InvestmentFocus::Transactions => InvestmentFocus::List,
                };
                self.investments_list.focused = self.focus == InvestmentFocus::List;
                ActionResult::Consumed
            }
            _ => match self.focus {
                InvestmentFocus::Tree => {
                    let result = self.tree.handle_key(key);
                    if result.consumed() {
                        self.update_investments_list();
                    }
                    result
                }
                InvestmentFocus::List => {
                    if key.code == KeyCode::Char('e') {
                        if let Some(investment) = self.investments_list.selected_investment() {
                            let mut input = InputBox::new("Original amount");
                            input.state.focused = true;
                            let amount = investment.amount_original_in_cents.unsigned_abs();
                            let sign = if investment.amount_original_in_cents < 0 {
                                "-"
                            } else {
                                ""
                            };
                            format!("{sign}{}.{:02}", amount / 100, amount % 100)
                                .chars()
                                .for_each(|character| input.state.insert_char(character));
                            self.original_amount_edit =
                                OriginalAmountEditState::Editing { input, error: None };
                            return ActionResult::Consumed;
                        }
                        return ActionResult::None;
                    }
                    let result = self.investments_list.handle_key(key);
                    if result.consumed() {
                        self.update_transactions_list();
                    }
                    result
                }
                InvestmentFocus::Transactions => self.transactions_list.handle_key(key),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn make_investment(
        name: &str,
        investment_type: &str,
        subtype: &str,
        status: &str,
    ) -> Investment {
        Investment {
            db_id: None,
            identifier: name.to_string(),
            date: Utc::now(),
            due_date: None,
            name: name.to_string(),
            amount_in_cents: 1000,
            amount_original_in_cents: 2000,
            investment_type: investment_type.to_string(),
            subtype: subtype.to_string(),
            status: status.to_string(),
            rate: None,
            rate_type: None,
            rate_periodicity: None,
            fixed_annual_rate: None,
        }
    }

    #[test]
    fn parses_original_amounts_as_cents() {
        assert_eq!(InvestmentsView::parse_amount_in_cents("123"), Ok(12_300));
        assert_eq!(InvestmentsView::parse_amount_in_cents("123.4"), Ok(12_340));
        assert_eq!(InvestmentsView::parse_amount_in_cents("-12.34"), Ok(-1_234));
        assert_eq!(
            InvestmentsView::parse_amount_in_cents("-92233720368547758.08"),
            Ok(i64::MIN)
        );
        assert!(InvestmentsView::parse_amount_in_cents("12.345").is_err());
        assert!(InvestmentsView::parse_amount_in_cents("invalid").is_err());
    }

    #[test]
    fn creates_tree_nodes_for_investment_types_and_subtypes() {
        let view = InvestmentsView::new(
            vec![
                make_investment("cash-1", "cash", "checking", "active"),
                make_investment("cash-2", "cash", "checking", "active"),
                make_investment("em-1", "emergency", "savings", "active"),
                make_investment("em-2", "emergency", "savings", "inactive"),
            ],
            vec![],
        );

        assert_eq!(view.tree.tree_nodes.len(), 2);
        assert_eq!(view.tree.tree_nodes[0].data.name, "cash");
        assert_eq!(view.tree.tree_nodes[0].children.len(), 1);
        assert_eq!(view.tree.tree_nodes[0].children[0].data.name, "checking");
        assert_eq!(view.tree.tree_nodes[1].data.name, "emergency");
        assert_eq!(view.tree.tree_nodes[1].children.len(), 1);
    }

    #[test]
    fn tab_focuses_the_investment_list_for_keyboard_navigation() {
        let mut view = InvestmentsView::new(
            vec![
                make_investment("cash-1", "cash", "checking", "active"),
                make_investment("cash-2", "cash", "checking", "active"),
            ],
            vec![],
        );

        assert_eq!(
            view.investments_list.selected_investment().unwrap().name,
            "cash-1"
        );
        assert!(
            view.handle_key(&KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
                .consumed()
        );
        assert!(
            view.handle_key(&KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
                .consumed()
        );
        assert_eq!(
            view.investments_list.selected_investment().unwrap().name,
            "cash-2"
        );
        assert!(view.investments_list.focused);

        view.handle_key(&KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert!(!view.investments_list.focused);
    }

    #[test]
    fn transactions_list_tracks_selected_investment_and_scrolls() {
        let transactions = vec![
            make_transaction("tx-1", "cash-1"),
            make_transaction("tx-2", "cash-1"),
            make_transaction("tx-3", "cash-2"),
        ];
        let mut view = InvestmentsView::new(
            vec![
                make_investment("cash-1", "cash", "checking", "active"),
                make_investment("cash-2", "cash", "checking", "active"),
            ],
            transactions,
        );

        assert_eq!(view.transactions_list.rows.len(), 2);
        assert_eq!(
            view.transactions_list.selected_row().unwrap().identifier,
            "tx-1"
        );
        view.handle_key(&KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        view.handle_key(&KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(
            view.handle_key(&KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
                .consumed()
        );
        assert_eq!(
            view.transactions_list.selected_row().unwrap().identifier,
            "tx-2"
        );
        view.handle_key(&KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        view.handle_key(&KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(view.transactions_list.rows.len(), 1);
        assert_eq!(
            view.transactions_list.selected_row().unwrap().identifier,
            "tx-3"
        );
    }

    #[test]
    fn investment_list_shows_buys_minus_sells_and_ignores_other_transactions() {
        let mut buy = make_transaction("buy", "cash-1");
        buy.amount_in_cents = 5_000;
        let mut sell = make_transaction("sell", "cash-1");
        sell.transaction_type = InvestmentTransactionType::Sell;
        sell.amount_in_cents = 1_000;
        let mut dividend = make_transaction("dividend", "cash-1");
        dividend.transaction_type = InvestmentTransactionType::Dividend;
        dividend.amount_in_cents = 9_000;

        let view = InvestmentsView::new(
            vec![make_investment("cash-1", "cash", "checking", "active")],
            vec![buy, sell, dividend],
        );

        assert_eq!(view.investments_list.rows[0].total_invested_in_cents, 4_000);
        let cash_tree_entry = &view.tree.tree_nodes[0].data;
        assert_eq!(cash_tree_entry.total_invested, 4_000);
        assert_eq!(cash_tree_entry.total_amount, 1_000);
        assert_eq!(
            cash_tree_entry.total_amount - cash_tree_entry.total_invested,
            -3_000
        );
    }

    #[test]
    fn investments_without_transactions_fall_back_to_original_amount() {
        let mut dividend = make_transaction("dividend", "dividend-only");
        dividend.transaction_type = InvestmentTransactionType::Dividend;

        let view = InvestmentsView::new(
            vec![
                make_investment("no-transactions", "cash", "checking", "active"),
                make_investment("dividend-only", "cash", "checking", "active"),
            ],
            vec![dividend],
        );

        let invested_by_id = view
            .investments_list
            .rows
            .iter()
            .map(|row| {
                (
                    row.investment.identifier.as_str(),
                    row.total_invested_in_cents,
                )
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(invested_by_id["no-transactions"], 2_000);
        assert_eq!(invested_by_id["dividend-only"], 0);
    }

    fn make_transaction(identifier: &str, investment_id: &str) -> InvestmentTransaction {
        InvestmentTransaction {
            db_id: None,
            identifier: identifier.to_string(),
            date: Utc::now(),
            transaction_type: InvestmentTransactionType::Buy,
            amount_in_cents: 1000,
            investment_id: investment_id.to_string(),
            price_per_unit_in_cents: None,
            quantity: Some(1.0),
            value_in_cents: None,
        }
    }
}
