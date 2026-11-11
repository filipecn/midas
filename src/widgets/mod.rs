use ratatui_interact::components::{Button, ButtonState, ButtonStyle, ButtonVariant};

use crate::budget::{Category, Transaction};

pub mod input_box;
pub mod list;
pub mod logger;

pub enum ActionResult {
    None,
    Consumed,
    PatternChanged(String),
    Submit,
    Cancel,
    Category(Category),
    SubmitRule(String, Category),
    SelectTransaction(Transaction),
    SelectId(usize),
}

impl ActionResult {
    pub fn consumed(&self) -> bool {
        match self {
            ActionResult::None => false,
            _ => true,
        }
    }
}

pub trait InteractableWidget {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame);
    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult;
}

pub struct ButtonDef {
    pub label: &'static str,
    pub icon: Option<&'static str>,
    pub variant: ButtonVariant,
    pub style: ButtonStyle,
    pub state: ButtonState,
}

impl ButtonDef {
    pub fn button(&self) -> Button<'_> {
        let mut b = Button::new(self.label, &self.state)
            .variant(self.variant)
            .style(self.style.clone());
        if let Some(icon) = self.icon {
            b = b.icon(icon);
        }
        b
    }
}
