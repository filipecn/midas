use std::cmp::Ordering;

use crate::common;
use dionysus::{wallet::Wallet, ERROR};
use ratatui::{
    text::Line,
    widgets::{HighlightSpacing, List, ListItem, ListState, StatefulWidget},
};
use slog::slog_error;

struct BalanceList {
    items: Vec<BalanceItem>,
    state: ListState,
    total: f64,
    change: f64,
}

#[derive(Debug, Default)]
struct BalanceItem {
    asset: String,
    free: f64,
    value: f64,
    change: f64,
}

use dionysus::binance::BinanceMarket;
use dionysus::market::Market;
use dionysus::wallet::BinanceWallet;

pub struct WalletWindow {
    binance_wallet: BinanceWallet,
    binance_market: BinanceMarket,
    balance_list: BalanceList,
    currency: String,
}
impl Default for WalletWindow {
    fn default() -> Self {
        let mut s = Self {
            binance_wallet: BinanceWallet::default(),
            binance_market: BinanceMarket::default(),
            balance_list: BalanceList {
                items: Vec::default(),
                state: ListState::default(),
                total: 0.0,
                change: 0.0,
            },
            currency: String::from("USDT"),
        };
        s.update();
        s
    }
}

fn compute_change_pct(start: f64, end: f64) -> f64 {
    if start.total_cmp(&end) == Ordering::Greater {
        start / end
    } else {
        -end / start
    }
}

impl WalletWindow {
    pub fn update(&mut self) {
        match self.binance_wallet.account.get_account() {
            Ok(answer) => {
                let mut items: Vec<BalanceItem> = answer
                    .balances
                    .iter()
                    .map(|x| BalanceItem {
                        asset: x.asset.clone(),
                        free: x.free.parse::<f64>().unwrap_or(0.0),
                        value: 0.0,
                        change: 0.0,
                    })
                    .filter(|x| x.free > 0.0)
                    .collect();

                // retrieve total balance in currency
                let mut current: f64 = 0.0;
                let mut initial: f64 = 0.0;
                for item in items.iter_mut() {
                    if item.asset != self.currency {
                        match self
                            .binance_market
                            .get_24h_price(item.asset.as_str(), self.currency.as_str())
                        {
                            Ok(stat) => {
                                item.value = stat.last_price * item.free;
                                let open = item.value - item.value * stat.price_change_percent;
                                item.change = compute_change_pct(open, item.value);
                                current += item.value;
                                initial += open;
                            }
                            Err(_) => (),
                        };
                    } else {
                        item.value = item.free;
                        current += item.free;
                        initial += item.free;
                    }
                }

                self.balance_list = BalanceList {
                    items,
                    state: ListState::default(),
                    total: current,
                    change: compute_change_pct(initial, current),
                };
            }
            Err(e) => ERROR!("{:?}", e),
        }
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let title = format!(
            "WALLET (USDT)  {:.2}({:.2}%)",
            self.balance_list.total, self.balance_list.change
        );
        let block = common::block(title.as_str());
        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .balance_list
            .items
            .iter()
            .enumerate()
            .map(|(_, balance_item)| {
                //let color = alternate_colors(i);
                ListItem::from(balance_item) //.bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            .highlight_style(common::SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, &mut self.balance_list.state);
    }
}

impl From<&BalanceItem> for ListItem<'_> {
    fn from(value: &BalanceItem) -> Self {
        let line = Line::styled(
            format!(
                " {:8} {: >12} {:.4} ({:.2}%)",
                value.asset, value.free, value.value, value.change
            ),
            common::NORMAL_FG,
        );
        ListItem::new(line)
    }
}
