use crate::cache::Cache;
use crate::finance::{DiError, Quote, Sample, Token};
use crate::time::{Date, TimeUnit};
use crate::{ERROR, INFO};
use binance;
use binance::websockets::*;
use slog::{self, o, slog_error, slog_info, Drain};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::sync::{
    atomic::AtomicBool,
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use threadpool::ThreadPool;

const MAX_CONCURRENT_THREADS: usize = 40;

pub struct BinanceStream {
    pub stream: binance::userstream::UserStream,
    keep_running: AtomicBool,
}

#[derive(Clone)]
pub struct BookLine {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Default)]
pub struct Book {
    pub token: Token,
    pub bids: Vec<BookLine>,
    pub asks: Vec<BookLine>,
}

impl Book {
    pub fn quote(&self) -> Quote {
        Quote {
            bid: if let Some(l) = self
                .bids
                .iter()
                .max_by(|a, b| a.price.partial_cmp(&b.price).unwrap())
            {
                l.price
            } else {
                0.0
            },
            ask: if let Some(l) = self
                .bids
                .iter()
                .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap())
            {
                l.price
            } else {
                0.0
            },
            token: self.token.clone(),
            biddate: Date::now(),
            askdate: Date::now(),
        }
    }
}

pub struct MarketTick {
    pub token: Token,
    pub price: f64,
    pub change_pct: f64,
}

pub enum MarketEvent {
    KLine((Token, Sample)),
    Ticks(Vec<MarketTick>),
    OrderBook(Book),
}

pub struct BinanceMarket {
    pub market: binance::market::Market,
    pub cache: Cache,
    pool: ThreadPool,
    event_channel: (Sender<MarketEvent>, Receiver<MarketEvent>),
    thread_control: Arc<Mutex<HashMap<String, bool>>>,
}

impl Default for BinanceStream {
    fn default() -> Self {
        let keys: Vec<String> = read_to_string("/home/filipecn/dev/midas/keys")
            .unwrap() // panic on possible file-reading errors
            .lines() // split the string into an iterator of string slices
            .map(String::from) // make each slice into a string
            .collect();
        let api_key = Some(keys[1].clone().into());
        Self {
            stream: binance::api::Binance::new(api_key, None),
            keep_running: AtomicBool::new(true),
        }
    }
}

