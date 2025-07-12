use dionysus::{strategy::Strategy, time::TimeUnit};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use tui_prompts::prelude::*;

#[derive(Default)]
pub struct OracleWindow<'a> {
    pub strategy: Strategy,
    current_field: usize,
    fields: Vec<(String, TextState<'a>)>,
}

impl<'a> OracleWindow<'a> {
    pub fn open(&mut self, strategy: &Strategy) {
        self.strategy = strategy.clone();
        self.fields.push((
            String::from("Oracle:          "),
            TextState::default().with_value(strategy.oracle.name()),
        ));
        self.fields.push((
            String::from("Time Resolution: "),
            TextState::default().with_value(strategy.duration.resolution.name()),
        ));
        for c in &strategy.counselors {
            self.fields.push((
                String::from("Conselour: "),
                TextState::default().with_value(c.name()),
            ));
        }
    }

    pub fn close(&mut self) {
        self.current_field = 0;
        self.fields.clear();
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, 2); 2])
            .split(area);

        let prompt_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1); self.fields.len()])
            .split(areas[0]);

        for (i, field) in self.fields.iter_mut().enumerate() {
            TextPrompt::from(field.0.clone()).draw(frame, prompt_areas[i], &mut field.1);
        }

        let strategy = self.strategy.clone();
        let debug = format!("{strategy:#?}");
        frame.render_widget(
            Paragraph::new(debug)
                .wrap(Wrap { trim: false })
                .block(Block::new().borders(Borders::LEFT)),
            areas[1],
        );
    }

    pub fn current(&mut self) -> &mut TextState<'a> {
        &mut self.fields[self.current_field].1
    }

    pub fn focus_next(&mut self) {
        self.fields[self.current_field].1.blur();
        self.current_field = (self.current_field + 1) % self.fields.len();
        self.fields[self.current_field].1.focus();
    }

    pub fn focus_prev(&mut self) {
        self.fields[self.current_field].1.blur();
        self.current_field = if self.current_field > 0 {
            self.current_field - 1
        } else {
            self.fields.len().saturating_sub(1)
        };
        self.fields[self.current_field].1.focus();
    }

    pub fn submit(&mut self) {
        self.current().complete();
        match self.current_field {
            0 => (),
            1 => {
                self.strategy.duration.resolution =
                    TimeUnit::from_name(self.current().value().into())
            }
            2 => (),
            _ => (),
        }
        self.focus_next();
    }
}
