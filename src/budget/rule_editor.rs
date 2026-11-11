use crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui_interact::{
    components::{ButtonState, ButtonStyle, ButtonVariant},
    events::is_activate_key,
    state::FocusManager,
};

use crate::{
    budget::category_picker::CategoryPicker,
    widgets::{ButtonDef, InteractableWidget, input_box::InputBox},
};

/// Which panel is focused
#[derive(Clone, Eq, PartialEq, Hash)]
enum FocusedItem {
    Pattern,
    Category,
    Apply,
    Submit,
}

pub struct RuleEditor {
    focus: FocusManager<FocusedItem>,
    pub category_type: String,
    pub pattern: InputBox,
    pub category_picker: CategoryPicker,
    pub submit_button: ButtonDef,
    pub apply_button: ButtonDef,
}

impl RuleEditor {
    pub fn new(category_type: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut focus = FocusManager::new();
        focus.register(FocusedItem::Pattern);
        focus.register(FocusedItem::Category);
        focus.register(FocusedItem::Apply);
        focus.register(FocusedItem::Submit);
        let mut pattern = InputBox::new("pattern");
        pattern.state.focused = true;
        Ok(Self {
            focus,
            category_type: category_type.to_string(),
            pattern,
            category_picker: CategoryPicker::new_for_type(category_type)?,
            submit_button: ButtonDef {
                label: "submit",
                icon: None,
                variant: ButtonVariant::SingleLine,
                style: ButtonStyle::default(),
                state: ButtonState::enabled(),
            },
            apply_button: ButtonDef {
                label: "apply",
                icon: None,
                variant: ButtonVariant::SingleLine,
                style: ButtonStyle::default(),
                state: ButtonState::enabled(),
            },
        })
    }
}

impl InteractableWidget for RuleEditor {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // pattern
                Constraint::Min(0),    // category
                Constraint::Length(3), // submit
            ])
            .split(area);

        let button_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        self.pattern.render(chunks[0], frame);
        self.category_picker.render(chunks[1], frame);
        let apply_button = self.apply_button.button();
        apply_button.render_stateful(button_chunks[0], frame.buffer_mut());
        let submit_button = self.submit_button.button();
        submit_button.render_stateful(button_chunks[1], frame.buffer_mut());
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> super::ActionResult {
        let mut result = super::ActionResult::Consumed;
        match key.code {
            KeyCode::Esc => {
                result = super::ActionResult::Cancel;
            }
            KeyCode::Tab => self.focus.next(),
            KeyCode::BackTab => self.focus.prev(),
            _ => match self.focus.current().unwrap() {
                FocusedItem::Pattern => {
                    result = self.pattern.handle_key(key);
                }
                FocusedItem::Category => {
                    result = self.category_picker.handle_key(key);
                }
                FocusedItem::Apply => {
                    if is_activate_key(key) {
                        result = super::ActionResult::Category(
                            self.category_picker.selected_category().unwrap(),
                        );
                    }
                }
                FocusedItem::Submit => {
                    if is_activate_key(key) {
                        result = super::ActionResult::SubmitRule(
                            self.pattern.state.text.clone(),
                            self.category_picker.selected_category().unwrap(),
                        );
                    }
                }
            },
        };
        self.pattern.state.focused = self.focus.is_focused(&FocusedItem::Pattern);
        self.category_picker.filter.state.focused = self.focus.is_focused(&FocusedItem::Category);
        self.apply_button.state.focused = self.focus.is_focused(&FocusedItem::Apply);
        self.submit_button.state.focused = self.focus.is_focused(&FocusedItem::Submit);
        result
    }
}
