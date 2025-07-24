use dionysus::binance::BinanceExchange;
use dionysus::finance::Token;
use ratatui::text::Line;

use crate::common;
use crate::common::ListWindow;

#[derive(Default)]
pub struct InfoWindow {
    list_window: ListWindow<String>,
}

impl InfoWindow {
    pub fn update(&mut self, exchange: &mut BinanceExchange, token: &Token) {
        self.list_window.items.clear();
        self.list_window
            .items
            .push(format!("Server Time: {:?}", exchange.server_time));

        self.list_window
            .items
            .push(format!("Current Token: {:?}", token.to_string()));

        let symbol = exchange.get(token);

        self.list_window
            .items
            .push(format!("Status: {}", symbol.status));

        // order types
        self.list_window.items.push(format!("Order Types:"));
        for o in &symbol.order_types {
            self.list_window.items.push(format!("    {:?}", o));
        }

        self.list_window.items.push(format!(
            "Spot Trading Allowed: {:?}",
            symbol.is_spot_trading_allowed
        ));

        self.list_window.items.push(format!("Lot Size:"));
        self.list_window
            .items
            .push(format!("    Min Quantity: {}", symbol.lot_min_qty));
    }

    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = common::block("INFO");

        self.list_window.render(area, buf, block, |info| {
            Line::styled(format!(" {:?}", info), common::PROFIT_COLOR)
        });
    }
}
