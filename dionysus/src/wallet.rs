use crate::finance::{DiError, Token};
use binance::account::Account;
use binance::api::*;
use binance::config::Config;
use std::collections::HashMap;
use std::fs::read_to_string;

#[derive(Debug, Default)]
pub struct Asset {
    pub free: f64,
}

pub trait DigitalWallet {
    fn get_balance(&self) -> Result<HashMap<Token, Asset>, DiError>;
}

pub struct BinanceWallet {
    pub account: Account,
}

impl Default for BinanceWallet {
    fn default() -> Self {
        BinanceWallet::new("", false)
    }
}

impl BinanceWallet {
    pub fn new(keys_file: &str, use_test_api: bool) -> Self {
        let keys: Vec<String> = read_to_string(&keys_file)
            .unwrap() // panic on possible file-reading errors
            .lines() // split the string into an iterator of string slices
            .map(String::from) // make each slice into a string
            .collect();
        let secret_key = Some(keys[0].clone().into());
        let api_key = Some(keys[1].clone().into());
        if use_test_api {
            let config = Config::default().set_rest_api_endpoint("https://testnet.binance.vision");
            Self {
                account: Binance::new_with_config(None, None, &config),
            }
        } else {
            Self {
                account: Binance::new(api_key, secret_key),
            }
        }
    }
}

impl DigitalWallet for BinanceWallet {
    fn get_balance(&self) -> Result<HashMap<Token, Asset>, DiError> {
        match self.account.get_account() {
            Ok(answer) => {
                let items: HashMap<Token, Asset> = answer
                    .balances
                    .into_iter()
                    .map(|x| {
                        (
                            Token::Symbol(x.asset.clone()),
                            Asset {
                                free: x.free.parse::<f64>().unwrap_or(0.0),
                            },
                        )
                    })
                    .filter(|(_, a)| a.free > 0.0)
                    .collect();
                Ok(items)
            }
            Err(e) => Err(DiError::Message(format!("{:?}", e))),
        }
    }
}