impl BinanceStream {
    pub fn start(&mut self) -> Result<(), DiError> {
        if let Ok(answer) = self.stream.start() {
            let listen_key = answer.listen_key;

            let mut web_socket = WebSockets::new(|event: WebsocketEvent| {
                match event {
                    WebsocketEvent::BalanceUpdate(account_update) => {
                        for balance in &account_update.balance {
                            println!(
                                "Asset: {}, free: {}, locked: {}",
                                balance.asset, balance.wallet_balance, balance.balance_change
                            );
                        }
                    }
                    WebsocketEvent::AccountUpdate(account_update) => {
                        for balance in &account_update.data.balances {
                            println!(
                                "Asset: {}, free: {}, locked: {}",
                                balance.asset, balance.wallet_balance, balance.balance_change
                            );
                        }
                    }
                    _ => (),
                };
                Ok(())
            });
            web_socket.connect(&listen_key).unwrap(); // check error
            if let Err(e) = web_socket.event_loop(&self.keep_running) {
                match e {
                    err => {
                        println!("Error: {:?}", err);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for BinanceMarket {
    fn default() -> Self {
        Self {
            market: binance::api::Binance::new(None, None),
            cache: Cache::default(),
            pool: ThreadPool::new(MAX_CONCURRENT_THREADS),
            event_channel: mpsc::channel(),
            thread_control: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl BinanceMarket {
    pub fn get_events(&self) -> Vec<MarketEvent> {
        let mut events: Vec<MarketEvent> = Vec::new();
        for event in self.event_channel.1.try_iter() {
            events.push(event);
        }
        events
    }

    fn register_service(&mut self, key: &str) -> bool {
        let mut control = self.thread_control.lock().unwrap();

        if control.contains_key(key) {
            return false;
        }
        // TODO check max number of threads
        control.insert(String::from(key), true);
        true
    }

    pub fn order_book_service(&mut self, token: &Token) {
        let key = format!("{}@depth@100ms", token.to_string().to_lowercase());

        if self.register_service(key.as_str()) {
            let _control = Arc::clone(&self.thread_control);
            let tx = self.event_channel.0.clone();
            let tk = token.clone();
            self.pool.execute(move || {
                let keep_running = AtomicBool::new(true);
                let mut web_socket = WebSockets::new(|event: WebsocketEvent| {
                    if let WebsocketEvent::DepthOrderBook(depth_order_book) = event {
                        tx.send(MarketEvent::OrderBook(Book {
                            token: tk.clone(),
                            bids: depth_order_book
                                .bids
                                .iter()
                                .map(|b| BookLine {
                                    price: b.price,
                                    quantity: b.qty,
                                })
                                .collect(),
                            asks: depth_order_book
                                .asks
                                .iter()
                                .map(|b| BookLine {
                                    price: b.price,
                                    quantity: b.qty,
                                })
                                .collect(),
                        }))
                        .unwrap();
                    }

                    Ok(())
                });

                INFO!("order-book service: {:?}", key);
                web_socket.connect(&key).unwrap(); // check error
                if let Err(e) = web_socket.event_loop(&keep_running) {
                    match e {
                        err => {
                            ERROR!("order-book service error {:?}: {:?}", key, err);
                        }
                    }
                }
                web_socket.disconnect().unwrap();
            });
        }
    }

    pub fn day_ticker_all_service(&mut self, currency: &str) {
        let key = format!("!ticker@arr");
        if self.register_service(key.as_str()) {
            let _control = Arc::clone(&self.thread_control);
            let curr = String::from(currency);
            let tx = self.event_channel.0.clone();
            self.pool.execute(move || {
                let keep_running = AtomicBool::new(true); // Used to control the event loop
                let agg_trade = format!("!ticker@arr"); // All Symbols
                let mut web_socket = WebSockets::new(|event: WebsocketEvent| {
                    match event {
                        // 24hr rolling window ticker statistics for all symbols that changed in an array.
                        WebsocketEvent::DayTickerAll(ticker_events) => {
                            let mut ticks: Vec<MarketTick> = Vec::new();
                            for tick_event in ticker_events {
                                if tick_event.symbol.contains(curr.as_str()) {
                                    ticks.push(MarketTick {
                                        token: Token::pair(
                                            &tick_event.symbol
                                                [..tick_event.symbol.len() - curr.len()],
                                            curr.as_str(),
                                        ),
                                        price: tick_event.current_close.parse::<f64>().unwrap(),
                                        change_pct: tick_event
                                            .price_change_percent
                                            .parse::<f64>()
                                            .unwrap(),
                                    });
                                }
                            }
                            if !ticks.is_empty() {
                                tx.send(MarketEvent::Ticks(ticks)).unwrap();
                            }
                        }
                        _ => (),
                    };

                    Ok(())
                });

                INFO!("all-ticker service: {:?}", agg_trade);
                web_socket.connect(&agg_trade).unwrap(); // check error
                if let Err(e) = web_socket.event_loop(&keep_running) {
                    match e {
                        err => {
                            ERROR!("all-ticker service error {:?}: {:?}", agg_trade, err);
                        }
                    }
                }
            });
        }
    }

    pub fn kline_service(&mut self, token: &Token, resolution: &TimeUnit) {
        let kline_key = format!(
            "{}@kline_{}",
            token.to_string().to_lowercase(),
            resolution.name()
        );
        if self.register_service(kline_key.as_str()) {
            let _control = Arc::clone(&self.thread_control);
            let tx = self.event_channel.0.clone();
            let res = resolution.clone();
            let tk = token.clone();

            self.pool.execute(move || {
                let keep_running = AtomicBool::new(true);
                let mut web_socket = WebSockets::new(|event: WebsocketEvent| {
                    match event {
                        WebsocketEvent::Kline(kline_event) => {
                            tx.send(MarketEvent::KLine((
                                tk.clone(),
                                Sample {
                                    resolution: res.clone(),
                                    timestamp: kline_event.kline.open_time as u64,
                                    open: kline_event.kline.open.parse::<f64>().unwrap(),
                                    high: kline_event.kline.high.parse::<f64>().unwrap(),
                                    low: kline_event.kline.low.parse::<f64>().unwrap(),
                                    close: kline_event.kline.close.parse::<f64>().unwrap(),
                                    volume: kline_event.kline.volume.parse::<f64>().unwrap() as u64,
                                },
                            )))
                            .unwrap();
                        }
                        _ => (),
                    };
                    Ok(())
                });

                INFO!("kline service: {:?}", kline_key);

                web_socket.connect(&kline_key).unwrap(); // check error
                if let Err(e) = web_socket.event_loop(&keep_running) {
                    match e {
                        err => {
                            ERROR!("kline service error {:?}: {:?}", kline_key, err);
                        }
                    }
                }
                web_socket.disconnect().unwrap();
            });
        }
    }
}
