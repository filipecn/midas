use crossterm::event::KeyCode;
use ratatui_interact::{
    components::{Input, InputState},
    events::{get_char, is_backspace, is_delete, is_end, is_home},
};

use crate::widgets::InteractableWidget;

pub struct InputBox {
    pub label: String,
    pub state: InputState,
}

impl InputBox {
    pub fn new(label: &str) -> Self {
        Self {
            label: String::from(label),
            state: InputState::new(""),
        }
    }
}

impl InteractableWidget for InputBox {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let input = Input::new(&self.state).label(self.label.as_str());
        let _region = input.render_stateful(frame, area);
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> super::ActionResult {
        if let Some(c) = get_char(key) {
            self.state.insert_char(c);
        } else if is_backspace(key) {
            self.state.delete_char_backward();
        } else if is_delete(key) {
            self.state.delete_char_forward();
        } else if key.code == KeyCode::Left {
            self.state.move_left();
        } else if key.code == KeyCode::Right {
            self.state.move_right();
        } else if is_home(key) {
            self.state.move_home();
        } else if is_end(key) {
            self.state.move_end();
        } else {
            return super::ActionResult::None;
        }
        super::ActionResult::PatternChanged(self.state.text.clone())
    }
}
