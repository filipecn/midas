use chrono::{DateTime, Utc};

pub mod category;
pub mod category_picker;
pub mod category_view;
pub mod data;
pub mod expense_group_tree;
pub mod expense_groups_view;
pub mod rule_editor;
pub mod transactions_list;
pub mod transactions_view;

pub use crate::widgets::ActionResult;

#[derive(Clone, Debug)]
pub struct Category {
    pub name: String,
    pub category_type: String,
    pub frequency: String,
    pub description: Option<String>,
    pub parent: Option<String>,
}

impl Category {
    pub fn uncategorized() -> Self {
        Self {
            name: String::from("Uncategorized"),
            category_type: String::from("expense"),
            frequency: String::from("variable"),
            description: None,
            parent: None,
        }
    }
    pub fn inverse_pair(&self) -> (String, String) {
        if let Some(parent) = &self.parent {
            (parent.clone(), self.name.clone())
        } else {
            (self.name.clone(), String::default())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorizationRule {
    pub pattern: String,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub db_id: Option<i64>,
    pub identifier: String,
    pub date: DateTime<Utc>,
    pub amount_in_cents: i64,
    pub account_id: String,
    pub category: Category,
    pub description: String,
    pub merchant: String,
}

impl Transaction {
    pub fn is_uncategorized(&self) -> bool {
        self.category.name.to_uppercase() == "UNCATEGORIZED"
    }
}
