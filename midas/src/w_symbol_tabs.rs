use dionysus::{finance::Token, ERROR};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{palette::tailwind, Color},
    widgets::{Tabs, Widget},
};
use slog::slog_error;
use std::iter::Iterator;

struct TabItem {
    token: Token,
    midas_indices: Vec<usize>,
    selected_index: usize,
}

#[derive(Default)]
pub struct SymbolTabs {
    selected_tab: usize,
    tabs: Vec<TabItem>,
}

impl TabItem {
    pub fn current(&self) -> Option<usize> {
        if self.midas_indices.len() > self.selected_index {
            Some(self.midas_indices[self.selected_index])
        } else {
            None
        }
    }

    pub fn add_index(&mut self, index: usize) {
        for i in &self.midas_indices {
            if *i == index {
                return;
            }
        }
        self.midas_indices.push(index);
        self.selected_index = self.midas_indices.len().saturating_sub(1);
    }

    pub fn next(&mut self) {
        if !self.midas_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.midas_indices.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.midas_indices.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.midas_indices.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

impl SymbolTabs {
    pub fn current(&self) -> Option<(usize, Token)> {
        if self.selected_tab < self.tabs.len() {
            if let Some(index) = self.tabs[self.selected_tab].current() {
                return Some((index, self.tabs[self.selected_tab].token.clone()));
            }
        }
        None
    }

    pub fn current_midas_index(&self) -> Option<usize> {
        if self.selected_tab < self.tabs.len() {
            if let Some(index) = self.tabs[self.selected_tab].current() {
                return Some(index);
            }
        }
        None
    }

    fn open_tab(&mut self, token: &Token) -> usize {
        for i in 0..self.tabs.len() {
            if self.tabs[i].token == token.clone() {
                return i;
            }
        }
        self.tabs.push(TabItem {
            token: token.clone(),
            midas_indices: Vec::new(),
            selected_index: 0,
        });
        self.tabs.len().saturating_sub(1)
    }

    pub fn add(&mut self, token: &Token, midas_index: usize) {
        let tab_index = self.open_tab(token);
        self.tabs[tab_index].add_index(midas_index);
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

    pub fn next_item(&mut self) {
        if self.selected_tab < self.tabs.len() {
            self.tabs[self.selected_tab].next();
        }
    }

    pub fn previous_item(&mut self) {
        if self.selected_tab < self.tabs.len() {
            self.tabs[self.selected_tab].previous();
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn tab(&self, i: usize) -> Option<(usize, Token)> {
        if i < self.tabs.len() {
            if self.tabs[i].midas_indices.len() > self.tabs[i].selected_index {
                return Some((
                    self.tabs[i].midas_indices[self.tabs[i].selected_index],
                    self.tabs[i].token.clone(),
                ));
            }
        }
        None
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
            .map(|x| format!("{:?}", x.token.name()))
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
