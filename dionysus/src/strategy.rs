use crate::{
    counselor::{Advice, Counselor, Signal},
    finance::*,
    historical_data::HistoricalData,
    time::{Date, TimeWindow},
    ERROR,
};

use serde::de::{Deserializer, Visitor};
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use slog::slog_error;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Decision {
    pub advice: Advice,
    pub pct: f64,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub enum Oracle {
    #[default]
    Delphi,
    Dodona,
}

impl Oracle {
    pub fn see(
        &self,
        quote: &Quote,
        history: &[Sample],
        counselors: &[Counselor],
    ) -> Result<Decision, DiError> {
        match self {
            Oracle::Delphi => {
                for counselor in counselors.iter() {
                    if let Ok(advice) = counselor.run(quote, history) {
                        match advice.signal {
                            Signal::Buy => return Ok(Decision { advice, pct: 0.6 }),
                            Signal::Sell => return Ok(Decision { advice, pct: 1.0 }),
                            _ => (),
                        }
                    }
                }
            }
            Oracle::Dodona => (),
        }
        Ok(Decision::default())
    }

    pub fn name(&self) -> String {
        match &self {
            Self::Delphi => format!("Delphi"),
            Self::Dodona => format!("Dodona"),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Strategy {
    pub oracle: Oracle,
    pub counselors: Vec<Counselor>,
    pub duration: TimeWindow,
}

impl Strategy {
    pub fn required_history_size(&self) -> usize {
        let mut ans: usize = 0;
        for c in &self.counselors {
            let r = c.required_samples();
            if r > ans {
                ans = r;
            }
        }
        ans
    }

    pub fn run(&self, quote: &Quote, history: &[Sample]) -> Result<Decision, DiError> {
        self.oracle.see(quote, history, &self.counselors)
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.oracle.name(), self.duration.resolution.name())
    }
}

#[derive(Clone)]
pub struct Chrysus {
    pub active: bool,
    pub token: Token,
    pub strategy: Strategy,
    pub capital: f64,
    locked_capital: f64,
    pub positions: HashMap<usize, Position>,
    pub balance: f64,
    pub book: Book,
    pub orders: HashMap<usize, Order>,
    next_position_index: usize,
    next_order_index: usize,
}

impl Serialize for Chrysus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Chrysus", 4)?;
        state.serialize_field("token", &self.token)?;
        state.serialize_field("strategy", &self.strategy)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Chrysus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ChrysusVisitor;
        impl<'de> Visitor<'de> for ChrysusVisitor {
            type Value = Chrysus;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("chrysus")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut token = None;
                let mut strategy = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "token" => token = Some(map.next_value()?),
                        "strategy" => strategy = Some(map.next_value()?),
                        _ => (), //Err(A::Error::unknown_field()),
                    }
                }

                let mut chrysus = Chrysus::new(&token.unwrap());
                chrysus.strategy = strategy.unwrap();
                Ok(chrysus)
            }
        }

        const FIELDS: &[&str] = &["token", "strategy"];
        deserializer.deserialize_struct("Chrysus", FIELDS, ChrysusVisitor)
    }
}

impl Chrysus {
    pub fn new(token: &Token) -> Self {
        Self {
            active: false,
            token: token.clone(),
            strategy: Strategy::default(),
            capital: 0.0,
            locked_capital: 0.0,
            positions: HashMap::new(),
            balance: 0.0,
            book: Book::default(),
            orders: HashMap::new(),
            next_position_index: 0,
            next_order_index: 0,
        }
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.token.name(), self.strategy.name())
    }

    fn print(&self) {
        let s = format!(
            "{:?} {:?} {:?} {:?}",
            self.token.name(),
            self.capital,
            self.locked_capital,
            self.balance
        );
        ERROR!("{:?}", s);
    }

    fn compute_orders(&mut self, quote: &Quote, decision: &Decision) -> Vec<Order> {
        let mut orders: Vec<Order> = Vec::new();
        match decision.advice.signal {
            Signal::Buy => {
                let available_capital = decision.pct * self.capital;
                let shares = available_capital as f64 / decision.advice.stop_price;
                if shares > 0.0 {
                    self.locked_capital += available_capital;
                    self.capital -= available_capital;
                    let order = Order {
                        index: self.next_order_index,
                        position_index: None,
                        id: None,
                        token: quote.token.clone(),
                        date: Date::now(),
                        quantity: shares,
                        side: Side::Buy,
                        price: decision.advice.stop_price,
                        stop_price: Some(decision.advice.stop_price),
                        order_type: decision.advice.order_type.clone(),
                        tif: decision.advice.tif.clone(),
                    };
                    orders.push(order.clone());
                    self.orders.insert(self.next_order_index, order);
                    self.next_order_index += 1;
                }
            }
            Signal::Sell => {
                for (position_index, position) in &mut self.positions {
                    if position.attached_order == None
                        && decision.advice.stop_price > position.price
                    {
                        let order = Order {
                            index: self.next_order_index,
                            position_index: Some(*position_index),
                            id: None,
                            token: quote.token.clone(),
                            date: Date::now(),
                            quantity: position.quantity,
                            side: Side::Sell,
                            price: decision.advice.stop_price,
                            stop_price: Some(decision.advice.stop_price),
                            order_type: decision.advice.order_type.clone(),
                            tif: decision.advice.tif.clone(),
                        };
                        position.attached_order = Some(self.next_order_index);
                        orders.push(order.clone());
                        self.orders.insert(self.next_order_index, order);
                        self.next_order_index += 1;
                    }
                }
            }
            _ => (),
        }
        orders
    }

    pub fn realize(&mut self, order: &Order) {
        ERROR!("{:?}", order);
        match order.side {
            Side::Sell => {
                if let Some(position_index) = order.position_index {
                    self.positions.remove(&position_index);
                }
                self.balance -= order.quantity;
                self.capital += order.quantity * order.price;
            }
            Side::Buy => {
                self.positions.insert(
                    self.next_position_index,
                    Position {
                        price: order.price,
                        token: order.token.clone(),
                        quantity: order.quantity,
                        date: order.date,
                        attached_order: None,
                    },
                );
                self.balance += order.quantity;
                self.locked_capital -= order.quantity * order.price;
            }
        }
        self.print();
    }

    pub fn decide(&mut self, book: Book, history: &impl HistoricalData) -> Vec<Order> {
        self.book = book;
        if let Some(quote) = self.book.quote() {
            if let Ok(samples) = history.get_last(&self.token, &self.strategy.duration) {
                match self.strategy.run(&quote, samples) {
                    Ok(decision) => return self.compute_orders(&quote, &decision),
                    Err(e) => {
                        ERROR!("{:?}", e);
                    }
                };
            }
        }
        Vec::new()
    }
}
