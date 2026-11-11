use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::{fs::File, path::PathBuf};

use crate::budget::{CategorizationRule, Category, Transaction};

pub struct Categories;
pub struct CategorizationRules;

thread_local! {
    static BUDGET_STATIC_DATA: RefCell<BudgetStaticData> = {
        RefCell::new(BudgetStaticData::default())
    }
}

#[macro_export]
macro_rules! get_budget_static_data {
    ($field:ident) => {
        $crate::budget::data::BUDGET_STATIC_DATA.with_borrow(|s| s.$field.clone())
    };
}

struct BudgetStaticData {
    // (name, Category)
    pub categories: HashMap<String, Category>,
    pub expense_categorization_rules: Vec<CategorizationRule>,
    pub income_categorization_rules: Vec<CategorizationRule>,
    pub categorization_rules_json: PathBuf,
    pub category_map: Vec<CategoryMapJson>,
    pub category_colors: HashMap<String, ratatui::style::Color>,
}

impl Default for BudgetStaticData {
    fn default() -> Self {
        Self {
            categories: HashMap::new(),
            expense_categorization_rules: Vec::new(),
            income_categorization_rules: Vec::new(),
            categorization_rules_json: PathBuf::new(),
            category_map: Vec::new(),
            category_colors: HashMap::new(),
        }
    }
}

impl BudgetStaticData {
    pub fn get_category(&self, category: &str) -> Option<Category> {
        self.categories.get(category).cloned()
    }

    pub fn get_categories(&self) -> Vec<Category> {
        self.categories.values().cloned().collect()
    }

    pub fn get_top_categories(&self) -> Vec<Category> {
        self.categories
            .values()
            .filter(|category| category.parent.is_none())
            .cloned()
            .collect()
    }

    pub fn get_subcategories(&self, parent: &str) -> Vec<Category> {
        self.categories
            .values()
            .filter(|category| category.parent == Some(parent.to_string()))
            .cloned()
            .collect()
    }
}

impl Categories {
    pub fn load(path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let f = File::open(path)?;
        let categories_json: Vec<CategoryJson> = serde_json::from_reader(f)?;
        let mut categories = HashMap::new();
        let mut category_colors = HashMap::new();
        for cat in categories_json {
            let parent_color = parse_hex_color(&cat.color)?;
            category_colors.insert(cat.name.clone(), parent_color);
            categories.insert(
                cat.name.clone(),
                Category {
                    name: cat.name.clone(),
                    category_type: cat.category_type.clone(),
                    frequency: cat.frequency.clone(),
                    description: cat.description,
                    parent: None,
                },
            );
            for (index, subcat) in cat.subcategories.into_iter().enumerate() {
                category_colors.insert(
                    subcat.name.clone(),
                    vary_color_saturation(parent_color, index),
                );
                categories.insert(
                    subcat.name.clone(),
                    Category {
                        name: subcat.name,
                        category_type: cat.category_type.clone(),
                        frequency: subcat.frequency,
                        description: subcat.description,
                        parent: Some(cat.name.clone()),
                    },
                );
            }
        }
        BUDGET_STATIC_DATA.with_borrow_mut(|sd| {
            sd.categories = categories;
            sd.category_colors = category_colors;
        });
        Ok(())
    }

