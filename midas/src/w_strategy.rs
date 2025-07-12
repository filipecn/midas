use std::collections::HashMap;

use dionysus::backtest::Backtest;
use ratatui::style::Color;
use ratatui::text::Line;

use crate::common;
use crate::common::ListWindow;
use crate::midas::Midas;

struct StrategyItem {
    name: String,
    color: Color,
}

#[derive(Default)]
pub struct StrategyWindow {
    list: ListWindow<StrategyItem>,
}

impl StrategyWindow {
    pub fn update(
        &mut self,
        midas: &Midas,
        backtests: &HashMap<usize, Backtest>,
        selected: Option<usize>,
    ) {
        self.list.items.clear();
        for (i, chrysus) in midas.hesperides.iter().enumerate() {
            let mut color = common::NORMAL_FG;
            if let Some(s) = selected {
                if s == i {
                    color = common::PROFIT_COLOR;
                }
            }
            {
                let txt = chrysus.name();
                self.list.items.push(StrategyItem { name: txt, color });
            }
            if let Some(backtest) = backtests.get(&i) {
                let mut txt = format!("{:?}", backtest.period.pretty_string(),);
                if let Some(tick) = midas.ticks.get(&chrysus.token) {
                    txt.push_str(
                        format!(" [{:.2}%]", backtest.compute_profit(tick.price)).as_str(),
                    );
                }
                txt.push_str(
                    format!(
                        " {:.5} / {:.5}",
                        backtest.symbol_balance, backtest.currency_balance
                    )
                    .as_str(),
                );

                self.list.items.push(StrategyItem { name: txt, color });
            }

            for i in &chrysus.strategy.counselors {
                self.list.items.push(StrategyItem {
                    name: i.name(),
                    color,
                });
            }
            self.list.items.push(StrategyItem {
                name: String::from("-------------------"),
                color,
            });
        }
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = common::block("ORACLES");

        self.list.render(area, buf, block, |item| {
            Line::styled(item.name.as_str(), item.color)
        });
    }
}
