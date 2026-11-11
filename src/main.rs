use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{Datelike, Local};
use clap::Parser;
use color_eyre::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
};
use ratatui_interact::components::{
    Menu, MenuBar, MenuBarAction, MenuBarClickTarget, MenuBarItem, MenuBarState,
    calculate_menu_bar_height, handle_menu_bar_key, handle_menu_bar_mouse,
};
use ratatui_interact::traits::ClickRegion;

mod b3_eligibility;
mod budget;
mod core;
mod db;
mod fundamentus;
mod investments;
mod market;
mod source_data;
mod scoring;
mod tui;
mod widgets;
use crate::{
    budget::{
        data::{Categories, CategorizationRules},
    },
    core::Accounts,
    db::{DB, DBConnection},
    db::budget::Transactions,
    source_data::fundamentus::{FUNDAMENTUS, Fundamentus},
    db::investments::Investments,
    market::{MARKET_DATA, MarketData},
    tui::TUI,
};
use crate::{
    budget::{expense_groups_view::ExpenseGroupsView, transactions_view::TransactionsView},
    fundamentus::view::FundamentusView,
    investments::{
        investment_transactions_view::InvestmentTransactionsView, investments_view::InvestmentsView,
    },
    market::view::MarketView,
    source_data::pluggy::{PLUGGY, PluggyError},
    widgets::{InteractableWidget, logger::LoggerWidget},
};

enum ActiveView {
    Transactions,
    Investments,
    InvestmentTransactions,
    ExpenseGroups,
    Fundamentus,
    Market,
}

struct App {
    active_view: ActiveView,
    menu_state: MenuBarState,
    bar_area: Rect,
    dropdown_area: Option<Rect>,
    click_regions: Vec<ClickRegion<MenuBarClickTarget>>,
}

impl App {
    fn new() -> Self {
        let mut menu_state = MenuBarState::new();
        menu_state.focused = true;
        Self {
            active_view: ActiveView::ExpenseGroups,
            menu_state,
            bar_area: Rect::default(),
            dropdown_area: None,
            click_regions: Vec::new(),
        }
    }

    fn handle_menu_action(&mut self, action: MenuBarAction) {
        match action {
            MenuBarAction::ItemSelect(id) => match id.as_str() {
                "transactions" => self.active_view = ActiveView::Transactions,
                "investments" => self.active_view = ActiveView::Investments,
                "investment_transactions" => self.active_view = ActiveView::InvestmentTransactions,
                "expense_groups" => self.active_view = ActiveView::ExpenseGroups,
                "fundamentus" => self.active_view = ActiveView::Fundamentus,
                "market" => self.active_view = ActiveView::Market,
                _ => {}
            },
            _ => {}
        }
    }
}

