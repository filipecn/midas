use std::collections::{BTreeMap, BTreeSet};

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fundamentus::{StockDetails, StockSummary};

const WEIGHT_TOLERANCE: f64 = 1e-8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Value,
    Quality,
    Growth,
    FinancialHealth,
    Dividends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Indicator {
    PriceEarnings,
    PriceBook,
    EvEbitda,
    Roic,
    Roe,
    EbitMargin,
    NetMargin,
    NetDebtToEquity,
    DividendYield,
    RevenueGrowth,
    EarningsGrowth,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HigherIsBetter,
    LowerIsBetter,
    Target { value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndicatorConfig {
    pub category: Category,
    pub weight: f64,
    pub direction: Direction,
}

impl Default for IndicatorConfig {
    fn default() -> Self {
        Self {
            category: Category::Value,
            weight: 0.0,
            direction: Direction::HigherIsBetter,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QarpConfig {
    pub category_weights: BTreeMap<Category, f64>,
    pub indicators: BTreeMap<Indicator, IndicatorConfig>,
    pub as_of_date: NaiveDate,
    pub minimum_overall_coverage: f64,
    pub minimum_category_coverage: f64,
    pub minimum_universe_size: usize,
    pub require_positive_earnings: bool,
    pub mandatory_indicators: Vec<Indicator>,
    pub max_quote_age_days: Option<i64>,
    pub require_quote_date: bool,
    pub sector_relative: bool,
    pub winsorize_percentiles: Option<(f64, f64)>,
    pub sector_excluded_indicators: BTreeMap<String, Vec<Indicator>>,
}

impl Default for QarpConfig {
    fn default() -> Self {
        use Category::*;
        use Direction::*;
        use Indicator::*;

        Self {
            category_weights: BTreeMap::from([
                (Value, 0.30),
                (Quality, 0.30),
                (Growth, 0.20),
                (FinancialHealth, 0.15),
                (Dividends, 0.05),
            ]),
            indicators: BTreeMap::from([
                (PriceEarnings, indicator(Value, 1.0 / 3.0, LowerIsBetter)),
                (EvEbitda, indicator(Value, 1.0 / 3.0, LowerIsBetter)),
                (PriceBook, indicator(Value, 1.0 / 3.0, LowerIsBetter)),
                (Roic, indicator(Quality, 0.25, HigherIsBetter)),
                (Roe, indicator(Quality, 0.25, HigherIsBetter)),
                (EbitMargin, indicator(Quality, 0.25, HigherIsBetter)),
                (NetMargin, indicator(Quality, 0.25, HigherIsBetter)),
                (RevenueGrowth, indicator(Growth, 0.5, HigherIsBetter)),
                (EarningsGrowth, indicator(Growth, 0.5, HigherIsBetter)),
                (
                    NetDebtToEquity,
                    indicator(FinancialHealth, 1.0, LowerIsBetter),
                ),
                (DividendYield, indicator(Dividends, 1.0, HigherIsBetter)),
            ]),
            as_of_date: Utc::now().date_naive(),
            minimum_overall_coverage: 0.50,
            minimum_category_coverage: 0.50,
            minimum_universe_size: 5,
            require_positive_earnings: true,
            mandatory_indicators: vec![PriceEarnings],
            max_quote_age_days: Some(90),
            require_quote_date: false,
            sector_relative: false,
            winsorize_percentiles: None,
            sector_excluded_indicators: BTreeMap::new(),
        }
    }
}

fn indicator(category: Category, weight: f64, direction: Direction) -> IndicatorConfig {
    IndicatorConfig {
        category,
        weight,
        direction,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StockFundamentals {
    pub ticker: String,
    pub sector: Option<String>,
    pub quote_date: Option<NaiveDate>,
    pub price_earnings: Option<f64>,
    pub price_book: Option<f64>,
    pub ev_ebitda: Option<f64>,
    pub roic: Option<f64>,
    pub roe: Option<f64>,
    pub ebit_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub net_debt_to_equity: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub revenue_growth: Option<f64>,
    pub earnings_growth: Option<f64>,
}

impl From<&StockDetails> for StockFundamentals {
    fn from(stock: &StockDetails) -> Self {
        let net_debt = parse_fundamentus_number(&stock.div_liquida);
        let equity = parse_fundamentus_number(&stock.patrim_liquido);
        Self {
            ticker: stock.ticker.trim().to_uppercase(),
            sector: nonempty(&stock.setor),
            quote_date: parse_quote_date(&stock.data_cot),
            price_earnings: parse_fundamentus_number(&stock.p_l),
            price_book: positive(parse_fundamentus_number(&stock.p_vp)),
            ev_ebitda: positive(parse_fundamentus_number(&stock.ev_ebitda)),
            roic: parse_fundamentus_number(&stock.roic),
            roe: parse_fundamentus_number(&stock.roe),
            ebit_margin: parse_fundamentus_number(&stock.marg_ebit),
            net_margin: parse_fundamentus_number(&stock.marg_liquida),
            net_debt_to_equity: match (net_debt, positive(equity)) {
                (Some(debt), Some(equity)) => finite(debt / equity),
                _ => None,
            },
            dividend_yield: nonnegative(parse_fundamentus_number(&stock.div_yield)),
            revenue_growth: parse_fundamentus_number(&stock.cres_rec_5a),
            earnings_growth: None,
        }
    }
}

impl From<&StockSummary> for StockFundamentals {
    fn from(stock: &StockSummary) -> Self {
        Self {
            ticker: stock.ticker.trim().to_uppercase(),
            price_earnings: parse_fundamentus_number(&stock.p_l),
            price_book: positive(parse_fundamentus_number(&stock.p_vp)),
            dividend_yield: nonnegative(parse_fundamentus_number(&stock.div_yield)),
            roe: parse_fundamentus_number(&stock.roe),
            ..Self::default()
        }
    }
}

impl StockFundamentals {
    pub fn value(&self, indicator: Indicator) -> Option<f64> {
        let value = match indicator {
            Indicator::PriceEarnings => positive(self.price_earnings),
            Indicator::PriceBook => self.price_book,
            Indicator::EvEbitda => self.ev_ebitda,
            Indicator::Roic => self.roic,
            Indicator::Roe => self.roe,
            Indicator::EbitMargin => self.ebit_margin,
            Indicator::NetMargin => self.net_margin,
            Indicator::NetDebtToEquity => self.net_debt_to_equity,
            Indicator::DividendYield => self.dividend_yield,
            Indicator::RevenueGrowth => self.revenue_growth,
            Indicator::EarningsGrowth => self.earnings_growth,
        };
        value.and_then(finite)
    }
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "-").then(|| value.to_string())
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn positive(value: Option<f64>) -> Option<f64> {
    value.filter(|value| *value > 0.0)
}

fn nonnegative(value: Option<f64>) -> Option<f64> {
    value.filter(|value| *value >= 0.0)
}

pub fn parse_fundamentus_number(raw: &str) -> Option<f64> {
    let mut value = raw
        .trim()
        .replace("R$", "")
        .replace('%', "")
        .replace(char::is_whitespace, "");
    if value.is_empty() || value == "-" || value == "--" {
        return None;
    }

    let negative_parentheses = value.starts_with('(') && value.ends_with(')');
    if negative_parentheses {
        value = value[1..value.len() - 1].to_string();
    }

    if value.contains(',') {
        value = value.replace('.', "").replace(',', ".");
    } else {
        value = value.replace('.', "");
    }
    let parsed = value.parse::<f64>().ok()?;
    finite(if negative_parentheses {
        -parsed
    } else {
        parsed
    })
}

fn parse_quote_date(raw: &str) -> Option<NaiveDate> {
    let value = raw.trim();
    NaiveDate::parse_from_str(value, "%d/%m/%Y")
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .ok()
}

pub trait ScoringStrategy {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn config(&self) -> &QarpConfig;

    fn additional_exclusion_reasons(&self, _stock: &StockFundamentals) -> Vec<ExclusionReason> {
        Vec::new()
    }
}

pub struct QarpStrategy {
    config: QarpConfig,
}

impl QarpStrategy {
    pub fn new(config: QarpConfig) -> Result<Self, ScoringError> {
        validate_config(&config)?;
        Ok(Self { config })
    }
}

impl Default for QarpStrategy {
    fn default() -> Self {
        Self::new(QarpConfig::default()).expect("built-in QARP configuration is valid")
    }
}

impl ScoringStrategy for QarpStrategy {
    fn name(&self) -> &str {
        "QARP"
    }

    fn version(&self) -> &str {
        "1"
    }

    fn config(&self) -> &QarpConfig {
        &self.config
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityResult {
    pub eligible: bool,
    pub overall_coverage: f64,
    pub category_coverage: BTreeMap<Category, f64>,
    pub exclusion_reasons: Vec<ExclusionReason>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    MissingMandatoryIndicator(Indicator),
    NegativeEarnings,
    InsufficientOverallCoverage,
    InsufficientCategoryCoverage(Category),
    StaleQuote,
    MissingQuoteDate,
    FutureQuoteDate,
    StrategySpecific(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorScore {
    pub value: Option<f64>,
    pub percentile_score: Option<f64>,
    pub configured_weight: f64,
    pub contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockScore {
    pub ticker: String,
    pub strategy: String,
    pub strategy_version: String,
    pub total_score: f64,
    pub category_scores: BTreeMap<Category, Option<f64>>,
    pub configured_category_weights: BTreeMap<Category, f64>,
    pub effective_category_weights: BTreeMap<Category, f64>,
    pub indicators: BTreeMap<Indicator, IndicatorScore>,
    pub data_coverage: f64,
    pub quote_date: Option<NaiveDate>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockEvaluation {
    pub ticker: String,
    pub rank: Option<usize>,
    pub eligibility: EligibilityResult,
    pub score: Option<StockScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringReport {
    pub strategy: String,
    pub strategy_version: String,
    pub configuration_id: String,
    pub as_of_date: NaiveDate,
    pub eligible: Vec<StockEvaluation>,
    pub ineligible: Vec<StockEvaluation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ScoringError {
    #[error("invalid scoring configuration: {0}")]
    InvalidConfiguration(String),
    #[error("stock ticker must not be empty")]
    EmptyTicker,
    #[error("duplicate stock ticker: {0}")]
    DuplicateTicker(String),
    #[error("failed to serialize scoring configuration: {0}")]
    ConfigurationSerialization(#[from] serde_json::Error),
}

pub fn validate_config(config: &QarpConfig) -> Result<(), ScoringError> {
    let category_total: f64 = config.category_weights.values().sum();
    if config.category_weights.is_empty()
        || config
            .category_weights
            .values()
            .any(|weight| !valid_weight(*weight))
        || (category_total - 1.0).abs() > WEIGHT_TOLERANCE
    {
        return Err(ScoringError::InvalidConfiguration(
            "category weights must be nonnegative and sum to 1.0".to_string(),
        ));
    }

    let mut indicator_totals = BTreeMap::<Category, f64>::new();
    for (indicator, settings) in &config.indicators {
        if !valid_weight(settings.weight) {
            return Err(ScoringError::InvalidConfiguration(format!(
                "indicator {indicator:?} has an invalid weight"
            )));
        }
        if !config.category_weights.contains_key(&settings.category) {
            return Err(ScoringError::InvalidConfiguration(format!(
                "indicator {indicator:?} references an unweighted category"
            )));
        }
        if let Direction::Target { value } = settings.direction
            && !value.is_finite()
        {
            return Err(ScoringError::InvalidConfiguration(format!(
                "indicator {indicator:?} has a non-finite target"
            )));
        }
        *indicator_totals.entry(settings.category).or_default() += settings.weight;
    }
    for category in config.category_weights.keys() {
        let Some(total) = indicator_totals.get(category) else {
            continue;
        };
        if (*total - 1.0).abs() > WEIGHT_TOLERANCE {
            return Err(ScoringError::InvalidConfiguration(format!(
                "indicator weights in {category:?} must sum to 1.0"
            )));
        }
    }

    if !valid_weight(config.minimum_overall_coverage)
        || !valid_weight(config.minimum_category_coverage)
    {
        return Err(ScoringError::InvalidConfiguration(
            "coverage thresholds must be between 0 and 1".to_string(),
        ));
    }
    if config.max_quote_age_days.is_some_and(|days| days < 0) {
        return Err(ScoringError::InvalidConfiguration(
            "maximum quote age must not be negative".to_string(),
        ));
    }
    if let Some((lower, upper)) = config.winsorize_percentiles
        && (!valid_weight(lower) || !valid_weight(upper) || lower >= upper)
    {
        return Err(ScoringError::InvalidConfiguration(
            "winsorization percentiles must satisfy 0 <= lower < upper <= 1".to_string(),
        ));
    }
    for mandatory in &config.mandatory_indicators {
        if !config.indicators.contains_key(mandatory) {
            return Err(ScoringError::InvalidConfiguration(format!(
                "mandatory indicator {mandatory:?} is not configured"
            )));
        }
    }
    Ok(())
}

fn valid_weight(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub fn run_strategy<S: ScoringStrategy>(
    strategy: &S,
    stocks: &[StockFundamentals],
) -> Result<ScoringReport, ScoringError> {
    let config = strategy.config();
    validate_config(config)?;

    let mut tickers = BTreeSet::new();
    for stock in stocks {
        let ticker = stock.ticker.trim().to_uppercase();
        if ticker.is_empty() {
            return Err(ScoringError::EmptyTicker);
        }
        if !tickers.insert(ticker.clone()) {
            return Err(ScoringError::DuplicateTicker(ticker));
        }
    }

    let universe_active_categories = active_categories(config, stocks);
    let mut eligible = Vec::new();
    let mut ineligible = Vec::new();
    for stock in stocks {
        let eligibility = evaluate_eligibility(strategy, stock, &universe_active_categories);
        let evaluation = StockEvaluation {
            ticker: stock.ticker.clone(),
            rank: None,
            eligibility: eligibility.clone(),
            score: None,
        };
        if eligibility.eligible {
            eligible.push((stock, evaluation));
        } else {
            ineligible.push(evaluation);
        }
    }

    let mut scored = Vec::with_capacity(eligible.len());
    let eligible_universe = eligible
        .iter()
        .map(|(stock, _)| (*stock).clone())
        .collect::<Vec<_>>();
    let scoring_active_categories = active_categories(config, &eligible_universe);
    for (stock, mut evaluation) in eligible {
        let score = score_eligible(
            strategy,
            stock,
            &eligible_universe,
            &scoring_active_categories,
            &evaluation.eligibility,
        );
        evaluation.score = Some(score);
        scored.push(evaluation);
    }
    scored.sort_by(|left, right| {
        right
            .score
            .as_ref()
            .map(|score| score.total_score)
            .unwrap_or_default()
            .total_cmp(
                &left
                    .score
                    .as_ref()
                    .map(|score| score.total_score)
                    .unwrap_or_default(),
            )
            .then_with(|| left.ticker.cmp(&right.ticker))
    });
    for (rank, evaluation) in scored.iter_mut().enumerate() {
        evaluation.rank = Some(rank + 1);
    }
    ineligible.sort_by(|left, right| left.ticker.cmp(&right.ticker));

    let mut warnings = Vec::new();
    if stocks.is_empty() {
        warnings.push("The scoring universe is empty.".to_string());
    }
    if scored.len() < config.minimum_universe_size {
        warnings.push(format!(
            "Only {} eligible stocks; configured minimum for meaningful percentiles is {}.",
            scored.len(),
            config.minimum_universe_size
        ));
    }
    let growth_indicators = config
        .indicators
        .keys()
        .filter(|indicator| {
            matches!(
                indicator,
                Indicator::RevenueGrowth | Indicator::EarningsGrowth
            )
        })
        .collect::<Vec<_>>();
    if !growth_indicators.is_empty() {
        if stocks.iter().any(|stock| stock.revenue_growth.is_some()) {
            if growth_indicators.contains(&&Indicator::EarningsGrowth) {
                warnings.push(
                    "Fundamentus provides five-year revenue growth, but not earnings growth."
                        .to_string(),
                );
            }
        } else {
            warnings.push(
                "Five-year revenue growth is missing from saved snapshots; refresh Fundamentus details to retrieve it. Earnings growth is not provided."
                    .to_string(),
            );
        }
    }
    if stocks.iter().any(|stock| stock.quote_date.is_none()) {
        warnings.push(
            "Fundamentus supplies a quote date, not the financial-statement publication date."
                .to_string(),
        );
    }
    let serialized_config = serde_json::to_string(config)?;
    let configuration_hash = serialized_config
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    Ok(ScoringReport {
        strategy: strategy.name().to_string(),
        strategy_version: strategy.version().to_string(),
        configuration_id: format!("{configuration_hash:016x}"),
        as_of_date: config.as_of_date,
        eligible: scored,
        ineligible,
        warnings,
    })
}

fn active_categories(config: &QarpConfig, stocks: &[StockFundamentals]) -> BTreeSet<Category> {
    config
        .category_weights
        .keys()
        .copied()
        .filter(|category| {
            config.indicators.iter().any(|(indicator, settings)| {
                settings.category == *category
                    && stocks.iter().any(|stock| {
                        !is_excluded_for_sector(config, stock, *indicator)
                            && stock.value(*indicator).is_some()
                    })
            })
        })
        .collect()
}

fn evaluate_eligibility<S: ScoringStrategy>(
    strategy: &S,
    stock: &StockFundamentals,
    active_categories: &BTreeSet<Category>,
) -> EligibilityResult {
    let config = strategy.config();
    let enabled_indicators = config
        .indicators
        .iter()
        .filter(|(indicator, settings)| {
            active_categories.contains(&settings.category)
                && !is_excluded_for_sector(config, stock, **indicator)
        })
        .collect::<Vec<_>>();
    let enabled_weight: f64 = enabled_indicators
        .iter()
        .map(|(_, settings)| config.category_weights[&settings.category] * settings.weight)
        .sum();
    let available_weight: f64 = enabled_indicators
        .iter()
        .filter(|(indicator, _)| stock.value(**indicator).is_some())
        .map(|(_, settings)| config.category_weights[&settings.category] * settings.weight)
        .sum();
    let overall_coverage = if enabled_weight > 0.0 {
        available_weight / enabled_weight
    } else {
        0.0
    };

    let mut category_coverage = BTreeMap::new();
    let mut reasons = Vec::new();
    for category in active_categories {
        let applicable = enabled_indicators
            .iter()
            .filter(|(_, settings)| settings.category == *category)
            .collect::<Vec<_>>();
        let category_weight: f64 = applicable.iter().map(|(_, settings)| settings.weight).sum();
        if category_weight == 0.0 {
            continue;
        }
        let covered: f64 = applicable
            .iter()
            .filter(|(indicator, _)| stock.value(**indicator).is_some())
            .map(|(_, settings)| settings.weight)
            .sum();
        let coverage = covered / category_weight;
        category_coverage.insert(*category, coverage);
        if coverage + WEIGHT_TOLERANCE < config.minimum_category_coverage {
            reasons.push(ExclusionReason::InsufficientCategoryCoverage(*category));
        }
    }

    for indicator in &config.mandatory_indicators {
        let negative_earnings = *indicator == Indicator::PriceEarnings
            && stock.price_earnings.is_some_and(|value| value <= 0.0);
        if (stock.value(*indicator).is_none() && !negative_earnings)
            || is_excluded_for_sector(config, stock, *indicator)
        {
            reasons.push(ExclusionReason::MissingMandatoryIndicator(*indicator));
        }
    }
    if config.require_positive_earnings
        && config.indicators.contains_key(&Indicator::PriceEarnings)
        && stock.price_earnings.is_none_or(|value| value <= 0.0)
    {
        let reason = if stock.price_earnings.is_some_and(|value| value <= 0.0) {
            ExclusionReason::NegativeEarnings
        } else {
            ExclusionReason::MissingMandatoryIndicator(Indicator::PriceEarnings)
        };
        reasons.push(reason);
    }
    if overall_coverage + WEIGHT_TOLERANCE < config.minimum_overall_coverage {
        reasons.push(ExclusionReason::InsufficientOverallCoverage);
    }

    let mut warnings = Vec::new();
    match stock.quote_date {
        None if config.require_quote_date => reasons.push(ExclusionReason::MissingQuoteDate),
        None => warnings.push("Quote date is missing or unparseable.".to_string()),
        Some(date) => {
            let today = config.as_of_date;
            if date > today {
                reasons.push(ExclusionReason::FutureQuoteDate);
            } else if config
                .max_quote_age_days
                .is_some_and(|max_age| (today - date).num_days() > max_age)
            {
                reasons.push(ExclusionReason::StaleQuote);
            }
        }
    }
    reasons.extend(strategy.additional_exclusion_reasons(stock));

    if category_coverage.values().any(|coverage| *coverage < 1.0) {
        warnings.push(
            "Missing indicators are not imputed or renormalized within a category; configured indicator weights remain in force."
                .to_string(),
        );
    }

    EligibilityResult {
        eligible: reasons.is_empty(),
        overall_coverage,
        category_coverage,
        exclusion_reasons: reasons,
        warnings,
    }
}

fn score_eligible<S: ScoringStrategy>(
    strategy: &S,
    stock: &StockFundamentals,
    universe: &[StockFundamentals],
    active_categories: &BTreeSet<Category>,
    eligibility: &EligibilityResult,
) -> StockScore {
    let config = strategy.config();
    let mut distributions = BTreeMap::<Indicator, Vec<(f64, Option<&str>)>>::new();
    for indicator in config.indicators.keys() {
        if !active_categories.contains(&config.indicators[indicator].category) {
            continue;
        }
        let values = universe
            .iter()
            .filter(|candidate| {
                !is_excluded_for_sector(config, candidate, *indicator)
                    && (!config.sector_relative
                        || same_sector(stock.sector.as_deref(), candidate.sector.as_deref()))
            })
            .filter_map(|candidate| {
                candidate
                    .value(*indicator)
                    .map(|value| (value, candidate.sector.as_deref()))
            })
            .collect();
        distributions.insert(*indicator, values);
    }

    let total_active_category_weight: f64 = active_categories
        .iter()
        .map(|category| config.category_weights[category])
        .sum();
    let effective_category_weights = active_categories
        .iter()
        .map(|category| {
            (
                *category,
                config.category_weights[category] / total_active_category_weight,
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut category_scores = BTreeMap::new();
    let mut indicator_scores = BTreeMap::new();
    let mut total_score = 0.0;
    for category in config.category_weights.keys() {
        let mut category_score = 0.0;
        let mut category_has_score = false;
        for (indicator, settings) in &config.indicators {
            if settings.category != *category {
                continue;
            }
            let value = if is_excluded_for_sector(config, stock, *indicator) {
                None
            } else {
                stock.value(*indicator)
            };
            let percentile_score = value.and_then(|candidate| {
                let distribution = distributions.get(indicator)?;
                let sector_values = distribution
                    .iter()
                    .filter(|(_, sector)| {
                        !config.sector_relative || same_sector(stock.sector.as_deref(), *sector)
                    })
                    .map(|(value, _)| *value)
                    .collect::<Vec<_>>();
                Some(normalize(
                    candidate,
                    &sector_values,
                    settings.direction,
                    config.winsorize_percentiles,
                ))
            });
            let contribution = percentile_score.unwrap_or_default() * settings.weight;
            if percentile_score.is_some() {
                category_has_score = true;
                category_score += contribution;
            }
            indicator_scores.insert(
                *indicator,
                IndicatorScore {
                    value,
                    percentile_score,
                    configured_weight: settings.weight,
                    contribution: contribution
                        * effective_category_weights
                            .get(category)
                            .copied()
                            .unwrap_or_default(),
                },
            );
        }
        if category_has_score && active_categories.contains(category) {
            let effective_weight = effective_category_weights[category];
            total_score += category_score * effective_weight;
            category_scores.insert(*category, Some(round_to(category_score, 2)));
        } else {
            category_scores.insert(*category, None);
        }
    }

    let mut warnings = eligibility.warnings.clone();
    if config.sector_relative && stock.sector.is_none() {
        warnings.push(
            "Sector is unavailable; this stock uses the full eligible universe for normalization."
                .to_string(),
        );
    }
    StockScore {
        ticker: stock.ticker.clone(),
        strategy: strategy.name().to_string(),
        strategy_version: strategy.version().to_string(),
        total_score: round_to(total_score.clamp(0.0, 100.0), 2),
        category_scores,
        configured_category_weights: config.category_weights.clone(),
        effective_category_weights,
        indicators: indicator_scores,
        data_coverage: eligibility.overall_coverage,
        quote_date: stock.quote_date,
        warnings,
    }
}

fn round_to(value: f64, decimal_places: u32) -> f64 {
    let factor = 10_f64.powi(decimal_places as i32);
    (value * factor).round() / factor
}

fn is_excluded_for_sector(
    config: &QarpConfig,
    stock: &StockFundamentals,
    indicator: Indicator,
) -> bool {
    let Some(sector) = stock.sector.as_deref() else {
        return false;
    };
    config
        .sector_excluded_indicators
        .iter()
        .find(|(configured_sector, _)| configured_sector.eq_ignore_ascii_case(sector.trim()))
        .is_some_and(|(_, indicators)| indicators.contains(&indicator))
}

fn same_sector(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.trim().eq_ignore_ascii_case(right.trim()),
        (None, _) => true,
        (Some(_), None) => false,
    }
}

fn normalize(
    value: f64,
    values: &[f64],
    direction: Direction,
    winsorize: Option<(f64, f64)>,
) -> f64 {
    if values.len() <= 1 {
        return 50.0;
    }
    let transformed = |value: f64| match direction {
        Direction::HigherIsBetter => value,
        Direction::LowerIsBetter => -value,
        Direction::Target { value: target } => -(value - target).abs(),
    };
    let mut observations = values
        .iter()
        .map(|value| transformed(*value))
        .collect::<Vec<_>>();
    observations.sort_by(f64::total_cmp);
    let candidate = transformed(value);
    let candidate = winsorize
        .map(|(lower, upper)| {
            candidate.clamp(
                quantile(&observations, lower),
                quantile(&observations, upper),
            )
        })
        .unwrap_or(candidate);
    let first = observations.partition_point(|observation| *observation < candidate);
    let after_last = observations.partition_point(|observation| *observation <= candidate);
    let midrank = (first + after_last - 1) as f64 / 2.0;
    (100.0 * midrank / (observations.len() - 1) as f64).clamp(0.0, 100.0)
}

fn quantile(sorted: &[f64], percentile: f64) -> f64 {
    let position = percentile * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(ticker: &str, pe: f64, roic: f64) -> StockFundamentals {
        StockFundamentals {
            ticker: ticker.to_string(),
            quote_date: Some(Utc::now().date_naive()),
            price_earnings: Some(pe),
            price_book: Some(2.0),
            ev_ebitda: Some(8.0),
            roic: Some(roic),
            roe: Some(12.0),
            ebit_margin: Some(15.0),
            net_margin: Some(8.0),
            net_debt_to_equity: Some(0.5),
            dividend_yield: Some(4.0),
            ..StockFundamentals::default()
        }
    }

    #[test]
    fn brazilian_numbers_parse_and_invalid_values_stay_missing() {
        assert_eq!(parse_fundamentus_number("1.234,56%"), Some(1234.56));
        assert_eq!(parse_fundamentus_number("1.234"), Some(1234.0));
        assert_eq!(parse_fundamentus_number("(2.345,67)"), Some(-2345.67));
        assert_eq!(parse_fundamentus_number("-"), None);
        assert_eq!(parse_fundamentus_number("NaN"), None);
        assert_eq!(parse_fundamentus_number("inf"), None);
    }

    #[test]
    fn snapshot_conversion_keeps_unavailable_and_zero_denominator_metrics_missing() {
        let details = StockDetails {
            ticker: "TEST3".to_string(),
            p_l: "8,50".to_string(),
            p_vp: "-".to_string(),
            div_liquida: "100.000".to_string(),
            patrim_liquido: "0".to_string(),
            ..StockDetails::default()
        };
        let stock = StockFundamentals::from(&details);
        assert_eq!(stock.price_earnings, Some(8.5));
        assert_eq!(stock.value(Indicator::PriceBook), None);
        assert_eq!(stock.net_debt_to_equity, None);
        assert_eq!(stock.value(Indicator::RevenueGrowth), None);
    }

    #[test]
    fn snapshot_conversion_reads_fundamentus_five_year_revenue_growth() {
        let details = StockDetails {
            ticker: "TEST3".to_string(),
            cres_rec_5a: "-2,3%".to_string(),
            ..StockDetails::default()
        };
        let stock = StockFundamentals::from(&details);

        assert_eq!(stock.value(Indicator::RevenueGrowth), Some(-2.3));
        assert_eq!(stock.value(Indicator::EarningsGrowth), None);
    }

    #[test]
    fn growth_warning_distinguishes_unavailable_earnings_growth() {
        let stocks = (0..5)
            .map(|index| {
                let mut stock = fixture(&format!("STOCK{index}"), 10.0, 10.0);
                stock.revenue_growth = Some(index as f64);
                stock
            })
            .collect::<Vec<_>>();

        let report = run_strategy(&QarpStrategy::default(), &stocks).unwrap();

        assert!(report.warnings.iter().any(|warning| {
            warning == "Fundamentus provides five-year revenue growth, but not earnings growth."
        }));
    }

    #[test]
    fn percentile_uses_midrank_for_ties_and_respects_direction() {
        assert_eq!(
            normalize(2.0, &[1.0, 2.0, 2.0, 3.0], Direction::HigherIsBetter, None),
            50.0
        );
        assert_eq!(
            normalize(1.0, &[1.0, 2.0, 3.0], Direction::LowerIsBetter, None),
            100.0
        );
        assert_eq!(
            normalize(3.0, &[1.0, 2.0, 3.0], Direction::HigherIsBetter, None),
            100.0
        );
        assert_eq!(
            normalize(7.0, &[7.0], Direction::HigherIsBetter, None),
            50.0
        );
        assert_eq!(
            normalize(
                10.0,
                &[1.0, 10.0, 100.0],
                Direction::Target { value: 10.0 },
                None
            ),
            100.0
        );
        assert_eq!(
            normalize(
                1000.0,
                &[1.0, 10.0, 100.0],
                Direction::HigherIsBetter,
                Some((0.0, 0.5))
            ),
            50.0
        );
    }

    #[test]
    fn weights_are_validated_with_tolerance() {
        let mut config = QarpConfig::default();
        assert!(validate_config(&config).is_ok());
        config.category_weights.insert(Category::Quality, 0.31);
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn negative_earnings_do_not_enter_value_rankings() {
        let negative = fixture("NEG", -4.0, 15.0);
        let report = run_strategy(
            &QarpStrategy::default(),
            &[negative.clone(), fixture("POS", 12.0, 10.0)],
        )
        .unwrap();
        assert!(report.eligible.iter().all(|stock| stock.ticker != "NEG"));
        assert!(report.ineligible.iter().any(|stock| {
            stock.ticker == "NEG"
                && stock.score.is_none()
                && stock
                    .eligibility
                    .exclusion_reasons
                    .contains(&ExclusionReason::NegativeEarnings)
        }));
        assert_eq!(negative.price_earnings, Some(-4.0));
    }

    #[test]
    fn mandatory_missing_and_insufficient_coverage_are_excluded() {
        let mut missing_mandatory = fixture("MISS", 10.0, 10.0);
        missing_mandatory.price_earnings = None;
        let mut low_coverage = fixture("LOW", 10.0, 10.0);
        low_coverage.price_book = None;
        low_coverage.ev_ebitda = None;
        low_coverage.roic = None;
        low_coverage.roe = None;
        low_coverage.ebit_margin = None;
        low_coverage.net_margin = None;
        low_coverage.net_debt_to_equity = None;
        low_coverage.dividend_yield = None;
        let report = run_strategy(
            &QarpStrategy::default(),
            &[
                missing_mandatory,
                low_coverage.clone(),
                fixture("OK", 15.0, 10.0),
            ],
        )
        .unwrap();
        assert!(report.eligible.iter().all(|stock| stock.ticker != "MISS"));
        assert!(report.eligible.iter().all(|stock| stock.ticker != "LOW"));
        assert!(report.ineligible.iter().all(|stock| stock.score.is_none()));
        assert!(low_coverage.revenue_growth.is_none());
    }

    #[test]
    fn missing_indicator_is_omitted_not_ranked_as_zero() {
        let mut missing = fixture("MISS", 10.0, 10.0);
        missing.ev_ebitda = None;
        let report = run_strategy(
            &QarpStrategy::default(),
            &[fixture("A", 10.0, 10.0), fixture("B", 20.0, 20.0), missing],
        )
        .unwrap();
        let scored_missing = report
            .eligible
            .iter()
            .find(|stock| stock.ticker == "MISS")
            .and_then(|stock| stock.score.as_ref())
            .unwrap();
        let scored_complete = report
            .eligible
            .iter()
            .find(|stock| stock.ticker == "A")
            .and_then(|stock| stock.score.as_ref())
            .unwrap();
        let ev_ebitda = &scored_missing.indicators[&Indicator::EvEbitda];
        assert_eq!(ev_ebitda.value, None);
        assert_eq!(ev_ebitda.percentile_score, None);
        assert_eq!(ev_ebitda.contribution, 0.0);
        assert!(
            scored_missing.category_scores[&Category::Value].unwrap()
                < scored_complete.category_scores[&Category::Value].unwrap()
        );
    }

    #[test]
    fn report_is_deterministic_and_scores_only_eligible_stocks() {
        let stocks = vec![fixture("B", 20.0, 10.0), fixture("A", 10.0, 20.0)];
        let first =
            serde_json::to_string(&run_strategy(&QarpStrategy::default(), &stocks).unwrap())
                .unwrap();
        let second =
            serde_json::to_string(&run_strategy(&QarpStrategy::default(), &stocks).unwrap())
                .unwrap();
        assert_eq!(first, second);
    }

    struct ExcludingStrategy {
        config: QarpConfig,
    }

    impl ScoringStrategy for ExcludingStrategy {
        fn name(&self) -> &str {
            "Test strategy"
        }

        fn version(&self) -> &str {
            "1"
        }

        fn config(&self) -> &QarpConfig {
            &self.config
        }

        fn additional_exclusion_reasons(&self, stock: &StockFundamentals) -> Vec<ExclusionReason> {
            if stock.ticker == "A" {
                vec![ExclusionReason::StrategySpecific(
                    "excluded only by test strategy".to_string(),
                )]
            } else {
                Vec::new()
            }
        }
    }

    #[test]
    fn eligibility_is_strategy_specific() {
        let stock = fixture("A", 10.0, 10.0);
        let qarp = run_strategy(&QarpStrategy::default(), std::slice::from_ref(&stock)).unwrap();
        let other_strategy = ExcludingStrategy {
            config: QarpConfig::default(),
        };
        let other = run_strategy(&other_strategy, &[stock]).unwrap();
        assert_eq!(qarp.eligible.len(), 1);
        assert_eq!(other.ineligible.len(), 1);
        assert_eq!(
            other.ineligible[0].eligibility.exclusion_reasons,
            vec![ExclusionReason::StrategySpecific(
                "excluded only by test strategy".to_string()
            )]
        );
    }

    #[test]
    fn stale_quotes_are_excluded_against_configured_evaluation_date() {
        let mut config = QarpConfig::default();
        config.as_of_date = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
        config.max_quote_age_days = Some(90);
        let strategy = QarpStrategy::new(config).unwrap();
        let mut stock = fixture("OLD", 10.0, 10.0);
        stock.quote_date = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        let report = run_strategy(&strategy, &[stock]).unwrap();
        assert!(
            report.ineligible[0]
                .eligibility
                .exclusion_reasons
                .contains(&ExclusionReason::StaleQuote)
        );
        assert_eq!(
            report.as_of_date,
            NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
        );
    }

    #[test]
    fn sector_exclusions_and_config_defaults_are_supported() {
        let config: QarpConfig = serde_json::from_str(r#"{"sector_relative":true}"#).unwrap();
        assert!(config.sector_relative);
        assert_eq!(config.minimum_overall_coverage, 0.5);

        let mut excluded = fixture("BANK", 10.0, 10.0);
        excluded.sector = Some("Banks".to_string());
        let mut config = QarpConfig::default();
        config
            .sector_excluded_indicators
            .insert("banks".to_string(), vec![Indicator::NetDebtToEquity]);
        let strategy = QarpStrategy::new(config).unwrap();
        let report = run_strategy(&strategy, &[excluded]).unwrap();
        assert!(report.eligible[0].score.is_some());
        assert_eq!(
            report.eligible[0].score.as_ref().unwrap().indicators[&Indicator::NetDebtToEquity]
                .value,
            None
        );
    }

    #[test]
    fn empty_and_small_universes_are_reported_without_panicking() {
        let strategy = QarpStrategy::default();
        let empty = run_strategy(&strategy, &[]).unwrap();
        assert!(empty.eligible.is_empty());
        let small = run_strategy(&strategy, &[fixture("ONE", 10.0, 10.0)]).unwrap();
        assert_eq!(small.eligible[0].score.as_ref().unwrap().total_score, 50.0);
        assert!(!small.warnings.is_empty());
    }
}
