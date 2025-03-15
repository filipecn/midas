use ratatui::text::Line;

use crate::common;
use crate::common::ListWindow;
use dionysus::finance::{Quote, Sample};
use dionysus::oracles::{Advice, Oracle, Signal};
use dionysus::ERROR;
use slog::slog_error;

#[derive(Default)]
pub struct StrategyWindow {
    oracles: Vec<(Oracle, Advice)>,
}

impl StrategyWindow {
    pub fn add_oracle(&mut self, oracle: &Oracle) {
        self.oracles.push((oracle.clone(), Advice::default()));
    }

    pub fn run(&mut self, quote: &Quote, history: &[Sample]) {
        for (oracle, advice) in &mut self.oracles {
            match oracle.run(quote, history) {
                Ok(a) => *advice = a,
                Err(e) => ERROR!("{:?}", e),
            }
        }
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = common::block("ORACLES");

        let mut list: ListWindow<Line> = ListWindow::default();
        for (oracle, advice) in &self.oracles {
            let mut color = common::NORMAL_FG;
            if advice.signal == Signal::Buy {
                color = common::LOSS_COLOR;
            } else if advice.signal == Signal::Sell {
                color = common::PROFIT_COLOR;
            }

            list.items
                .push(Line::styled(format!("{:?}", oracle), color));
            list.items
                .push(Line::styled(format!("  {:?}", advice), color));
        }

        list.render(area, buf, block, |s| s.clone());
    }
}
