use dionysus::finance::Token;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{palette::tailwind, Color},
    widgets::{Tabs, Widget},
};
use std::iter::Iterator;

#[derive(Default)]
pub struct SymbolTabs {
    selected_tab: usize,
    tabs: Vec<Token>,
}

impl SymbolTabs {
    pub fn current(&self) -> Option<(usize, Token)> {
        if self.selected_tab < self.tabs.len() {
            Some((self.selected_tab, self.tabs[self.selected_tab].clone()))
        } else {
            None
        }
    }

    pub fn add(&mut self, token: &Token) {
        for sta in &self.tabs {
            if *sta == *token {
                return;
            }
        }

        self.tabs.push(token.clone());
        self.selected_tab = self.tabs.len() - 1;
    }

    pub fn remove(&mut self, token: &Token) {
        let index = self
            .tabs
            .iter()
            .position(|x| *x == *token)
            .unwrap_or(self.tabs.len());
        if index < self.tabs.len() {
            self.tabs.remove(index);
        }
    }

    pub fn next(&mut self) {
        if !self.tabs.is_empty() {
            self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.tabs.is_empty() {
            if self.selected_tab == 0 {
                self.selected_tab = self.tabs.len() - 1;
            } else {
                self.selected_tab -= 1;
            }
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn tab(&self, i: usize) -> Option<Token> {
        if i < self.tabs.len() {
            Some(self.tabs[i].clone())
        } else {
            None
        }
    }
}

impl Widget for &SymbolTabs {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.tabs.is_empty() {
            return;
        }
        let tab_titles: Vec<String> = self
            .tabs
            .iter()
            .map(|x| format!("{:?}/{:?}", x.get_symbol(), x.get_currency()))
            .collect();
        let highlight_style = (Color::default(), tailwind::BLUE.c700);
        let selected_tab_index = self.selected_tab as usize;
        Tabs::new(tab_titles)
            .highlight_style(highlight_style)
            .select(selected_tab_index)
            .padding("", "")
            .divider(" ")
            .render(area, buf);
    }
}
