use crate::{
    budget::{
        Category, Transaction, category::Classifier, category_picker::CategoryPicker,
        data::CategorizationRules, db::Transactions, rule_editor::RuleEditor,
        transactions_list::TransactionsList,
    },
    widgets::{ActionResult, InteractableWidget},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use regex::Regex;
use reqwest::redirect::Action;

enum TransactionEditState {
    Inactive,
    Editing {
        category_picker: CategoryPicker,
        target: CategoryTarget,
    },
    RuleEditing {
        rule_editor: RuleEditor,
    },
}

#[derive(Clone)]
enum CategoryTarget {
    Description(String),
    Transaction(usize),
}

pub struct TransactionsView {
    edit_state: TransactionEditState,
    pub transactions: Vec<Transaction>,
    view: TransactionsList,
}

impl TransactionsView {
    fn is_income_transaction(transaction: &Transaction) -> bool {
        transaction.amount_in_cents > 0
    }

    pub fn input_box_focused(&self) -> bool {
        match &self.edit_state {
            TransactionEditState::Inactive => false,
            TransactionEditState::Editing { .. } => true,
            TransactionEditState::RuleEditing { rule_editor } => {
                rule_editor.pattern.state.focused
                    || rule_editor.category_picker.filter.state.focused
            }
        }
    }

    pub fn new(transactions: Vec<Transaction>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut view = TransactionsList::new(
            transactions
                .iter()
                .enumerate()
                .map(|t| (t.0, t.1.clone()))
                .collect(),
        )?;
        view.show_account = false;
        view.show_merchant = false;
        Ok(TransactionsView {
            transactions: transactions.clone(),
            view,
            edit_state: TransactionEditState::Inactive,
        })
    }

    pub fn filter(
        &mut self,
        re: &str,
        category_type: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let regex = if re.is_empty() {
            None
        } else {
            Some(Regex::new(&re.to_uppercase()).unwrap())
        };
        let filtered_transactions: Vec<(usize, Transaction)> = if let Some(re) = &regex {
            self.transactions
                .iter()
                .enumerate()
                .filter_map(|t| {
                    let desc = t.1.description.clone().to_uppercase();
                    if re.is_match(&desc)
                        && category_type.is_none_or(|category_type| {
                            Self::is_income_transaction(&t.1) == (category_type == "income")
                        })
                    {
                        Some((t.0, t.1.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            self.transactions
                .iter()
                .enumerate()
                .filter(|(_, transaction)| {
                    category_type.is_none_or(|category_type| {
                        Self::is_income_transaction(transaction) == (category_type == "income")
                    })
                })
                .map(|t| (t.0, t.1.clone()))
                .collect()
        };
        self.view = TransactionsList::new(filtered_transactions)?;
        self.view.show_account = false;
        self.view.show_merchant = false;
        Ok(())
    }

    fn filter_uncategorized(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let transactions = self
            .transactions
            .iter()
            .enumerate()
            .filter(|(_, transaction)| transaction.is_uncategorized())
            .map(|(id, transaction)| (id, transaction.clone()))
            .collect();
        self.view = TransactionsList::new(transactions)?;
        self.view.show_account = false;
        self.view.show_merchant = false;
        Ok(())
    }

    pub fn fill_categories(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let classifier = Classifier::new()?;
        for row in self.view.rows.iter_mut() {
            if !row.transaction.is_uncategorized() {
                continue;
            }
            let label = classifier.label(&row.transaction);
            self.transactions[row.id].category = label.clone();
            row.transaction.category = label;
            Transactions::update_transaction(&self.transactions[row.id])?;
        }
        Ok(())
    }

    fn apply_category_to_all(
        &mut self,
        category: Category,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // edit rows
        for row in self.view.rows.iter_mut() {
            self.transactions[row.id].category = category.clone();
        }
        Transactions::update_transactions(&self.transactions)?;
        Ok(())
    }

    fn apply_category(
        &mut self,
        desc: &String,
        category: Category,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // edit rows
        for row in self.view.rows.iter_mut() {
            if row.transaction.description.contains(desc) {
                row.transaction.category = category.clone();
                self.transactions[row.id].category = category.clone();
                Transactions::update_transaction(&row.transaction)?;
            }
        }
        Ok(())
    }

    fn apply_category_to_transaction(
        &mut self,
        row_id: usize,
        category: Category,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(transaction) = self.transactions.get_mut(row_id) else {
            return Ok(());
        };
        transaction.category = category.clone();
        Transactions::update_transaction(transaction)?;

        if let Some(row) = self.view.rows.iter_mut().find(|row| row.id == row_id) {
            row.transaction.category = category;
        }

        Ok(())
    }
}

impl InteractableWidget for TransactionsView {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let [content_area, actions_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(4)])
            .areas(area);

        let mut constraints: Vec<Constraint> = Vec::new();
        if let TransactionEditState::Inactive = self.edit_state {
            constraints.push(Constraint::Min(0));
        } else {
            constraints.push(Constraint::Min(0));
            constraints.push(Constraint::Percentage(30));
        };
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(content_area);

        self.view.render(chunks[0], frame);

        match &self.edit_state {
            TransactionEditState::Inactive => {}
            TransactionEditState::Editing {
                category_picker, ..
            } => {
                category_picker.render(chunks[1], frame);
            }
            TransactionEditState::RuleEditing { rule_editor } => {
                rule_editor.render(chunks[1], frame);
            }
        }

        let actions = match &self.edit_state {
            TransactionEditState::Inactive => Text::from(vec![
                Line::from(
                    "↑/↓ Navigate   Enter Category by description   C Category this row   c Create/apply rule",
                ),
                Line::from(
                    "u Uncategorized   r Auto-categorize   v Toggle sensitive values   l Logger   q Quit",
                ),
            ]),
            TransactionEditState::Editing { .. } => Text::from(vec![
                Line::from("↑/↓ Navigate categories   ←/→ Collapse/expand   Type to filter"),
                Line::from("Enter Select category   Esc Cancel"),
            ]),
            TransactionEditState::RuleEditing { .. } => Text::from(vec![
                Line::from("Type pattern   Tab/Shift+Tab Change focus   ↑/↓ Navigate categories"),
                Line::from("Enter Activate focused action   Esc Cancel"),
            ]),
        };
        frame.render_widget(
            Paragraph::new(actions)
                .block(Block::default().borders(Borders::ALL).title(" Actions ")),
            actions_area,
        );
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        let mut result = ActionResult::Consumed;
        match self.edit_state {
            TransactionEditState::Inactive => {
                if let ActionResult::SelectTransaction(_) = self.view.handle_key(key) {
                    let category_picker = CategoryPicker::new().unwrap();
                    if let Some(row) = self.view.rows.get(self.view.state.selected_index) {
                        self.edit_state = TransactionEditState::Editing {
                            category_picker,
                            target: CategoryTarget::Description(
                                row.transaction.description.clone(),
                            ),
                        };
                    }
                } else if key.code == crossterm::event::KeyCode::Char('C') {
                    if let Some(row) = self.view.rows.get(self.view.state.selected_index) {
                        let category_picker = CategoryPicker::new().unwrap();
                        self.edit_state = TransactionEditState::Editing {
                            category_picker,
                            target: CategoryTarget::Transaction(row.id),
                        };
                    }
                } else if key.code.is_char('c') {
                    if let Some(row) = self.view.rows.get(self.view.state.selected_index) {
                        let category_type = if Self::is_income_transaction(&row.transaction) {
                            "income"
                        } else {
                            "expense"
                        };
                        let rule_editor = RuleEditor::new(category_type).unwrap();
                        self.edit_state = TransactionEditState::RuleEditing { rule_editor };
                    }
                } else if key.code.is_char('u') {
                    let _ = self.filter_uncategorized();
                } else if key.code.is_char('r') {
                    let _ = self.fill_categories();
                } else {
                    result = ActionResult::None;
                }
            }
            TransactionEditState::Editing {
                ref mut category_picker,
                ref target,
            } => match category_picker.handle_key(key) {
                ActionResult::Category(category) => {
                    match target.clone() {
                        CategoryTarget::Description(description) => {
                            let _ = self.apply_category(&description, category);
                        }
                        CategoryTarget::Transaction(row_id) => {
                            if let Err(error) = self.apply_category_to_transaction(row_id, category)
                            {
                                log::error!(
                                    target: "midas",
                                    "Failed to update category for transaction row {row_id}: {error}"
                                );
                            }
                        }
                    }
                    self.edit_state = TransactionEditState::Inactive;
                }
                ActionResult::Cancel => {
                    self.edit_state = TransactionEditState::Inactive;
                }
                ActionResult::None => result = ActionResult::None,
                _ => result = ActionResult::None,
            },
            TransactionEditState::RuleEditing {
                ref mut rule_editor,
            } => match rule_editor.handle_key(key) {
                ActionResult::PatternChanged(pattern) => {
                    let category_type = rule_editor.category_type.clone();
                    let _ = self.filter(&pattern, Some(&category_type));
                }
                ActionResult::SubmitRule(pattern, category) => {
                    let _ = CategorizationRules::add(pattern.as_str(), &category);
                    let _ = self.apply_category_to_all(category);
                    // update json rule
                    if let Err(e) = CategorizationRules::write() {
                        dbg!(e);
                    }
                    self.edit_state = TransactionEditState::Inactive;
                }
                ActionResult::Cancel => {
                    self.edit_state = TransactionEditState::Inactive;
                }
                ActionResult::Category(category) => {
                    let _ = self.apply_category_to_all(category);
                }
                _ => result = ActionResult::None,
            },
        };
        result
    }
}