fn create_menus() -> Vec<Menu> {
    vec![
        Menu::new("Budget").items(vec![
            MenuBarItem::action("transactions", "Transactions"),
            MenuBarItem::action("expense_groups", "Expense Groups"),
        ]),
        Menu::new("Investments").items(vec![
            MenuBarItem::action("investments", "Investments"),
            MenuBarItem::action("investment_transactions", "Investment Transactions"),
            MenuBarItem::action("fundamentus", "Fundamentus"),
            MenuBarItem::action("market", "Market"),
        ]),
    ]
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Midas - simple monthly budget TUI")]
struct Args {
    /// Path to SQLite database file
    #[arg(long, default_value = "midas.db")]
    db: String,

    /// Path to categories JSON to pre-load
    #[arg(short, long, default_value = "categories.json")]
    categories: PathBuf,

    /// Path to accounts JSON to pre-load
    #[arg(short, long, default_value = "accounts.json")]
    accounts: PathBuf,

    /// Path to categorization rules JSON file
    #[arg(short, long, default_value = "rules.json")]
    rules: PathBuf,

    /// Path to the source-to-target category map JSON file
    #[arg(long, default_value = "category_map.json")]
    category_map: PathBuf,

    /// Path to transactions CSV files directory
    #[arg(short, long, default_value = "data")]
    data: PathBuf,

    /// Score saved Fundamentus stock snapshots and print a JSON report
    #[arg(long)]
    score: bool,

    /// Optional JSON QARP configuration used with --score
    #[arg(long)]
    score_config: Option<PathBuf>,

    /// Print B3 eligibility validation for saved Fundamentus detail snapshots
    #[arg(long)]
    eligibility: bool,

    /// Import a manually obtained B3 security-evidence JSON snapshot
    #[arg(long)]
    b3_snapshot: Option<PathBuf>,

    /// Maximum age in days for B3 eligibility evidence
    #[arg(long, default_value_t = 1)]
    eligibility_max_age_days: i64,

    /// Apply B3 eligibility when scoring: strict scores only verified eligible securities;
    /// research scores all securities and includes the eligibility report
    #[arg(long, value_enum)]
    score_eligibility: Option<ScoreEligibilityPolicy>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ScoreEligibilityPolicy {
    Strict,
    Research,
}

#[derive(serde::Deserialize)]
struct PluggyKeys {
    client_id: String,
    client_secret: String,
    item_id: String,
}

fn run_tui() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let transactions = db::budget::Transactions::read_all()?;
    let mut transactions_view = TransactionsView::new(transactions.clone())?;
    let investments = db::investments::Investments::read_all()?;
    let investment_transactions = db::investments::InvestmentTransactions::read_all()?;
    let mut investments_view =
        InvestmentsView::new(investments.clone(), investment_transactions.clone());
    let mut investment_transactions_view =
        InvestmentTransactionsView::new(investments, investment_transactions);
    let mut fundamentus_view = FundamentusView::new();
    let mut market_view = MarketView::new();
    let now = Local::now();
    let mut expense_groups = ExpenseGroupsView::new(now.year() as u32, now.month())?;
    let menus = create_menus();
    let mut app = App::new();
    let logger = LoggerWidget::new();
    let mut logger_open = false;

    loop {
        terminal.draw(|f| {
            render_app(
                f,
                &mut app,
                &menus,
                &transactions_view,
                &investments_view,
                &investment_transactions_view,
                &expense_groups,
                &fundamentus_view,
                &market_view,
            );
            if logger_open {
                let log_area = f.area().inner(ratatui::layout::Margin {
                    vertical: 2,
                    horizontal: 4,
                });
                logger.render(log_area, f);
            }
        })?;

        Fundamentus::update();
        fundamentus_view.update();
        MarketData::update();
        market_view.update();

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) => {
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }
                if logger_open {
                    if key.code == KeyCode::Esc || key.code == KeyCode::Char('l') {
                        logger_open = false;
                    } else {
                        logger.handle_key(&key);
                    }
                    continue;
                }
                let menu_is_active = app.menu_state.is_open;
                let is_menu_shortcut = key.modifiers.contains(KeyModifiers::ALT);
                if menu_is_active || is_menu_shortcut {
                    if let Some(action) = handle_menu_bar_key(&key, &mut app.menu_state, &menus) {
                        app.handle_menu_action(action);
                    }
                    continue;
                }
                if key.code == KeyCode::Esc {
                    if matches!(&app.active_view, ActiveView::Fundamentus)
                        && fundamentus_view.close_details_window()
                    {
                        continue;
                    }
                    if matches!(&app.active_view, ActiveView::Market)
                        && market_view.close_details_window()
                    {
                        continue;
                    }
                    break;
                }
                let action_result = match app.active_view {
                    ActiveView::Transactions => transactions_view.handle_key(&key),
                    ActiveView::Investments => investments_view.handle_key(&key),
                    ActiveView::InvestmentTransactions => {
                        investment_transactions_view.handle_key(&key)
                    }
                    ActiveView::ExpenseGroups => expense_groups.handle_key(&key),
                    ActiveView::Fundamentus => fundamentus_view.handle_key(&key),
                    ActiveView::Market => market_view.handle_key(&key),
                };
                if !action_result.consumed() {
                    if key.code == KeyCode::Char('v') {
                        TUI::set_sensitive_values_visible(!TUI::sensitive_values_visible());
                    } else if key.code == KeyCode::Char('l') {
                        logger_open = !logger_open;
                    } else if key.code == KeyCode::Char('q') {
                        break;
                    }
                }
            }
            Event::Mouse(mouse) => {
                if let Some(action) = handle_menu_bar_mouse(
                    &mouse,
                    &mut app.menu_state,
                    app.bar_area,
                    app.dropdown_area,
                    &app.click_regions,
                    &menus,
                ) {
                    if matches!(&action, MenuBarAction::ItemSelect(_)) {
                        app.handle_menu_action(action);
                    }
                }
            }
            _ => {}
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn render_app(
    frame: &mut ratatui::Frame,
    app: &mut App,
    menus: &[Menu],
    transactions_view: &TransactionsView,
    investments_view: &InvestmentsView,
    investment_transactions_view: &InvestmentTransactionsView,
    expense_groups: &ExpenseGroupsView,
    fundamentus_view: &FundamentusView,
    market_view: &MarketView,
) {
    let menu_height = calculate_menu_bar_height();
    let [menu_area, content_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(menu_height), Constraint::Min(1)])
        .areas(frame.area());

    match app.active_view {
        ActiveView::Transactions => transactions_view.render(content_area, frame),
        ActiveView::Investments => investments_view.render(content_area, frame),
        ActiveView::InvestmentTransactions => {
            investment_transactions_view.render(content_area, frame)
        }
        ActiveView::ExpenseGroups => expense_groups.render(content_area, frame),
        ActiveView::Fundamentus => fundamentus_view.render(content_area, frame),
        ActiveView::Market => market_view.render(content_area, frame),
    }

    let menu_bar = MenuBar::new(menus, &app.menu_state);
    let (bar_area, dropdown_area, click_regions) = menu_bar.render_stateful(frame, menu_area);
    app.bar_area = bar_area;
    app.dropdown_area = dropdown_area;
    app.click_regions = click_regions;
}