    pub fn top_categories() -> Vec<Category> {
        BUDGET_STATIC_DATA.with_borrow(|sd| sd.get_top_categories())
    }
    pub fn subcategories(parent: &str) -> Vec<Category> {
        BUDGET_STATIC_DATA.with_borrow(|sd| sd.get_subcategories(parent))
    }
    pub fn load_map(path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let f = File::open(path)?;
        let category_map: Vec<CategoryMapJson> = serde_json::from_reader(f)?;
        BUDGET_STATIC_DATA.with_borrow_mut(|sd| {
            sd.category_map = category_map;
        });
        Ok(())
    }
    pub fn map() -> Vec<CategoryMapJson> {
        BUDGET_STATIC_DATA.with_borrow(|sd| sd.category_map.clone())
    }
    pub fn get(name: &str) -> Option<Category> {
        BUDGET_STATIC_DATA.with_borrow(|sd| sd.get_category(name))
    }
    /// Find category with name or return Uncategorized.
    pub fn get_or_uncategorized(name: &str) -> Category {
        BUDGET_STATIC_DATA.with_borrow(|sd| {
            if let Some(category) = sd.categories.get(name) {
                category.clone()
            } else {
                Category::uncategorized()
            }
        })
    }
    pub fn income() -> Category {
        BUDGET_STATIC_DATA.with_borrow(|sd| {
            if let Some(category) = sd.categories.get("Income") {
                category.clone()
            } else {
                Category {
                    name: "Income".to_string(),
                    category_type: "income".to_string(),
                    frequency: "variable".to_string(),
                    description: Some("Income category".to_string()),
                    parent: None,
                }
            }
        })
    }
    pub fn is_income(category: &Category) -> bool {
        category.category_type == "income"
    }
    /// Find top level category.
    pub fn top_of(name: &str) -> String {
        let category = Categories::get_or_uncategorized(name);
        if let Some(parent) = category.parent {
            parent
        } else {
            category.name
        }
    }
    pub fn overlap(parent: Category, child: Category) -> bool {
        if parent.name == child.name || parent.name == Categories::top_of(&child.name) {
            return true;
        }
        let mut current = child;
        while let Some(parent_name) = &current.parent {
            if parent_name == &parent.name {
                return true;
            }
            current = Categories::get_or_uncategorized(parent_name);
        }
        false
    }
    pub fn get_expense_groups() -> Vec<(String, Vec<Category>)> {
        let mut groups: HashMap<String, Vec<Category>> = HashMap::new();
        BUDGET_STATIC_DATA.with_borrow(|sd| {
            for category in sd.categories.values() {
                if category.category_type != "expense" {
                    continue;
                }
                if let Some(parent) = &category.parent {
                    groups
                        .entry(parent.clone())
                        .or_insert_with(Vec::new)
                        .push(category.clone());
                } else {
                    groups.entry(category.name.clone()).or_insert_with(Vec::new);
                }
            }
        });
        let mut result: Vec<(String, Vec<Category>)> = groups.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
    pub fn get_top_category_color(category_name: &str) -> ratatui::style::Color {
        let top_category = Categories::top_of(category_name);
        BUDGET_STATIC_DATA
            .with_borrow(|sd| sd.category_colors.get(&top_category).cloned())
            .unwrap_or(ratatui::style::Color::Gray)
    }
    pub fn get_color(category_name: &str) -> ratatui::style::Color {
        BUDGET_STATIC_DATA
            .with_borrow(|sd| sd.category_colors.get(category_name).cloned())
            .unwrap_or(ratatui::style::Color::Gray)
    }
}

impl CategorizationRules {
    pub fn load(path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let f = File::open(path)?;
        let rules_json: Vec<CategorizationRuleJson> = serde_json::from_reader(f)?;
        BUDGET_STATIC_DATA.with_borrow_mut(|sd| {
            sd.expense_categorization_rules.clear();
            sd.income_categorization_rules.clear();
            sd.categorization_rules_json = path.clone();
            for rule_json in rules_json {
                let category = rule_json.subcategory.unwrap_or(rule_json.category);
                let rule = CategorizationRule {
                    pattern: rule_json.pattern,
                    category: category.clone(),
                };
                if let Some(category) = sd.get_category(&category) {
                    if Categories::is_income(&category) {
                        sd.income_categorization_rules.push(rule);
                    } else {
                        sd.expense_categorization_rules.push(rule);
                    }
                }
            }
        });
        Ok(())
    }
    pub fn expense_rules() -> Vec<CategorizationRule> {
        BUDGET_STATIC_DATA.with_borrow(|sd| sd.expense_categorization_rules.clone())
    }
    pub fn income_rules() -> Vec<CategorizationRule> {
        BUDGET_STATIC_DATA.with_borrow(|sd| sd.income_categorization_rules.clone())
    }
    pub fn write() -> Result<(), Box<dyn Error>> {
        let rules_json = get_budget_static_data!(categorization_rules_json);
        let file = File::create(rules_json.clone())?;
        let mut rules: Vec<CategorizationRuleJson> = Vec::new();
        BUDGET_STATIC_DATA.with_borrow(|static_data| {
            for rule in static_data
                .expense_categorization_rules
                .iter()
                .chain(static_data.income_categorization_rules.iter())
            {
                if let Some(rule_category) = static_data.get_category(&rule.category) {
                    let (category, subcategory) = rule_category.inverse_pair();
                    rules.push(CategorizationRuleJson {
                        pattern: rule.pattern.clone(),
                        category,
                        subcategory: if subcategory.is_empty() {
                            None
                        } else {
                            Some(subcategory)
                        },
                    });
                }
            }
        });
        serde_json::to_writer_pretty(file, &rules)?;
        Ok(())
    }
    pub fn add(pattern: &str, category: &Category) -> Result<(), Box<dyn Error>> {
        BUDGET_STATIC_DATA.with_borrow_mut(|data| {
            let rule = CategorizationRule {
                pattern: String::from(pattern),
                category: category.name.clone(),
            };
            if Categories::is_income(category) {
                data.income_categorization_rules.push(rule);
            } else {
                data.expense_categorization_rules.push(rule);
            }
        });
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct CategoryJson {
    pub name: String,
    #[serde(rename = "type")]
    pub category_type: String,
    pub frequency: String,
    pub description: Option<String>,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub subcategories: Vec<SubcategoryJson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct SubcategoryJson {
    pub name: String,
    pub frequency: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CategorizationRuleJson {
    pub pattern: String,
    pub category: String,
    pub subcategory: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CategoryMapJson {
    pub source_category_id: String,
    pub source_category_name: String,
    pub target_category: String,
    pub target_subcategory: String,
}

fn parse_hex_color(value: &str) -> Result<ratatui::style::Color, Box<dyn Error>> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid category color: {value}").into());
    }
    Ok(ratatui::style::Color::Rgb(
        u8::from_str_radix(&hex[0..2], 16)?,
        u8::from_str_radix(&hex[2..4], 16)?,
        u8::from_str_radix(&hex[4..6], 16)?,
    ))
}

fn vary_color_saturation(color: ratatui::style::Color, index: usize) -> ratatui::style::Color {
    let ratatui::style::Color::Rgb(red, green, blue) = color else {
        return color;
    };
    let red = red as f64 / 255.0;
    let green = green as f64 / 255.0;
    let blue = blue as f64 / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    let value = max;
    let saturation_factor = 0.65 + (index % 4) as f64 * 0.15;
    let saturation = (saturation * saturation_factor).min(1.0);
    let hue = if delta == 0.0 {
        0.0
    } else if max == red {
        ((green - blue) / delta).rem_euclid(6.0)
    } else if max == green {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    } / 6.0;
    let chroma = value * saturation;
    let hue_sector = hue * 6.0;
    let second = chroma * (1.0 - ((hue_sector % 2.0) - 1.0).abs());
    let (red_prime, green_prime, blue_prime) = match hue_sector {
        h if h < 1.0 => (chroma, second, 0.0),
        h if h < 2.0 => (second, chroma, 0.0),
        h if h < 3.0 => (0.0, chroma, second),
        h if h < 4.0 => (0.0, second, chroma),
        h if h < 5.0 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let minimum = value - chroma;
    ratatui::style::Color::Rgb(
        ((red_prime + minimum) * 255.0).round() as u8,
        ((green_prime + minimum) * 255.0).round() as u8,
        ((blue_prime + minimum) * 255.0).round() as u8,
    )
}

pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try RFC3339 / ISO8601 first (handles timezone offsets)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Common datetime formats (with space separator)
    let dt_fmts = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%d/%m/%Y %H:%M:%S",
        "%d/%m/%Y %H:%M",
        "%m/%d/%Y %H:%M:%S",
        "%m/%d/%Y %H:%M",
    ];
    for fmt in &dt_fmts {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(Utc.from_utc_datetime(&naive));
        }
    }

    // Common datetime formats with 'T' separator
    let t_fmts = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
    ];
    for fmt in &t_fmts {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(Utc.from_utc_datetime(&naive));
        }
    }

