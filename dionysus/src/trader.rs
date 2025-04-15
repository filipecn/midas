use crate::finance::{DiError, Order, OrderType, Side, TimeInForce};
use crate::wallet::BinanceWallet;
use binance::model::Transaction;

pub trait Trader {
    fn buy_order(&self, order: &Order) -> Result<Transaction, DiError>;
    fn sell_order(&self, order: &Order) -> Result<Transaction, DiError>;
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

fn convert_tif(tif: &TimeInForce) -> binance::account::TimeInForce {
    match tif {
        TimeInForce::FOK => binance::account::TimeInForce::FOK,
        TimeInForce::IOC => binance::account::TimeInForce::IOC,
        TimeInForce::GTC => binance::account::TimeInForce::GTC,
    }
}

impl Trader for BinanceWallet {
    fn buy_order(&self, order: &Order) -> Result<Transaction, DiError> {
        let symbol = order.token.to_string();
        match order.order_type {
            OrderType::StopMarket => Err(DiError::NotImplemented),
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

    fn sell_order(&self, order: &Order) -> Result<Transaction, DiError> {
        let symbol = order.token.to_string();
        match order.order_type {
            OrderType::StopMarket => Err(DiError::NotImplemented),
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
