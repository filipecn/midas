use crate::common;
use crate::common::ListWindow;
use crate::midas::Midas;
use ratatui::{
    prelude::{Buffer, Rect},
    style::Color,
    text::Line,
};

struct BacktestItem {
    name: String,
    color: Color,
}

#[derive(Default)]
pub struct BacktestWindow {
    list: ListWindow<BacktestItem>,
}

impl BacktestWindow {
    pub fn open(&mut self, midas: &Midas) {}

    pub fn render(&mut self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = common::block("BACKTESTS");

        self.list.render(area, buf, block, |item| {
            Line::styled(item.name.as_str(), item.color)
        });
    }
}
