use ratatui::{
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    investments::investment_transactions_list::InvestmentTransactionsList,
    investments::{Investment, InvestmentTransaction},
    tui::TUI,
    widgets::InteractableWidget,
};

pub struct InvestmentTransactionsView {
    view: InvestmentTransactionsList,
    investments: Vec<Investment>,
}

impl InvestmentTransactionsView {
    pub fn new(
        investments: Vec<Investment>,
        investment_transactions: Vec<InvestmentTransaction>,
    ) -> Self {
        Self {
            view: InvestmentTransactionsList::new(investment_transactions),
            investments,
        }
    }

    fn invested_amount_in_cents(&self) -> i64 {
        self.view
            .rows
            .iter()
            .fold(0, |total, transaction| match transaction.transaction_type {
                crate::investments::InvestmentTransactionType::Buy => {
                    total + transaction.amount_in_cents
                }
                crate::investments::InvestmentTransactionType::Sell => {
                    total - transaction.amount_in_cents
                }
                _ => total,
            })
    }

    fn render_details(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Investment transaction details");

        let lines = self
            .view
            .selected_transaction()
            .map(|transaction| {
                let fields = [
                    (
                        "Database ID",
                        transaction
                            .db_id
                            .map_or_else(|| "-".to_string(), |id| id.to_string()),
                    ),
                    ("Transaction ID", transaction.identifier.clone()),
                    (
                        "Date",
                        transaction.date.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    ),
                    ("Investment ID", transaction.investment_id.clone()),
                    ("Type", transaction.transaction_type.to_string()),
                    (
                        "Quantity",
                        transaction
                            .quantity
                            .map(|quantity| format!("{quantity:.4}"))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Price per unit",
                        transaction
                            .price_per_unit_in_cents
                            .map(|amount| TUI::format_amount_for_display(amount, 0))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Value",
                        transaction
                            .value_in_cents
                            .map(|amount| TUI::format_amount_for_display(amount, 0))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Amount",
                        TUI::format_amount_for_display(transaction.amount_in_cents, 0),
                    ),
                ];
                fields
                    .into_iter()
                    .map(|(label, value)| {
                        Line::from(vec![
                            Span::styled(
                                format!("{label}: "),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(value, Style::default().fg(palette.budget)),
                        ])
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![Line::from("Select a transaction from the list.")]);

        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_investment_details(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let palette = TUI::color_palette();
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Investment details");

        let lines = self
            .view
            .selected_transaction()
            .and_then(|transaction| {
                self.investments
                    .iter()
                    .find(|investment| investment.identifier == transaction.investment_id)
            })
            .map(|investment| {
                let fields = [
                    ("Name", investment.name.clone()),
                    ("Identifier", investment.identifier.clone()),
                    ("Type", investment.investment_type.clone()),
                    ("Subtype", investment.subtype.clone()),
                    ("Status", investment.status.clone()),
                    (
                        "Amount",
                        TUI::format_amount_for_display(investment.amount_in_cents, 0),
                    ),
                    (
                        "Original amount",
                        TUI::format_amount_for_display(investment.amount_original_in_cents, 0),
                    ),
                    (
                        "Due date",
                        investment
                            .due_date
                            .map(|date| date.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Rate",
                        investment
                            .rate
                            .map(|rate| format!("{rate:.2}%"))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                    (
                        "Fixed annual rate",
                        investment
                            .fixed_annual_rate
                            .map(|rate| format!("{rate:.2}%"))
                            .unwrap_or_else(|| "-".to_string()),
                    ),
                ];
                fields
                    .into_iter()
                    .map(|(label, value)| {
                        Line::from(vec![
                            Span::styled(
                                format!("{label}: "),
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(value, Style::default().fg(palette.budget)),
                        ])
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![Line::from("No investment details available.")]);

        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
            area,
        );
    }
}

impl InteractableWidget for InvestmentTransactionsView {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let [summary_area, panels_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let [list_area, panels_area] =
            Layout::horizontal([Constraint::Percentage(65), Constraint::Min(1)]).areas(panels_area);
        let [transaction_details_area, investment_details_area] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Min(1)]).areas(panels_area);

        let palette = TUI::color_palette();
        let summary = Line::from(vec![
            Span::styled(
                "Current invested amount: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                TUI::format_amount_for_display(self.invested_amount_in_cents(), 0),
                Style::default().fg(palette.green),
            ),
        ]);
        frame.render_widget(Paragraph::new(summary), summary_area);

        self.view.render(list_area, frame);
        self.render_details(transaction_details_area, frame);
        self.render_investment_details(investment_details_area, frame);
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> crate::widgets::ActionResult {
        self.view.handle_key(key)
    }
}
