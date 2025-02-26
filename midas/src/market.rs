use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{
        palette::tailwind::{BLUE, GREEN, SLATE},
        Color, Modifier, Style, Stylize,
    },
    symbols,
    text::Line,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph,
        StatefulWidget, Widget, Wrap,
    },
};

use crate::common::{LOSS_COLOR, NORMAL_BG, NORMAL_HEADER_STYLE, PROFIT_COLOR, SELECTED_STYLE};
use dionysus::binance::BinanceMarket;
use dionysus::market::Market;

struct BalanceList {
    items: Vec<BalanceItem>,
    state: ListState,
}

#[derive(Debug)]
struct BalanceItem {
    symbol: String,
    price: f64,
    change: f64,
    status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Status {
    Profit,
    Loss,
}

pub struct MarketWindow {
    binance_market: BinanceMarket,
    balance_list: BalanceList,
}

impl Default for MarketWindow {
    fn default() -> Self {
        let mut s = Self {
            binance_market: BinanceMarket::default(),
            balance_list: BalanceList {
                items: Vec::default(),
                state: ListState::default(),
            },
        };
        s.update();
        s
    }
}

impl MarketWindow {
    pub fn update(&mut self) {
        match self.binance_market.get_all_24h_price_stats("USDT") {
            Ok(prices) => {
                self.balance_list = BalanceList {
                    items: prices
                        .iter()
                        .map(|x| BalanceItem {
                            symbol: x.symbol.clone(),
                            price: x.last_price,
                            change: x.price_change_percent,
                            status: if x.price_change_percent > 0.0 {
                                Status::Profit
                            } else {
                                Status::Loss
                            },
                        })
                        .collect(),
                    state: ListState::default(),
                };
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = Block::new()
            .title(Line::raw("MARKET (USDT)").centered())
            .borders(Borders::ALL)
            .border_set(symbols::border::EMPTY)
            .border_style(NORMAL_HEADER_STYLE)
            .bg(NORMAL_BG);

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
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.balance_list.state);
    }
}

impl From<&BalanceItem> for ListItem<'_> {
    fn from(value: &BalanceItem) -> Self {
        let txt = format!(
            " {:10} {: >12} ({:.2}%)",
            value.symbol, value.price, value.change
        );
        let line = match value.status {
            Status::Profit => Line::styled(txt, PROFIT_COLOR),
            Status::Loss => Line::styled(txt, LOSS_COLOR),
        };
        ListItem::new(line)
    }
}
