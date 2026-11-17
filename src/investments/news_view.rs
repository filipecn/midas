use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver},
};

use chrono::Datelike;
use ratatui::{
    layout::{Alignment, Rect},
    widgets::Paragraph,
};

use crate::{
    source_data::cvm::{
        CsvEncoding, CsvOptions, ResourceDownloader,
        companies::{CorporateDocumentKind, IpeDocument, parse_ipe_rows},
        extract_zip,
    },
    widgets::{
        ActionResult, InteractableWidget,
        input_box::InputBox,
        list::{GenericList, ListColumn},
    },
};

pub struct NewsView {
    year: i32,
    facts: Option<GenericList<IpeDocument>>,
    all_facts: Vec<IpeDocument>,
    company_filter: InputBox,
    error: Option<String>,
    receiver: Option<Receiver<Result<Vec<IpeDocument>, String>>>,
}

impl NewsView {
    pub fn new() -> Self {
        let mut company_filter = InputBox::new("Filter company");
        company_filter.state.focused = true;
        let mut view = Self {
            year: chrono::Local::now().year(),
            facts: None,
            all_facts: Vec::new(),
            company_filter,
            error: None,
            receiver: None,
        };
        view.start_loading();
        view
    }

    fn start_loading(&mut self) {
        let (sender, receiver) = mpsc::channel();
        let year = self.year;
        self.error = None;
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let result = load_material_facts(year);
            let _ = sender.send(result);
        });
    }

    pub fn update(&mut self) {
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(rows)) => {
                self.all_facts = rows;
                self.refresh_filtered_facts(None);
                self.receiver = None;
            }
            Ok(Err(error)) => {
                self.error = Some(error);
                self.receiver = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.error = Some("The CVM news request stopped unexpectedly".into());
                self.receiver = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn refresh_filtered_facts(&mut self, selected_protocol: Option<&str>) {
        let filter = self.company_filter.state.text.to_lowercase();
        let rows = self
            .all_facts
            .iter()
            .filter(|fact| fact.company_name.to_lowercase().contains(&filter))
            .cloned()
            .collect::<Vec<_>>();
        let mut facts = create_facts_list(rows);
        if let Some(protocol) = selected_protocol {
            if let Some(index) = facts
                .rows
                .iter()
                .position(|fact| fact.protocol.as_deref() == Some(protocol))
            {
                facts.state.selected_index = index;
            }
        }
        self.facts = Some(facts);
    }
}

impl Default for NewsView {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractableWidget for NewsView {
    fn render(&self, area: Rect, frame: &mut ratatui::Frame) {
        let [filter_area, content_area] = ratatui::layout::Layout::vertical([
            ratatui::layout::Constraint::Length(3),
            ratatui::layout::Constraint::Min(0),
        ])
        .areas(area);
        self.company_filter.render(filter_area, frame);
        if let Some(error) = &self.error {
            frame.render_widget(
                Paragraph::new(format!("{error}  |  Press r to retry"))
                    .alignment(Alignment::Center),
                content_area,
            );
        } else if let Some(facts) = &self.facts {
            if facts.rows.is_empty() && !self.company_filter.state.text.is_empty() {
                frame.render_widget(
                    Paragraph::new("No companies match that search").alignment(Alignment::Center),
                    content_area,
                );
            } else {
                facts.render(content_area, frame);
            }
        } else {
            frame.render_widget(
                Paragraph::new(format!("Loading CVM material facts for {}...", self.year))
                    .alignment(Alignment::Center),
                content_area,
            );
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> ActionResult {
        if key.code == crossterm::event::KeyCode::Char('r') && self.error.is_some() {
            self.start_loading();
            return ActionResult::Consumed;
        }
        if key.code == crossterm::event::KeyCode::Tab {
            self.company_filter.state.focused = !self.company_filter.state.focused;
            return ActionResult::Consumed;
        }
        if self.company_filter.state.focused {
            match self.company_filter.handle_key(key) {
                ActionResult::PatternChanged(_) => {
                    let selected_protocol = self
                        .facts
                        .as_ref()
                        .and_then(|facts| facts.selected_row())
                        .and_then(|fact| fact.protocol.as_deref())
                        .map(str::to_owned);
                    self.refresh_filtered_facts(selected_protocol.as_deref());
                    return ActionResult::Consumed;
                }
                ActionResult::None => {}
                result => return result,
            }
        }
        self.facts
            .as_mut()
            .map_or(ActionResult::None, |facts| facts.handle_key(key))
    }
}

fn create_facts_list(rows: Vec<IpeDocument>) -> GenericList<IpeDocument> {
    let columns = BTreeMap::from([
        (
            "Date".to_string(),
            ListColumn::new(12, |fact: &IpeDocument| {
                fact.submission_date
                    .or(fact.reference_date)
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "-".into())
            }),
        ),
        (
            "Company".to_string(),
            ListColumn::new(28, |fact: &IpeDocument| fact.company_name.clone()),
        ),
        (
            "Material fact".to_string(),
            ListColumn::new(60, |fact: &IpeDocument| {
                fact.subject
                    .as_deref()
                    .filter(|subject| !subject.trim().is_empty())
                    .or(fact.species.as_deref())
                    .or(fact.document_type.as_deref())
                    .unwrap_or("Material Fact")
                    .to_string()
            }),
        ),
    ]);
    GenericList::new(rows, columns)
        .with_title("CVM material facts (IPE)")
        .with_column_order(["Date", "Company", "Material fact"])
        .with_sticky_header()
}

fn load_material_facts(year: i32) -> Result<Vec<IpeDocument>, String> {
    use crate::source_data::cvm::{CkanClient, companies::CompanyDisclosureSource};

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("midas-cvm-news-{}-{stamp}", std::process::id()));
    let result = (|| {
        let ckan = CkanClient::with_defaults().map_err(|error| error.to_string())?;
        let downloader = ResourceDownloader::default();
        let source = CompanyDisclosureSource::new(&ckan, &downloader);
        let archive = source
            .download_ipe(year, directory.join(format!("ipe-{year}.zip")), false)
            .map_err(|error| error.to_string())?;
        let extracted =
            extract_zip(archive.path, directory.join("ipe")).map_err(|error| error.to_string())?;
        let csv = extracted
            .files
            .into_iter()
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy().to_ascii_lowercase();
                    name.starts_with("ipe_cia_aberta_") && name.ends_with(".csv")
                })
            })
            .ok_or_else(|| "The IPE archive did not contain its annual CSV".to_string())?;
        let options = CsvOptions {
            encoding: CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };
        let mut facts = parse_ipe_rows(
            std::fs::File::open(csv).map_err(|error| error.to_string())?,
            options,
        )
        .filter_map(Result::ok)
        .filter(|fact| fact.kind() == CorporateDocumentKind::MaterialFact)
        .collect::<Vec<_>>();
        facts.sort_by_key(|fact| std::cmp::Reverse(fact.submission_date.or(fact.reference_date)));
        facts.truncate(250);
        Ok(facts)
    })();
    let _ = std::fs::remove_dir_all(directory);
    result
}
