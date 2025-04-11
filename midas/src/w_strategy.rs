use std::collections::HashMap;

use ratatui::text::Line;

use crate::common;
use crate::common::ListWindow;
use dionysus::finance::Token;
use dionysus::strategy::Chrysus;

struct StrategyItem {
    name: String,
}

#[derive(Default)]
pub struct StrategyWindow {
    list: ListWindow<StrategyItem>,
}

impl StrategyWindow {
    pub fn update(&mut self, targets: &HashMap<Token, Chrysus>) {
        self.list.items = targets
            .iter()
            .map(|(_, chrysus)| StrategyItem {
                name: chrysus.name(),
            })
            .collect();
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = common::block("ORACLES");

        self.list.render(area, buf, block, |item| {
            Line::styled(item.name.as_str(), common::LOSS_COLOR)
        });
    }
}
