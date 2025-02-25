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
    DefaultTerminal,
};

const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

struct BalanceList {
    items: Vec<BalanceItem>,
    state: ListState,
}

#[derive(Debug)]
struct BalanceItem {
    asset: String,
    free: f64,
    status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Status {
    Profit,
    Loss,
}

use dionysus::wallet::BinanceWallet;

pub struct WalletWindow {
    binance_wallet: BinanceWallet,
    balance_list: BalanceList,
}
impl Default for WalletWindow {
    fn default() -> Self {
        let mut s = Self {
            binance_wallet: BinanceWallet::default(),
            balance_list: BalanceList {
                items: Vec::default(),
                state: ListState::default(),
            },
        };
        s.update();
        s
    }
}

impl WalletWindow {
    pub fn update(&mut self) {
        match self.binance_wallet.account.get_account() {
            Ok(answer) => {
                self.balance_list = BalanceList {
                    items: answer
                        .balances
                        .iter()
                        .map(|x| BalanceItem {
                            asset: x.asset.clone(),
                            free: x.free.parse::<f64>().unwrap_or(0.0),
                            status: Status::Profit,
                        })
                        .filter(|x| x.free > 0.0)
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
            .title(Line::raw("TODO List").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .balance_list
            .items
            .iter()
            .enumerate()
            .map(|(i, balance_item)| {
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
        let line = match value.status {
            Status::Profit => Line::styled(format!(" ☐ {}", value.free), TEXT_FG_COLOR),
            Status::Loss => Line::styled(format!(" ✓ {}", value.free), COMPLETED_TEXT_FG_COLOR),
        };
        ListItem::new(line)
    }
}