fn load_transactions() -> Result<(), Box<dyn Error>> {
    let mut db = DB
        .get()
        .ok_or_else(|| rusqlite::Error::InvalidQuery)?
        .lock()
        .unwrap();

    db::budget::init(&db.conn)?;

    let accounts = Accounts::get_all();

    for account in accounts {
        match PLUGGY.with_borrow_mut(|p| p.get_recent_transactions(&account.identifier, 200)) {
            Ok(transactions) => {
                let _ = Transactions::write(&mut db.conn, &transactions.clone());
            }
            Err(PluggyError::Api {
                status,
                message,
                parameter,
            }) => {
                eprintln!("API Error {status}: {message} (Parameter: {parameter:?})");
            }
            Err(PluggyError::MissingApiKey) => eprintln!("Please configure your API key"),
            Err(err) => eprintln!("Unhandled error: {err}"),
        }
    }
    Ok(())
}

fn load_investments() -> Result<(), Box<dyn Error>> {
    let mut db = DB
        .get()
        .ok_or_else(|| rusqlite::Error::InvalidQuery)?
        .lock()
        .unwrap();

    db::investments::init(&db.conn)?;

    let accounts = Accounts::get_all();

    for _ in accounts {
        match PLUGGY.with_borrow_mut(|p| p.get_investments()) {
            Ok(investments) => {
                let _ = Investments::write(&mut db.conn, &investments.clone());
            }
            Err(PluggyError::Api {
                status,
                message,
                parameter,
            }) => {
                eprintln!("API Error {status}: {message} (Parameter: {parameter:?})");
            }
            Err(PluggyError::MissingApiKey) => eprintln!("Please configure your API key"),
            Err(err) => eprintln!("Unhandled error: {err}"),
        }
    }
    Ok(())
}

