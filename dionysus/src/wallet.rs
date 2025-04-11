use crate::finance::{DiError, Order, OrderType, Position, Side, TimeInForce, Token};
use binance::account::Account;
use binance::api::*;
use binance::model::Transaction;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Ancestors;

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

pub trait Trader {
    fn create_order(&self, order: &Order) -> Result<u64, DiError>;
}

pub struct BinanceWallet {
    pub account: Account,
}

impl Default for BinanceWallet {
    fn default() -> Self {
        BinanceWallet::new("/home/filipecn/dev/midas/keys")
    }
}

fn convert_tif(tif: &TimeInForce) -> binance::account::TimeInForce {
    match tif {
        TimeInForce::FOK => binance::account::TimeInForce::FOK,
        TimeInForce::IOC => binance::account::TimeInForce::IOC,
        TimeInForce::GTC => binance::account::TimeInForce::GTC,
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

    pub fn buy_order(&self, order: &Order) -> Result<Transaction, DiError> {
        let symbol = order.token.to_string();
        match order.order_type {
            OrderType::Stop => Err(DiError::NotImplemented),
            OrderType::Limit => match self.account.limit_buy(symbol, order.quantity, order.price) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(format!("{}", e))),
            },
            OrderType::Market => match self.account.market_buy(symbol, order.quantity) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(format!("{}", e))),
            },
            OrderType::StopLimit => match self.account.stop_limit_buy_order(
                symbol,
                order.quantity,
                order.price,
                order.stop_price.unwrap(),
                convert_tif(&order.tif),
            ) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(format!("{}", e))),
            },
        }
    }

    pub fn sell_order(&self, order: &Order) -> Result<Transaction, DiError> {
        let symbol = order.token.to_string();
        match order.order_type {
            OrderType::Stop => Err(DiError::NotImplemented),
            OrderType::Limit => {
                match self.account.limit_sell(symbol, order.quantity, order.price) {
                    Ok(answer) => Ok(answer),
                    Err(e) => Err(DiError::Message(format!("{}", e))),
                }
            }
            OrderType::Market => match self.account.market_sell(symbol, order.quantity) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(format!("{}", e))),
            },
            OrderType::StopLimit => match self.account.stop_limit_sell_order(
                symbol,
                order.quantity,
                order.price,
                order.stop_price.unwrap(),
                convert_tif(&order.tif),
            ) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(format!("{}", e))),
            },
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

impl Trader for BinanceWallet {
    fn create_order(&self, order: &Order) -> Result<u64, DiError> {
        match match order.side {
            Side::Buy => self.buy_order(order),
            Side::Sell => self.sell_order(order),
        } {
            Ok(t) => Ok(t.order_id),
            Err(e) => Err(e),
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
