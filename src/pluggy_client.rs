use chrono::DateTime;
use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::budget::category::Classifier;
use crate::budget::category::HintData;
use crate::budget::{Category, Transaction};
use crate::core::Account;
use crate::investments::Investment;
use crate::investments::InvestmentTransaction;

const BASE_URL: &str = "https://api.pluggy.ai";
// 2 hours minus 60 seconds
const API_KEY_TTL: Duration = Duration::from_secs(2 * 60 * 60 - 60);

thread_local! {
    pub static PLUGGY: RefCell<PluggyClient> = {
        RefCell::new(PluggyClient::default())
    }
}

#[derive(Debug, Deserialize)]
pub struct PluggyApiErrorPayload {
    pub code: Option<u16>,
    pub message: String,
    pub parameter: Option<String>,
}

#[derive(Error, Debug)]
pub enum PluggyError {
    #[error("API key is missing or invalid")]
    MissingApiKey,

    #[error("Pluggy API request failed [HTTP {status}]: {message}")]
    Api {
        status: reqwest::StatusCode,
        message: String,
        parameter: Option<String>,
    },

    #[error("Network/HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse JSON response: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Failed to write transaction log: {0}")]
    Log(#[from] std::io::Error),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthRequest<'a> {
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    api_key: String,
}

#[derive(Deserialize)]
struct AccountResponse {
    number: Option<String>,
}

#[derive(Deserialize)]
struct TransactionsResponse {
    results: Vec<Value>,
}

#[derive(Deserialize, Debug, Default)]
struct AccountsResponse {
    results: Vec<PluggyAccount>,
}

#[derive(Deserialize, Debug)]
struct PluggyAccount {
    id: String,
    #[serde(rename = "type")]
    account_type: String,
    subtype: Option<String>,
    name: Option<String>,
}

pub struct PluggyClient {
    client_id: String,
    client_secret: String,
    item_id: Option<String>,
    http: Client,
    api_key: Option<String>,
    api_key_expires_at: Instant,
    accounts: Vec<PluggyAccount>,
}

impl Default for PluggyClient {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            item_id: None,
            http: Client::default(),
            api_key: None,
            api_key_expires_at: Instant::now(),
            accounts: Vec::new(),
        }
    }
}

impl PluggyClient {
    pub fn init(
        &mut self,
        client_id: String,
        client_secret: String,
        item_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client_id = client_id;
        self.client_secret = client_secret;
        self.item_id = item_id;
        self.http = Client::new();
        let auth_api_key = self.get_api_key()?;
        // (Fetch accounts & loop results)
        self.accounts = if let Some(id) = self.item_id.clone() {
            let url = format!("https://api.pluggy.ai/accounts?itemId={}", id);
            self.http
                .get(&url)
                .header("X-API-KEY", &auth_api_key)
                .send()?
                .json::<AccountsResponse>()?
                .results
        } else {
            Vec::new()
        };

        Ok(())
    }

    fn get_api_key(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(ref key) = self.api_key {
            if Instant::now() < self.api_key_expires_at {
                return Ok(key.clone());
            }
        }

        // Authenticate & fetch API Key
        let auth_payload = AuthRequest {
            client_id: &self.client_id,
            client_secret: &self.client_secret,
        };
        let auth_res = self
            .http
            .post(format!("{}/auth", BASE_URL))
            .json(&auth_payload)
            .send()?
            .json::<AuthResponse>()?;

        self.api_key = Some(auth_res.api_key.clone());
        self.api_key_expires_at = Instant::now() + API_KEY_TTL;

        Ok(auth_res.api_key)
    }

    pub fn get_accounts(&self) -> Vec<Account> {
        self.accounts
            .iter()
            .map(|pa| Account {
                name: pa.name.clone().unwrap_or_default(),
                account_type: Some(pa.account_type.clone()),
                identifier: pa.id.clone(),
            })
            .collect()
    }
    /// Ask Pluggy to refresh the connection with the institution now, instead of
    /// waiting for the next scheduled auto-sync (which can be up to 24h away).
    /// Pluggy rate-limits manual triggers (1/hour for new clients), so a failure
    /// here is logged and swallowed -- we still read whatever data is already
    /// available rather than aborting the run.
    pub fn trigger_sync(&mut self, item_id: &str) {
        let api_key = match self.get_api_key() {
            Ok(key) => key,
            Err(e) => {
                println!("could not trigger Pluggy sync due to auth failure: {}", e);
                return;
            }
        };

        let url = format!("{}/items/{}", BASE_URL, item_id);
        let response = self
            .http
            .patch(&url)
            .header("X-API-KEY", &api_key)
            .json(&serde_json::json!({}))
            .send();

        match response {
            Ok(res) => {
                if let Err(exc) = res.error_for_status() {
                    println!(
                        "could not trigger Pluggy sync, using existing data: {}",
                        exc
                    );
                }
            }
            Err(exc) => {
                println!(
                    "could not trigger Pluggy sync, using existing data: {}",
                    exc
                );
            }
        }
    }

