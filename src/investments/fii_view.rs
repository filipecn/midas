use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver},
};

use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    source_data::b3::{ListedFii, ListedFiiDetails},
    source_data::fundamentus::FundamentusFiiData,
    widgets::{
        ActionResult, InteractableWidget,
        input_box::InputBox,
        list::{GenericList, ListColumn},
    },
};

pub struct FiiView {
    funds: Option<GenericList<ListedFii>>,
    all_funds: Vec<ListedFii>,
    search: InputBox,
    error: Option<String>,
    receiver: Option<Receiver<Result<Vec<ListedFii>, String>>>,
    details: Option<FiiDetailsPopup>,
}

struct FiiDetailsPopup {
    ticker: String,
    trading_code: Option<String>,
    result: Option<Result<ListedFiiDetails, String>>,
    receiver: Option<Receiver<Result<ListedFiiDetails, String>>>,
    tab: FiiDetailsTab,
    fundamentals: Option<Result<FundamentusFiiData, String>>,
    fundamentals_receiver: Option<Receiver<Result<FundamentusFiiData, String>>>,
    fundamentals_scroll: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FiiDetailsTab {
    B3,
    Fundamentus,
}

fn append_fundamentals_group<'a>(
    lines: &mut Vec<Line<'a>>,
    title: &'a str,
    fields: &'a [(String, String)],
) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        title,
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.extend(
        fields
            .iter()
            .map(|(label, value)| Line::from(format!("{label}: {value}"))),
    );
}

impl FiiView {
    pub fn new() -> Self {
        let mut search = InputBox::new("Search FII");
        search.state.focused = true;
        let mut view = Self {
            funds: None,
            all_funds: Vec::new(),
            search,
            error: None,
            receiver: None,
            details: None,
        };
        view.start_loading();
        view
    }

    fn start_loading(&mut self) {
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.error = None;
        std::thread::spawn(move || {
            let _ = sender.send(crate::source_data::b3::listed_fiis());
        });
    }