    // Date-only formats: treat as midnight UTC
    let date_fmts = ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y"];
    for fmt in &date_fmts {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            if let Some(ndt) = date.and_hms_opt(0, 0, 0) {
                return Some(Utc.from_utc_datetime(&ndt));
            }
        }
    }

    None
}

pub fn load_transactions_from_csv(path: &PathBuf) -> Result<Vec<Transaction>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();

    // extract account name from file name (without extension)
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Account")
        .to_string();
    let account_name = file_name.split('_').next().unwrap_or("").to_string();
    // Find column indices by matching common header names (case-insensitive)
    let mut col_indices = std::collections::HashMap::new();
    for (idx, header) in headers.iter().enumerate() {
        let normalized = header.to_lowercase();
        if normalized.contains("date") {
            col_indices.insert("date", idx);
        } else if normalized.contains("account") {
            col_indices.insert("account", idx);
        } else if normalized.contains("amount") {
            col_indices.insert("amount", idx);
        } else if normalized.contains("category") && !normalized.contains("sub") {
            col_indices.insert("category", idx);
        } else if normalized.contains("subcategory") || normalized.contains("sub_category") {
            col_indices.insert("subcategory", idx);
        } else if normalized.contains("description")
            || normalized.contains("desc")
            || normalized.contains("title")
        {
            col_indices.insert("description", idx);
        } else if normalized.contains("merchant") {
            col_indices.insert("merchant", idx);
        } else if normalized.contains("id") {
            col_indices.insert("identifier", idx);
        }
    }

    let mut transactions = Vec::new();

    for result in rdr.records() {
        let record = result?;

        let date_s = col_indices
            .get("date")
            .and_then(|&i| record.get(i))
            .unwrap_or("");
        let account = col_indices
            .get("account")
            .and_then(|&i| record.get(i))
            .unwrap_or(&account_name);
        let amount_s = col_indices
            .get("amount")
            .and_then(|&i| record.get(i))
            .unwrap_or("0");
        let category = col_indices
            .get("category")
            .and_then(|&i| record.get(i))
            .unwrap_or("");
        let description = col_indices
            .get("description")
            .and_then(|&i| record.get(i))
            .unwrap_or("");
        let merchant = col_indices
            .get("merchant")
            .and_then(|&i| record.get(i))
            .unwrap_or("");
        let identifier = col_indices
            .get("identifier")
            .and_then(|&i| record.get(i))
            .unwrap_or("");

        let amount_f64: f64 = amount_s.parse().unwrap_or(0.0);

        let transaction = Transaction {
            db_id: None,
            date: parse_date(date_s).ok_or_else(|| format!("Unable to parse date: {}", date_s))?,
            account_id: account.to_string(),
            identifier: identifier.to_string(),
            amount_in_cents: (amount_f64 * 100.0).round() as i64,
            category: {
                BUDGET_STATIC_DATA.with_borrow(|static_data| {
                    static_data
                        .get_category(category)
                        .unwrap_or(Category::uncategorized())
                })
            },
            merchant: merchant.to_string(),
            description: description.to_string(),
        };
        transactions.push(transaction);
    }
    Ok(transactions)
}