fn load_investment_transactions() -> Result<(), Box<dyn Error>> {
    let investments = db::investments::Investments::read_all()?;

    for investment in investments {
        match PLUGGY.with_borrow_mut(|p| p.get_investment_transactions(&investment.identifier)) {
            Ok(investment_transactions) => {
                let mut db = DB
                    .get()
                    .ok_or_else(|| rusqlite::Error::InvalidQuery)?
                    .lock()
                    .unwrap();
                db::investments::InvestmentTransactions::write(
                    &mut db.conn,
                    &investment_transactions,
                )?;
            }
            Err(PluggyError::Api {
                status,
                message,
                parameter,
            }) => {
                eprintln!("API Error {status}: {message} (Parameter: {parameter:?})");
            }
            Err(PluggyError::MissingApiKey) => eprintln!("Please configure your API key"),
            Err(err) => eprintln!("Unhandled error: {err}"),
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let instance = MarketData::new(
        std::env::var("TWELVEDATA_API_KEY").ok(),
        std::env::var("FINNHUB_API_KEY").ok(),
    );
    MARKET_DATA.set(Mutex::new(instance)).ok();

    color_eyre::install()?;
    tui_logger::init_logger(log::LevelFilter::Trace)?;
    tui_logger::set_default_level(log::LevelFilter::Trace);
    log::info!(target: "midas", "Logger initialized");

    let args = Args::parse();
    if args.eligibility_max_age_days < 0 {
        return Err("--eligibility-max-age-days must be nonnegative".into());
    }
    if args.score_eligibility.is_some() && !args.score {
        return Err("--score-eligibility requires --score".into());
    }

    if args.score || args.eligibility || args.b3_snapshot.is_some() {
        let db = DBConnection::new(&args.db)?;
        db::fundamentus::init(&db.conn)?;
        b3_eligibility::init(&db.conn)?;
        if let Some(path) = &args.b3_snapshot {
            let snapshot = b3_eligibility::parse_snapshot(&std::fs::read_to_string(path)?)?;
            b3_eligibility::save_snapshot(&db.conn, &snapshot)?;
            eprintln!(
                "Imported {} B3 evidence records from {}.",
                snapshot.securities.len(),
                snapshot.source
            );
        }

        if args.eligibility || args.score_eligibility.is_some() {
            let details = db::fundamentus::Stocks::read_all_from(&db.conn)?;
            let tickers = details
                .iter()
                .map(|stock| stock.ticker.clone())
                .collect::<Vec<_>>();
            let snapshot = b3_eligibility::load_snapshot(&db.conn)?;
            let eligibility = b3_eligibility::build_report(
                &tickers,
                snapshot.as_ref(),
                args.eligibility_max_age_days,
            );
            if args.eligibility && !args.score {
                println!("{}", serde_json::to_string_pretty(&eligibility)?);
                return Ok(());
            }
            if args.score && (args.score_eligibility.is_some() || args.eligibility) {
                let policy = args
                    .score_eligibility
                    .unwrap_or(ScoreEligibilityPolicy::Research);
                let eligible_tickers = b3_eligibility::eligible_tickers(&eligibility);
                let stocks = details
                    .iter()
                    .map(scoring::StockFundamentals::from)
                    .filter(|stock| {
                        matches!(policy, ScoreEligibilityPolicy::Research)
                            || eligible_tickers.contains(&stock.ticker.trim().to_ascii_uppercase())
                    })
                    .collect::<Vec<_>>();
                let config = match &args.score_config {
                    Some(path) => serde_json::from_str(&std::fs::read_to_string(path)?)?,
                    None => scoring::QarpConfig::default(),
                };
                let strategy = scoring::QarpStrategy::new(config)?;
                let score_report = scoring::run_strategy(&strategy, &stocks)?;
                let policy_name = match policy {
                    ScoreEligibilityPolicy::Strict => "strict",
                    ScoreEligibilityPolicy::Research => "research",
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "eligibility_policy": policy_name,
                        "eligibility": eligibility,
                        "scoring": score_report
                    }))?
                );
                return Ok(());
            }
        }

        if args.score {
            let details = db::fundamentus::Stocks::read_all_from(&db.conn)?;
            let stocks = details
                .iter()
                .map(scoring::StockFundamentals::from)
                .collect::<Vec<_>>();
            let config = match &args.score_config {
                Some(path) => serde_json::from_str(&std::fs::read_to_string(path)?)?,
                None => scoring::QarpConfig::default(),
            };
            let strategy = scoring::QarpStrategy::new(config)?;
            let report = scoring::run_strategy(&strategy, &stocks)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else if args.b3_snapshot.is_some() {
            println!("B3 eligibility snapshot cached in the selected database.");
        }
        return Ok(());
    }

    let keys: PluggyKeys = serde_json::from_str(&std::fs::read_to_string("keys")?)?;
    PLUGGY.with_borrow_mut(|pluggy| {
        pluggy.init(keys.client_id, keys.client_secret, Some(keys.item_id))
    })?;

    Categories::load(&args.categories)?;
    Categories::load_map(&args.category_map)?;
    // Accounts::load(&args.accounts)?;
    Accounts::add(&PLUGGY.with_borrow(|pl| pl.get_accounts()));
    CategorizationRules::load(&args.rules)?;

    let db = DBConnection::new(&args.db)?;
    db::fundamentus::init(&db.conn)?;
    b3_eligibility::init(&db.conn)?;
    if let Err(_) = DB.set(Mutex::new(db)) {
        eprintln!("Failed to create/load database");
    }

    let fun = source_data::fundamentus::Fundamentus::new();
    if let Err(_) = FUNDAMENTUS.set(Mutex::new(fun)) {
        eprintln!("Error initializing fundamentus");
    }

    load_transactions()?;
    load_investments()?;
    load_investment_transactions()?;

    run_tui()?;

    Ok(())
}
