use std::{cell::RefCell, collections::HashMap, fs::File, path::PathBuf};

use serde::Deserialize;

thread_local! {
    static STATIC_CORE_DATA: RefCell<StaticCoreData> = {
        RefCell::new(StaticCoreData::default())
    }
}

#[macro_export]
macro_rules! get_static_core_data {
    ($field:ident) => {
        $crate::STATIC_CORE_DATA.with_borrow(|s| s.$field.clone())
    };
}

#[derive(Default)]
pub enum FetchState<T> {
    #[default]
    Idle,
    Loading,
    Loaded(T),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct Account {
    pub name: String,
    pub account_type: Option<String>,
    pub identifier: String,
}

pub struct Accounts;

pub struct StaticCoreData {
    pub accounts: HashMap<String, Account>,
    pub account_colors: HashMap<String, ratatui::style::Color>,
}

impl StaticCoreData {
    pub fn get_account_color(&self, account_id: &str) -> Option<ratatui::style::Color> {
        self.account_colors.get(account_id).cloned()
    }
}

impl Default for StaticCoreData {
    fn default() -> Self {
        Self {
            accounts: HashMap::new(),
            account_colors: HashMap::new(),
        }
    }
}

#[derive(Deserialize)]
pub struct AccountJson {
    pub name: String,
    #[serde(default)]
    pub account_type: Option<String>,
    pub identifier: String,
}

impl Accounts {
    fn color_for_account(account_id: &str) -> ratatui::style::Color {
        let hash = account_id.bytes().fold(0u32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as u32)
        });
        ratatui::style::Color::Indexed(16 + (hash % 216) as u8)
    }
    pub fn load(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let f = File::open(path)?;
        let json_accounts: Vec<AccountJson> = serde_json::from_reader(f)?;
        STATIC_CORE_DATA.with_borrow_mut(|sd| {
            sd.accounts = json_accounts
                .iter()
                .map(|account| {
                    (
                        account.identifier.clone(),
                        Account {
                            name: account.name.clone(),
                            account_type: account.account_type.clone(),
                            identifier: account.identifier.clone(),
                        },
                    )
                })
                .collect();
            sd.account_colors = sd
                .accounts
                .keys()
                .map(|account_id| (account_id.clone(), Self::color_for_account(account_id)))
                .collect();
        });
        Ok(())
    }
    pub fn add(accounts: &[Account]) {
        STATIC_CORE_DATA.with_borrow_mut(|sd| {
            for account in accounts.iter() {
                if !sd.accounts.contains_key(&account.identifier) {
                    sd.account_colors.insert(
                        account.identifier.clone(),
                        Self::color_for_account(&account.identifier),
                    );
                    sd.accounts
                        .insert(account.identifier.clone(), account.clone());
                }
            }
        })
    }
    pub fn get_all() -> Vec<Account> {
        STATIC_CORE_DATA.with_borrow(|sd| {
            sd.accounts
                .iter()
                .map(|(_, account)| account.clone())
                .collect()
        })
    }
    pub fn get_type(account_id: &str) -> Option<String> {
        STATIC_CORE_DATA.with_borrow(|sd| {
            sd.accounts
                .get(account_id)
                .and_then(|account| account.account_type.clone())
        })
    }
    pub fn get_account_color(account_id: &str) -> Option<ratatui::style::Color> {
        STATIC_CORE_DATA.with_borrow(|sd| sd.account_colors.get(account_id).cloned())
    }
}