    pub fn get_recent_transactions(
        &mut self,
        account_id: &str,
        lookback_days: i64,
    ) -> Result<Vec<Transaction>, PluggyError> {
        let api_key = self.get_api_key().map_err(|_| PluggyError::MissingApiKey)?;

        let date_from = (chrono::Utc::now() - chrono::Duration::days(lookback_days))
            .format("%Y-%m-%d")
            .to_string();

        let url = format!("{}/v2/transactions", BASE_URL);

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TransactionsResponse {
            results: Vec<PluggyTransaction>,
            next_page: Option<String>,
        }

        let classifier = Classifier::new().map_err(|e| PluggyError::Api {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to initialize classifier: {}", e),
            parameter: None,
        })?;

        let mut all_transactions = Vec::new();
        let mut next_page_cursor: Option<String> = None;
        let mut transaction_log = OpenOptions::new()
            .create(true)
            .append(true)
            .open("pluggy_transactions.log")?;

        loop {
            // Construct query parameters using dateFrom and optional nextPage cursor
            let mut query_params = vec![
                ("accountId", account_id.to_string()),
                ("dateFrom", date_from.clone()),
            ];

            if let Some(ref cursor) = next_page_cursor {
                query_params.push(("nextPage", cursor.clone()));
            }

            let response = self
                .http
                .get(&url)
                .header("X-API-KEY", &api_key)
                .query(&query_params)
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();

                let (message, parameter) =
                    match serde_json::from_str::<PluggyApiErrorPayload>(&body) {
                        Ok(payload) => (payload.message, payload.parameter),
                        Err(_) => (body, None),
                    };

                return Err(PluggyError::Api {
                    status,
                    message,
                    parameter,
                });
            }

            // let payload = response.json::<TransactionsResponse>()?;
            let raw_text = response.text()?;
            let payload: TransactionsResponse = match serde_json::from_str(&raw_text) {
                Ok(parsed) => parsed,
                Err(err) => {
                    eprintln!("=== JSON DECODING ERROR ===");
                    eprintln!("Serde Error: {err}");
                    eprintln!("Raw JSON Body:\n{raw_text}");
                    eprintln!("===========================");
                    return Err(PluggyError::Json(err));
                }
            };
            let has_results = !payload.results.is_empty();

            for pt in payload.results {
                let serialized_transaction = serde_json::to_string(&pt)?;
                writeln!(transaction_log, "{serialized_transaction}")?;

                let merchant = pt
                    .merchant
                    .as_ref()
                    .and_then(|m| m.business_name.as_deref().or(m.name.as_deref()))
                    .unwrap_or("")
                    .to_string();

                let amount = (pt.amount.unwrap_or_default() * 100.0).round() as i64;
                let amount_in_cents = match pt.r#type.as_deref() {
                    Some("CREDIT") => amount.abs(),
                    Some("DEBIT") => -amount.abs(),
                    _ => amount,
                };
                let mut transaction = Transaction {
                    db_id: None,
                    identifier: pt.id,
                    date: pt.date.unwrap_or_else(Utc::now),
                    amount_in_cents,
                    account_id: pt.account_id.unwrap_or_default(),
                    category: Category::uncategorized(),
                    description: pt.description.unwrap_or_default(),
                    merchant,
                };

                transaction.category = classifier.label_with_hint(
                    &transaction,
                    HintData {
                        category: pt.category.clone(),
                        category_id: pt.category_id.clone(),
                    },
                );

                all_transactions.push(transaction);
            }

