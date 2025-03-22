use ratatui::{
    layout::{Constraint, Layout},
    text::Line,
    widgets::{Block, Borders},
};
use std::cmp::Ordering;

use crate::common::{self};
use common::ListWindow;
use dionysus::finance::{Book, BookLine};

pub struct OrderBookWindow {
    pub book: Book,
    bids_window: ListWindow<BookLine>,
    asks_window: ListWindow<BookLine>,
}

impl Default for OrderBookWindow {
    fn default() -> Self {
        Self {
            book: Book::default(),
            bids_window: ListWindow::default(),
            asks_window: ListWindow::default(),
        }
    }
}

impl OrderBookWindow {
    pub fn update_with(&mut self, new_book: Book) {
        self.book = new_book;
        self.book.bids.sort_by(|a, b| {
            a.price
                .partial_cmp(&b.price)
                .map(Ordering::reverse)
                .unwrap()
        });
        self.book
            .asks
            .sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
        self.asks_window.items = self.book.asks.clone();
        self.bids_window.items = self.book.bids.clone();
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let [bids_area, asks_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);
        let bids_block = Block::default().borders(Borders::RIGHT).title("SELL");
        self.bids_window.render(bids_area, buf, bids_block, |item| {
            Line::styled(
                format!("{} {}", item.price, item.quantity),
                common::LOSS_COLOR,
            )
        });
        let asks_block = Block::default().borders(Borders::LEFT).title("BUY");
        self.asks_window.render(asks_area, buf, asks_block, |item| {
            Line::styled(
                format!("{} {}", item.price, item.quantity),
                common::PROFIT_COLOR,
            )
        });
    }
}
