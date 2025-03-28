use crate::finance::{DiError, Position, Token};
use crate::ERROR;
use binance::account::Account;
use binance::api::*;
use slog::slog_error;
use std::collections::HashMap;
use std::fs::read_to_string;

#[derive(Debug, Default)]
pub struct Asset {
    pub symbol: Token,
    pub free: f64,
}

#[derive(Default)]
pub struct Wallet {
    capital: HashMap<Token, f64>,
    positions: HashMap<Token, Position>,
    balance: HashMap<Token, Asset>,
}

pub trait DigitalWallet {
    fn get_balance(&self) -> Result<HashMap<Token, Asset>, DiError>;
}

pub struct BinanceWallet {
    pub account: Account,
}

impl Default for BinanceWallet {
    fn default() -> Self {
        BinanceWallet::new("/home/filipecn/dev/midas/keys")
    }
}

impl BinanceWallet {
    pub fn new(keys_file: &str) -> Self {
        let keys: Vec<String> = read_to_string(&keys_file)
            .unwrap() // panic on possible file-reading errors
            .lines() // split the string into an iterator of string slices
            .map(String::from) // make each slice into a string
            .collect();
        let secret_key = Some(keys[0].clone().into());
        let api_key = Some(keys[1].clone().into());
        Self {
            account: Binance::new(api_key, secret_key),
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
                                symbol: Token::Symbol(x.asset.clone()),
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

#[cfg(test)]
mod tests {
    use crate::wallet::BinanceWallet;

    #[test]
    fn test_binance_wallet() {
        let wallet = BinanceWallet::new("/home/filipecn/dev/midas/keys");

        match wallet.account.get_account() {
            Ok(answer) => println!("{:?}", answer.balances),
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
