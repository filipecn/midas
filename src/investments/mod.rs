use chrono::{DateTime, Utc};
use std::fmt;

pub mod db;
pub mod investment_transactions_list;
pub mod investment_transactions_view;
pub mod investments_list;
pub mod investments_tree;
pub mod investments_view;

#[derive(Debug, Clone)]
pub struct Investment {
    pub db_id: Option<i64>,
    pub identifier: String,
    pub date: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub name: String,
    pub amount_in_cents: i64,
    pub amount_original_in_cents: i64,
    pub investment_type: String,
    pub subtype: String,
    pub status: String,
    pub rate: Option<f64>,
    pub rate_type: Option<String>,
    pub rate_periodicity: Option<String>,
    pub fixed_annual_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InvestmentTransactionType {
    Buy,
    Sell,
    Dividend,
    Interest,
    Fee,
    Other(String),
}

impl From<&str> for InvestmentTransactionType {
    fn from(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "buy" => Self::Buy,
            "sell" => Self::Sell,
            "dividend" => Self::Dividend,
            "interest" => Self::Interest,
            "fee" => Self::Fee,
            _ => Self::Other(value.to_string()),
        }
    }
}

impl From<String> for InvestmentTransactionType {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<InvestmentTransactionType> for String {
    fn from(value: InvestmentTransactionType) -> Self {
        match value {
            InvestmentTransactionType::Buy => "Buy".to_string(),
            InvestmentTransactionType::Sell => "Sell".to_string(),
            InvestmentTransactionType::Dividend => "Dividend".to_string(),
            InvestmentTransactionType::Interest => "Interest".to_string(),
            InvestmentTransactionType::Fee => "Fee".to_string(),
            InvestmentTransactionType::Other(value) => value,
        }
    }
}

impl fmt::Display for InvestmentTransactionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&String::from(self.clone()))
    }
}

#[derive(Debug, Clone)]
pub struct InvestmentTransaction {
    pub db_id: Option<i64>,
    pub identifier: String,
    pub date: DateTime<Utc>,
    pub transaction_type: InvestmentTransactionType,
    pub amount_in_cents: i64,
    pub investment_id: String,
    pub price_per_unit_in_cents: Option<i64>,
    pub quantity: Option<f64>,
    pub value_in_cents: Option<i64>,
}