    pub fn update(&mut self) {
        if let Some(details) = &mut self.details {
            if let Some(receiver) = &details.receiver {
                match receiver.try_recv() {
                    Ok(result) => {
                        details.result = Some(result);
                        details.receiver = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        details.result =
                            Some(Err("The B3 fund detail request stopped unexpectedly".into()));
                        details.receiver = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(receiver) = &details.fundamentals_receiver {
                match receiver.try_recv() {
                    Ok(result) => {
                        details.fundamentals = Some(result);
                        details.fundamentals_receiver = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        details.fundamentals =
                            Some(Err("Fundamentus FII request stopped unexpectedly".into()));
                        details.fundamentals_receiver = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(funds)) => {
                self.all_funds = funds;
                self.refresh();
                self.receiver = None;
            }
            Ok(Err(error)) => {
                self.error = Some(error);
                self.receiver = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.error = Some("The B3 FII request stopped unexpectedly".into());
                self.receiver = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn refresh(&mut self) {
        let term = self.search.state.text.to_lowercase();
        let rows = self
            .all_funds
            .iter()
            .filter(|fund| {
                fund.ticker.to_lowercase().contains(&term)
                    || fund
                        .trading_code
                        .as_deref()
                        .is_some_and(|code| code.to_lowercase().contains(&term))
                    || fund.name.to_lowercase().contains(&term)
                    || fund.legal_name.to_lowercase().contains(&term)
            })
            .cloned()
            .collect();
        let columns = BTreeMap::from([
            (
                "Code".to_string(),
                ListColumn::new(16, |fund: &ListedFii| fund.ticker.clone()),
            ),
            (
                "Trading code".to_string(),
                ListColumn::new(18, |fund: &ListedFii| {
                    fund.trading_code.clone().unwrap_or_else(|| "-".into())
                }),
            ),
            (
                "Fund".to_string(),
                ListColumn::new(80, |fund: &ListedFii| fund.name.clone()),
            ),
            (
                "Razão Social".to_string(),
                ListColumn::new(100, |fund: &ListedFii| fund.legal_name.clone()),
            ),
        ]);
        self.funds = Some(
            GenericList::new(rows, columns)
                .with_title("B3 listed FIIs")
                .with_column_order(["Code", "Trading code", "Fund", "Razão Social"])
                .with_sticky_header(),
        );
    }

    pub fn close_details_window(&mut self) -> bool {
        self.details.take().is_some()
    }

    fn open_selected_details(&mut self) {
        let Some(fund) = self
            .funds
            .as_ref()
            .and_then(|funds| funds.selected_row())
            .cloned()
        else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.details = Some(FiiDetailsPopup {
            ticker: fund.ticker.clone(),
            trading_code: fund.trading_code.clone(),
            result: None,
            receiver: Some(receiver),
            tab: FiiDetailsTab::B3,
            fundamentals: None,
            fundamentals_receiver: None,
            fundamentals_scroll: 0,
        });
        if let Some(trading_code) = fund.trading_code.clone() {
            let (fundamentals_sender, fundamentals_receiver) = mpsc::channel();
            if let Some(popup) = &mut self.details {
                popup.fundamentals_receiver = Some(fundamentals_receiver);
            }
            std::thread::spawn(move || {
                let result = crate::source_data::fundamentus::fetch_fii_fundamentals(&trading_code);
                let _ = fundamentals_sender.send(result);
            });
        }
        std::thread::spawn(move || {
            let _ = sender.send(crate::source_data::b3::listed_fii_details(&fund.ticker));
        });
    }

    fn render_details(&self, area: Rect, frame: &mut ratatui::Frame) {
        let Some(popup) = &self.details else {
            return;
        };
        let width = area.width.saturating_sub(2).min(120);
        let height = area.height.saturating_sub(2).min(40);
        let popup_area = Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup_area);
        let block = Block::default().borders(Borders::ALL).title(format!(
            " {} details | Tab switch tab | Esc close ",
            popup.ticker
        ));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let [tabs_area, content_area] =
            ratatui::layout::Layout::vertical([Constraint::Length(1), Constraint::Min(0)])
                .areas(inner);
        let b3_title = if popup.tab == FiiDetailsTab::B3 {
            "[B3]"
        } else {
            " B3 "
        };
        let fundamentus_title = if popup.tab == FiiDetailsTab::Fundamentus {
            "[Fundamentus]"
        } else {
            " Fundamentus "
        };
        frame.render_widget(
            Paragraph::new(format!("{b3_title}   {fundamentus_title}")),
            tabs_area,
        );
        if popup.tab == FiiDetailsTab::Fundamentus {
            match (&popup.trading_code, &popup.fundamentals) {
                (None, _) => frame.render_widget(
                    Paragraph::new(
                        "No trading code is available for this fund in the B3 securities catalog.",
                    ),
                    content_area,
                ),
                (_, None) => {
                    frame.render_widget(Paragraph::new("Loading Fundamentus data..."), content_area)
                }
                (_, Some(Err(error))) => frame.render_widget(
                    Paragraph::new(format!("Could not load Fundamentus data: {error}")),
                    content_area,
                ),
                (_, Some(Ok(data))) => {
                    let mut lines = vec![Line::from(Span::styled(
                        data.name.as_str(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ))];
                    append_fundamentals_group(&mut lines, "Overview", &data.overview);
                    append_fundamentals_group(&mut lines, "Indicators", &data.indicators);
                    append_fundamentals_group(
                        &mut lines,
                        "Results · last 12 months",
                        &data.results_12m,
                    );
                    append_fundamentals_group(
                        &mut lines,
                        "Results · last 3 months",
                        &data.results_3m,
                    );
                    append_fundamentals_group(&mut lines, "Balance sheet", &data.balance);
                    append_fundamentals_group(&mut lines, "Asset composition", &data.assets);
                    frame.render_widget(
                        Paragraph::new(Text::from(lines))
                            .wrap(Wrap { trim: true })
                            .scroll((popup.fundamentals_scroll, 0)),
                        content_area,
                    );
                }
            }
            return;
        }
        let content = match &popup.result {
            None => Text::from("Loading B3 fund details..."),
            Some(Err(error)) => Text::from(format!("Could not load fund details: {error}")),
            Some(Ok(details)) => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Fund: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(details.fund.trim()),
                    ]),
                    Line::from(format!(
                        "Fund code: {}    Trading code: {}    CVM code: {}",
                        details.code.trim(),
                        popup.trading_code.as_deref().unwrap_or("-"),
                        details.code_cvm.as_deref().unwrap_or("-")
                    )),
                    Line::from(format!(
                        "Trading segment: {}    Trading lot: {}",
                        details.segment.trim(),
                        details.trading_lot.trim()
                    )),
                    Line::from(format!(
                        "Form: {}    Bookkeeping agent: {}",
                        details.form.trim(),
                        details.ifd.trim()
                    )),
                    Line::from(format!(
                        "Units: {}    Quotation date: {}",
                        details.quantity.trim(),
                        details.quoted_per.trim()
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Recent cash distributions",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                ];
                lines.extend(details.cash_dividends.iter().take(10).map(|dividend| {
                    Line::from(format!(
                        "{} | {} | R$ {} | {} | {}",
                        dividend.payment_date,
                        dividend.label,
                        dividend.rate,
                        dividend.related_to,
                        dividend.isin_code
                    ))
                }));
                Text::from(lines)
            }
        };
        frame.render_widget(Paragraph::new(content).wrap(Wrap { trim: true }), inner);
    }
}

impl Default for FiiView {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractableWidget for FiiView {
    fn render(&self, area: Rect, frame: &mut ratatui::Frame) {
        let [search_area, content_area] =
            ratatui::layout::Layout::vertical([Constraint::Length(3), Constraint::Min(0)])
                .areas(area);
        self.search.render(search_area, frame);
        if let Some(error) = &self.error {
            frame.render_widget(
                Paragraph::new(format!("{error}  |  Press r to retry"))
                    .alignment(Alignment::Center),
                content_area,
            );
        } else if let Some(funds) = &self.funds {
            if funds.rows.is_empty() && !self.search.state.text.is_empty() {
                frame.render_widget(
                    Paragraph::new("No FIIs match that search").alignment(Alignment::Center),
                    content_area,
                );
            } else {
                funds.render(content_area, frame);
            }
        } else {
            frame.render_widget(
                Paragraph::new("Loading B3 listed FIIs...").alignment(Alignment::Center),
                content_area,
            );
        }
        self.render_details(area, frame);
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        if self.details.is_some() {
            match key.code {
                crossterm::event::KeyCode::Esc => {
                    self.close_details_window();
                }
                crossterm::event::KeyCode::Tab => {
                    if let Some(popup) = &mut self.details {
                        popup.tab = match popup.tab {
                            FiiDetailsTab::B3 => FiiDetailsTab::Fundamentus,
                            FiiDetailsTab::Fundamentus => FiiDetailsTab::B3,
                        };
                    }
                }
                crossterm::event::KeyCode::Down => {
                    if let Some(popup) = &mut self.details {
                        if popup.tab == FiiDetailsTab::Fundamentus {
                            popup.fundamentals_scroll = popup.fundamentals_scroll.saturating_add(1);
                        }
                    }
                }
                crossterm::event::KeyCode::Up => {
                    if let Some(popup) = &mut self.details {
                        if popup.tab == FiiDetailsTab::Fundamentus {
                            popup.fundamentals_scroll = popup.fundamentals_scroll.saturating_sub(1);
                        }
                    }
                }
                _ => {}
            }
            return ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Char('r') && self.error.is_some() {
            self.start_loading();
            return ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Tab {
            self.search.state.focused = !self.search.state.focused;
            return ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Enter {
            self.open_selected_details();
            return ActionResult::Consumed;
        }
        if self.search.state.focused {
            match self.search.handle_key(key) {
                ActionResult::PatternChanged(_) => {
                    self.refresh();
                    return ActionResult::Consumed;
                }
                ActionResult::None => {}
                result => return result,
            }
        }
        self.funds
            .as_mut()
            .map_or(ActionResult::None, |funds| funds.handle_key(key))
    }
}
