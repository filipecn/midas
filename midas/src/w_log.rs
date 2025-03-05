use ratatui::prelude::Color;
use ratatui::prelude::Style;
use ratatui::widgets::{Block, Widget};
use slog::{self, o, Drain};
use slog_scope;
use slog_scope::GlobalLoggerGuard;
use tui_logger;
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerWidget};

pub fn init() -> GlobalLoggerGuard {
    tui_logger::init_logger(tui_logger::LevelFilter::Trace).unwrap();
    let drain = tui_logger::slog_drain().fuse();
    let log = slog::Logger::root(drain, o!());
    slog_scope::set_global_logger(log)
}

#[derive(Default)]
pub struct LogWindow {}

impl Widget for &LogWindow {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        TuiLoggerWidget::default()
            .block(Block::bordered().title("LOG"))
            //.opt_formatter(formatter)
            .output_separator('|')
            .output_timestamp(Some("%F %H:%M:%S%.3f".to_string()))
            .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(false)
            .output_file(false)
            .output_line(false)
            .style(Style::default().fg(Color::White))
            .render(area, buf);
    }
}
