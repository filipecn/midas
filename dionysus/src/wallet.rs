use binance::account::Account;
use binance::api::*;
use std::fs::read_to_string;

pub trait Wallet {}

pub struct BinanceWallet {
    pub account: Account,
}

impl Default for BinanceWallet {
    fn default() -> Self {
        BinanceWallet::new()
    }
}

impl BinanceWallet {
    pub fn new() -> Self {
        let keys: Vec<String> = read_to_string("/home/filipecn/dev/midas/keys")
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

#[cfg(test)]
mod tests {
    use crate::wallet::BinanceWallet;

    #[test]
    fn test_binance_wallet() {
        let wallet = BinanceWallet::new();

        match wallet.account.get_account() {
            Ok(answer) => println!("{:?}", answer.balances),
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
