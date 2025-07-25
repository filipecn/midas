use ratatui::{
    layout::Alignment,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph, Widget, Wrap},
};

#[derive(Default)]
pub struct HelpWindow {}

impl HelpWindow {
    pub fn render(&mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let text = vec![
            Line::from("Esc    : Close float windows."),
            Line::from("?      : Open/close help float window."),
            Line::from("/      : Open/close info float window."),
            Line::from("l      : Open/close log float window."),
            Line::from("o      : Open current oracle float window."),
            Line::from("ctrl+t : Iterate pairs."),
            Line::from("ctrl+o : Iterate pair oracles."),
            Line::from("a      : Enter command."),
            Line::from(""),
            Line::from("COMMANDS".blue()),
            Line::from(""),
            Line::from("load <symbol> <currency = usdt>"),
            Line::from("graph <indicator> <indicator params>"),
            Line::from("oracle <oracle>"),
            Line::from("res <resolution>"),
            Line::from("hist <size>"),
            Line::from("backtest"),
            Line::from("save"),
        ];
        Paragraph::new(text)
            .block(Block::bordered().title("Help"))
            .style(Style::new().white().on_black())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }
}
