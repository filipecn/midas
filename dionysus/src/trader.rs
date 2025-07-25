use crate::binance::binance_error;
use crate::finance::{DiError, Order, OrderStatus, OrderType, Side, TimeInForce, Token};
use crate::time::Date;
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
    fn get_all_open_orders(&self) -> Result<Vec<OrderStatus>, DiError>;
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
                Err(e) => Err(DiError::Message(binance_error(e.0))),
            },
            OrderType::Market => match self.account.market_buy(symbol, order.quantity) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(binance_error(e.0))),
            },
            OrderType::StopLimit => match self.account.stop_limit_buy_order(
                symbol,
                order.quantity,
                order.price,
                order.stop_price.unwrap(),
                convert_tif(&order.tif),
            ) {
                Ok(answer) => Ok(answer),
                Err(e) => Err(DiError::Message(binance_error(e.0))),
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

    fn get_all_open_orders(&self) -> Result<Vec<OrderStatus>, DiError> {
        match self.account.get_all_open_orders() {
            Ok(orders) => {
                let mut r: Vec<OrderStatus> = Vec::new();
                for o in orders {
                    let order = Order {
                        index: 0,
                        position_index: Some(0),
                        id: Some(o.order_id as i64),
                        token: Token::from_string(&o.symbol),
                        date: Date::from_timestamp(o.time),
                        side: Side::from_string(&o.side),
                        quantity: o.orig_qty.parse::<f64>().unwrap(),
                        price: o.price,
                        stop_price: Some(o.stop_price),
                        order_type: OrderType::from_string(&o.type_name),
                        tif: TimeInForce::from_string(&o.time_in_force),
                    };
                    let order_status = OrderStatus {
                        order,
                        executed_qty: o.executed_qty.parse::<f64>().unwrap(),
                        status: o.status,
                        update_time: Date::from_timestamp(o.update_time),
                        is_working: o.is_working,
                    };
                    r.push(order_status);
                }
                Ok(r)
            }
            Err(e) => Err(DiError::Message(format!("{:?}", e))),
        }
    }
}
