// fundamentus_view.rs

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::fundamentus::StockDetails;

pub struct FundamentusWidget<'a> {
    stock: &'a StockDetails,
}

impl<'a> FundamentusWidget<'a> {
    pub fn new(stock: &'a StockDetails) -> Self {
        Self { stock }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Divide vertical space: Header -> Info/Market -> Indicators -> Financials
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title / Quick Stats Header
                Constraint::Length(7), // General Info & Market Capitalization
                Constraint::Length(8), // Fundamental Indicators Grid
                Constraint::Min(6),    // Balance Sheet & Income Statement
            ])
            .split(area);

        self.render_header(frame, chunks[0]);
        self.render_general_info(frame, chunks[1]);
        self.render_indicators(frame, chunks[2]);
        self.render_financials(frame, chunks[3]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title_line = Line::from(vec![
            Span::styled(
                &self.stock.ticker,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - "),
            Span::styled(
                &self.stock.nome,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | Cotação: "),
            Span::styled(
                format!("R$ {}", self.stock.cotacao),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | Data: "),
            Span::styled(&self.stock.data_cot, Style::default().fg(Color::DarkGray)),
        ]);

        let header = Paragraph::new(title_line).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" pyFundamentus CLI - Summary ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        );

        frame.render_widget(header, area);
    }

    fn render_general_info(&self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Column 1: Company Profile
        let profile_rows = vec![
            Row::new(vec![
                Cell::from("Tipo:").fg(Color::Cyan),
                Cell::from(self.stock.tipo.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Setor:").fg(Color::Cyan),
                Cell::from(self.stock.setor.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Subsetor:").fg(Color::Cyan),
                Cell::from(self.stock.subsetor.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Vol $ Médio (2m):").fg(Color::Cyan),
                Cell::from(self.stock.vol_medio.as_str()),
            ]),
        ];

        let profile_table = Table::new(
            profile_rows,
            [Constraint::Percentage(40), Constraint::Percentage(60)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Informações Gerais "),
        );

        // Column 2: Market Valuation
        let market_rows = vec![
            Row::new(vec![
                Cell::from("Valor de Mercado:").fg(Color::Cyan),
                Cell::from(self.stock.val_mercado.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Valor da Firma:").fg(Color::Cyan),
                Cell::from(self.stock.val_firma.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Nro. Ações:").fg(Color::Cyan),
                Cell::from(self.stock.nro_acoes.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Min / Max (52s):").fg(Color::Cyan),
                Cell::from(format!(
                    "{} / {}",
                    self.stock.min_52sem, self.stock.max_52sem
                )),
            ]),
        ];

        let market_table = Table::new(
            market_rows,
            [Constraint::Percentage(45), Constraint::Percentage(55)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Valor de Mercado "),
        );

        frame.render_widget(profile_table, cols[0]);
        frame.render_widget(market_table, cols[1]);
    }

    fn render_indicators(&self, frame: &mut Frame, area: Rect) {
        let rows = vec![
            Row::new(vec![
                Cell::from("P/L:").fg(Color::Yellow),
                Cell::from(self.stock.p_l.as_str()),
                Cell::from("P/VP:").fg(Color::Yellow),
                Cell::from(self.stock.p_vp.as_str()),
                Cell::from("PSR:").fg(Color::Yellow),
                Cell::from(self.stock.psr.as_str()),
                Cell::from("Div. Yield:").fg(Color::Yellow),
                Cell::from(self.stock.div_yield.as_str()).fg(Color::Green),
            ]),
            Row::new(vec![
                Cell::from("LPA:").fg(Color::Yellow),
                Cell::from(self.stock.lpa.as_str()),
                Cell::from("VPA:").fg(Color::Yellow),
                Cell::from(self.stock.vpa.as_str()),
                Cell::from("EV/EBITDA:").fg(Color::Yellow),
                Cell::from(self.stock.ev_ebitda.as_str()),
                Cell::from("ROIC:").fg(Color::Yellow),
                Cell::from(self.stock.roic.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Marg. Bruta:").fg(Color::Yellow),
                Cell::from(self.stock.marg_bruta.as_str()),
                Cell::from("Marg. EBIT:").fg(Color::Yellow),
                Cell::from(self.stock.marg_ebit.as_str()),
                Cell::from("Marg. Líquida:").fg(Color::Yellow),
                Cell::from(self.stock.marg_liquida.as_str()),
                Cell::from("ROE:").fg(Color::Yellow),
                Cell::from(self.stock.roe.as_str()).fg(Color::Green),
            ]),
            Row::new(vec![
                Cell::from("Cres. Rec (5a):").fg(Color::Yellow),
                Cell::from(self.stock.cres_rec_5a.as_str()).fg(Color::Green),
            ]),
        ];

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(12),
                Constraint::Percentage(13),
                Constraint::Percentage(12),
                Constraint::Percentage(13),
                Constraint::Percentage(12),
                Constraint::Percentage(13),
                Constraint::Percentage(12),
                Constraint::Percentage(13),
            ],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Indicadores Fundamentalistas "),
        );

        frame.render_widget(table, area);
    }

    fn render_financials(&self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Balance Sheet (Balanço Patrimonial)
        let balance_rows = vec![
            Row::new(vec![
                Cell::from("Ativo Total:").fg(Color::Magenta),
                Cell::from(self.stock.ativo.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Disponibilidades:").fg(Color::Magenta),
                Cell::from(self.stock.disponibilidades.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Dívida Bruta:").fg(Color::Magenta),
                Cell::from(self.stock.div_bruta.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Dívida Líquida:").fg(Color::Magenta),
                Cell::from(self.stock.div_liquida.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Patrimônio Líquido:").fg(Color::Magenta),
                Cell::from(self.stock.patrim_liquido.as_str()),
            ]),
        ];

        let balance_table = Table::new(
            balance_rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Balanço Patrimonial "),
        );

        // Income Statement (DRE - Últimos 12 Meses)
        let income_rows = vec![
            Row::new(vec![
                Cell::from("Receita Líquida (12m):").fg(Color::Magenta),
                Cell::from(self.stock.rec_liq_12m.as_str()),
            ]),
            Row::new(vec![
                Cell::from("Lucro Líquido (12m):").fg(Color::Magenta),
                Cell::from(self.stock.lucro_liq_12m.as_str()),
            ]),
        ];

        let income_table = Table::new(
            income_rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Demonstração de Resultados (12m) "),
        );

        frame.render_widget(balance_table, cols[0]);
        frame.render_widget(income_table, cols[1]);
    }
}
