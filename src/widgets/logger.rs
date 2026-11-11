use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    widgets::{Clear, Widget},
};
use tui_logger::{LevelFilter, TuiLoggerWidget, TuiWidgetEvent, TuiWidgetState};

pub struct LoggerWidget {
    pub state: TuiWidgetState,
}

impl LoggerWidget {
    pub fn new() -> Self {
        Self {
            state: TuiWidgetState::new().set_default_display_level(LevelFilter::Trace),
        }
    }

    pub fn render(&self, area: Rect, frame: &mut ratatui::Frame) {
        frame.render_widget(Clear, area);
        TuiLoggerWidget::default()
            .block(ratatui::widgets::Block::bordered().title("Logs (l or Esc to close)"))
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_target(true)
            .state(&self.state)
            .render(area, frame.buffer_mut());
    }

    pub fn handle_key(&self, key: &KeyEvent) -> bool {
        let event = match key.code {
            KeyCode::Up => Some(TuiWidgetEvent::PrevPageKey),
            KeyCode::Down => Some(TuiWidgetEvent::NextPageKey),
            KeyCode::Left => Some(TuiWidgetEvent::LeftKey),
            KeyCode::Right => Some(TuiWidgetEvent::RightKey),
            KeyCode::PageUp => Some(TuiWidgetEvent::PrevPageKey),
            KeyCode::PageDown => Some(TuiWidgetEvent::NextPageKey),
            KeyCode::Char(' ') => Some(TuiWidgetEvent::SpaceKey),
            KeyCode::Char('f') => Some(TuiWidgetEvent::FocusKey),
            KeyCode::Char('h') => Some(TuiWidgetEvent::HideKey),
            KeyCode::Char('+') => Some(TuiWidgetEvent::PlusKey),
            KeyCode::Char('-') => Some(TuiWidgetEvent::MinusKey),
            KeyCode::Esc => Some(TuiWidgetEvent::EscapeKey),
            _ => None,
        };
        if let Some(event) = event {
            self.state.transition(event);
            true
        } else {
            false
        }
    }
}
