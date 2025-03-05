use ratatui::text::Line;
use std::cmp::Ordering;

use crate::common;
use crate::common::ListWindow;
use dionysus::binance::MarketTick;

#[derive(Default)]
pub struct MarketWindow {
    list_window: ListWindow<MarketTick>,
}

impl MarketWindow {
    pub fn update_with(&mut self, ticks: Vec<MarketTick>) {
        self.list_window.items = ticks;
        self.list_window.items.sort_by(|a, b| {
            a.change_pct
                .partial_cmp(&b.change_pct)
                .map(Ordering::reverse)
                .unwrap()
        });
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = common::block("MARKET").title(Line::from("USDT").left_aligned());

        self.list_window.render(area, buf, block, |market_tick| {
            Line::styled(
                format!(
                    " {:10} {: >12} ({:.2}%)",
                    market_tick.token.get_symbol(),
                    market_tick.price,
                    market_tick.change_pct
                ),
                if market_tick.change_pct > 0.0 {
                    common::PROFIT_COLOR
                } else {
                    common::LOSS_COLOR
                },
            )
        });
    }
}
