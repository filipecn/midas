use ratatui::layout::{Constraint, Direction, Layout};

use crate::{
    budget::Category,
    budget::category_view::CategoryView,
    widgets::{ActionResult, InteractableWidget, input_box::InputBox},
};

pub struct CategoryPicker {
    pub filter: InputBox,
    pub category_view: CategoryView,
    category_type: Option<String>,
}

impl CategoryPicker {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            filter: InputBox::new("filter"),
            category_view: CategoryView::new()?,
            category_type: None,
        })
    }

    pub fn new_for_type(category_type: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            filter: InputBox::new("filter"),
            category_view: CategoryView::filter_by_type("", Some(category_type))?,
            category_type: Some(category_type.to_string()),
        })
    }

    pub fn selected_category(&self) -> Option<Category> {
        self.category_view.selected_category()
    }
}

impl InteractableWidget for CategoryPicker {
    fn render(&self, area: ratatui::layout::Rect, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Filter
                Constraint::Min(0),    // Remaining space
            ])
            .split(area);

        self.filter.render(chunks[0], frame);
        self.category_view.render(chunks[1], frame);
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        if key.code.is_esc() {
            ActionResult::Cancel
        } else if self.filter.handle_key(key).consumed() {
            self.filter.state.focused = true;
            let _ = self.category_view = CategoryView::filter_by_type(
                self.filter.state.text(),
                self.category_type.as_deref(),
            )
            .unwrap();
            ActionResult::Consumed
        } else {
            self.category_view.handle_key(key)
        }
    }
}
