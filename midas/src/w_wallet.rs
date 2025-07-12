use std::collections::HashMap;

use crate::common;
use crate::common::ListWindow;
use dionysus::finance::{MarketTick, Token};
use dionysus::utils::compute_change_pct;
use ratatui::text::Line;

struct BalanceItem {
    asset: String,
    free: f64,
    value: f64,
    change: f64,
}

#[derive(Default)]
pub struct WalletWindow {
    list_window: ListWindow<BalanceItem>,
    total: f64,
    total_change: f64,
}

impl WalletWindow {
    pub fn update(&mut self, balance: HashMap<Token, f64>, ticks: &HashMap<Token, MarketTick>) {
        let mut wallet_ticks: HashMap<Token, MarketTick> = HashMap::new();
        for (token, tick) in ticks
            .iter()
            .filter(|(token, _)| balance.contains_key(&token.symbol()))
        {
            wallet_ticks.insert(token.clone(), tick.clone());
        }
        self.list_window.items = balance
            .iter()
            .map(|(token, value)| BalanceItem {
                asset: token.to_string(),
                free: value.clone(),
                value: 0.0,
                change: 0.0,
            })
            .collect();
        for item in self.list_window.items.iter_mut() {
            if let Some(mt) = wallet_ticks.get(&Token::pair(&item.asset, "USDT")) {
                item.value = item.free * mt.price;
                item.change = mt.change_pct;
            }
            if item.asset == "USDT" {
                item.value = item.free;
                item.change = 0.0;
            }
        }
        self.list_window.items.sort_by(|a, b| a.asset.cmp(&b.asset));
        let mut current: f64 = 0.0;
        let mut initial: f64 = 0.0;
        for item in self.list_window.items.iter_mut() {
            current += item.value;
            initial += item.value - item.value * item.change;
        }
        self.total = current;
        self.total_change = compute_change_pct(initial, current);
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let title = format!(
            "WALLET (USDT)  {:.2}({:.2}%)",
            self.total, self.total_change
        );
        let block = common::block(title.as_str());

        self.list_window.render(area, buf, block, |value| {
            Line::styled(
                format!(
                    " {:8} {: >12} {:.4} ({:.2}%)",
                    value.asset, value.free, value.value, value.change
                ),
                common::NORMAL_FG,
            )
        });
    }
}