            // Advance to next page cursor or terminate if None/empty
            match payload.next_page {
                Some(cursor) if !cursor.is_empty() && has_results => {
                    next_page_cursor = Some(cursor);
                    // Sleep 250ms before fetching the next page
                    thread::sleep(Duration::from_millis(250));
                }
                _ => break,
            }
        }

        Ok(all_transactions)
    }

    pub fn get_investments(&mut self) -> Result<Vec<Investment>, PluggyError> {
        let api_key = self.get_api_key().map_err(|_| PluggyError::MissingApiKey)?;
        let url = format!("{}/investments", BASE_URL);

        let mut all_investments: Vec<Investment> = Vec::new();
        let mut current_page: u32 = 1;
        let mut investment_log = OpenOptions::new()
            .create(true)
            .append(true)
            .open("pluggy_investments.log")?;

        loop {
            let item_id_str = self.item_id.as_ref().ok_or(PluggyError::MissingApiKey)?;
            let page_str = current_page.to_string();
            let query_params = [
                ("itemId", item_id_str.as_str()),
                ("page", &page_str),
                ("pageSize", "500"),
            ];

            let response = self
                .http
                .get(&url)
                .header("X-API-KEY", &api_key)
                .query(&query_params)
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();

                let (message, parameter) =
                    match serde_json::from_str::<PluggyApiErrorPayload>(&body) {
                        Ok(payload) => (payload.message, payload.parameter),
                        Err(_) => (body, None),
                    };

                return Err(PluggyError::Api {
                    status,
                    message,
                    parameter,
                });
            }

            let raw_text = response.text()?;
            let payload: InvestmentsResponse = match serde_json::from_str(&raw_text) {
                Ok(parsed) => parsed,
                Err(err) => {
                    eprintln!("=== INVESTMENTS DESERIALIZATION ERROR ===");
                    eprintln!("Serde Error: {err}");
                    eprintln!("Raw Body:\n{raw_text}");
                    eprintln!("========================================");
                    return Err(PluggyError::Json(err));
                }
            };

            let total_pages = payload.total_pages.unwrap_or(current_page);
            let is_empty = payload.results.is_empty();

            for pluggy_investment in payload.results {
                let serialized_investment = serde_json::to_string(&pluggy_investment)?;
                writeln!(investment_log, "{serialized_investment}")?;

                let amount_in_cents =
                    (pluggy_investment.amount.unwrap_or_default() * 100.0).round() as i64;
                let amount_original_in_cents =
                    (pluggy_investment.amount_original.unwrap_or_default() * 100.0).round() as i64;
                let investment = Investment {
                    db_id: None,
                    identifier: pluggy_investment.id.clone(),
                    date: pluggy_investment
                        .date
                        .as_ref()
                        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    due_date: pluggy_investment
                        .due_date
                        .as_ref()
                        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    name: pluggy_investment.name.clone().unwrap_or_default(),
                    amount_in_cents,
                    amount_original_in_cents,
                    investment_type: pluggy_investment
                        .investment_type
                        .clone()
                        .unwrap_or_default(),
                    subtype: pluggy_investment.subtype.clone().unwrap_or_default(),
                    status: pluggy_investment.status.clone().unwrap_or_default(),
                    rate: pluggy_investment.rate,
                    rate_type: pluggy_investment.rate_type.clone(),
                    rate_periodicity: pluggy_investment.rate_periodicity.clone(),
                    fixed_annual_rate: pluggy_investment.fixed_annual_rate,
                };
                all_investments.push(investment);
            }

            if current_page >= total_pages || is_empty {
                break;
            }

            current_page += 1;
        }

        Ok(all_investments)
    }

    pub fn get_investment_transactions(
        &mut self,
        investment_id: &str,
    ) -> Result<Vec<InvestmentTransaction>, PluggyError> {
        let api_key = self.get_api_key().map_err(|_| PluggyError::MissingApiKey)?;
        let url = format!("{}/investments/{}/transactions", BASE_URL, investment_id);

        let mut all_transactions: Vec<InvestmentTransaction> = Vec::new();
        let mut current_page: u32 = 1;
        let mut transaction_log = OpenOptions::new()
            .create(true)
            .append(true)
            .open("pluggy_investment_transactions.log")?;

        loop {
            let page_str = current_page.to_string();
            let query_params = [("page", page_str.as_str()), ("pageSize", "500")];

            let response = self
                .http
                .get(&url)
                .header("X-API-KEY", &api_key)
                .query(&query_params)
                .send()?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();

                let (message, parameter) =
                    match serde_json::from_str::<PluggyApiErrorPayload>(&body) {
                        Ok(payload) => (payload.message, payload.parameter),
                        Err(_) => (body, None),
                    };

                return Err(PluggyError::Api {
                    status,
                    message,
                    parameter,
                });
            }

            let raw_text = response.text()?;
            let payload: InvestmentTransactionsResponse = match serde_json::from_str(&raw_text) {
                Ok(parsed) => parsed,
                Err(err) => {
                    eprintln!("=== INVESTMENT TRANSACTIONS DESERIALIZATION ERROR ===");
                    eprintln!("Serde Error: {err}");
                    eprintln!("Raw Body:\n{raw_text}");
                    eprintln!("====================================================");
                    return Err(PluggyError::Json(err));
                }
            };

            let total_pages = payload.total_pages.unwrap_or(current_page);
            let is_empty = payload.results.is_empty();

            for pluggy_investment_transaction in payload.results {
                let serialized_transaction = serde_json::to_string(&pluggy_investment_transaction)?;
                writeln!(transaction_log, "{serialized_transaction}")?;

                let amount_in_cents = (pluggy_investment_transaction.amount.unwrap_or_default()
                    * 100.0)
                    .round() as i64;
                let price_per_unit_in_cents = pluggy_investment_transaction
                    .price
                    .map(|p| (p * 100.0).round() as i64);
                let value_in_cents = pluggy_investment_transaction
                    .value
                    .map(|v| (v * 100.0).round() as i64);
                let transaction = InvestmentTransaction {
                    db_id: None,
                    identifier: pluggy_investment_transaction.id.clone(),
                    date: pluggy_investment_transaction
                        .date
                        .as_ref()
                        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    transaction_type: pluggy_investment_transaction
                        .transaction_type
                        .as_deref()
                        .unwrap_or("Other")
                        .into(),
                    amount_in_cents,
                    investment_id: investment_id.to_string(),
                    price_per_unit_in_cents,
                    quantity: pluggy_investment_transaction.quantity,
                    value_in_cents,
                };
                all_transactions.push(transaction);
            }

            if current_page >= total_pages || is_empty {
                break;
            }

            current_page += 1;
        }

        Ok(all_transactions)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluggyTransaction {
    pub id: String,
    pub account_id: Option<String>,
    pub amount: Option<f64>,
    pub amount_in_account_currency: Option<f64>,
    pub balance: Option<f64>,
    pub category: Option<String>,
    pub category_id: Option<String>,
    pub currency_code: Option<String>,
    pub description: Option<String>,
    pub description_raw: Option<String>,
    pub operation_type: Option<String>,
    pub operation_type_additional_info: Option<String>,
    pub order: Option<i32>,
    pub status: Option<String>,
    pub r#type: Option<String>, // "type" is a reserved keyword in Rust, so we prefix it with r#
    pub payment_data: Option<PaymentData>,
    pub merchant: Option<PluggyMerchant>,
    pub provider_code: Option<String>,
    pub provider_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub date: Option<DateTime<Utc>>,
    pub credit_card_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluggyMerchant {
    pub name: Option<String>,
    pub business_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentData {
    pub payment_method: String,
    pub payer: Option<PartyDetails>,
    pub receiver: Option<PartyDetails>,
    pub boleto_metadata: Option<serde_json::Value>,
    pub reason: Option<String>,
    pub receiver_reference_id: Option<String>,
    pub reference_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartyDetails {
    pub name: Option<String>,
    pub account_number: Option<String>,
    pub branch_number: Option<String>,
    pub routing_number: Option<String>,
    pub routing_number_ispb: Option<String>,
    pub document_number: Option<DocumentNumber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentNumber {
    pub r#type: String, // e.g., "CPF" or "CNPJ"
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluggyInvestment {
    pub id: String,
    pub item_id: Option<String>,
    pub code: Option<String>,
    pub name: Option<String>,
    pub number: Option<String>,
    pub balance: Option<f64>,
    pub amount: Option<f64>,
    pub amount_original: Option<f64>,
    pub currency_code: Option<String>,
    #[serde(rename = "type")]
    pub investment_type: Option<String>, // Renamed because 'type' is a reserved Rust keyword
    pub subtype: Option<String>,
    pub last_month_rate: Option<f64>,
    pub annual_rate: Option<f64>,
    pub date: Option<String>,
    pub due_date: Option<String>,
    pub status: Option<String>,
    pub rate: Option<f64>,
    pub rate_type: Option<String>,
    pub rate_periodicity: Option<String>,
    pub fixed_annual_rate: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvestmentsResponse {
    results: Vec<PluggyInvestment>,
    page: Option<u32>,
    total_pages: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluggyInvestmentTransaction {
    pub id: String,
    pub investment_id: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>, // e.g., "BUY", "SELL", "DIVIDEND", "TAX"
    pub quantity: Option<f64>,
    pub value: Option<f64>,
    pub amount: Option<f64>,
    pub date: Option<String>,
    pub trade_date: Option<String>,
    pub settlement_date: Option<String>,
    pub price: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvestmentTransactionsResponse {
    results: Vec<PluggyInvestmentTransaction>,
    page: Option<u32>,
    total_pages: Option<u32>,
}
