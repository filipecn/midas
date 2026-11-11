use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_MAX_DATA_AGE_DAYS: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityStatus {
    Eligible,
    Suspended,
    Delisted,
    Unknown,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    Listed,
    Delisted,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradingStatus {
    Normal,
    Suspended,
    Restricted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B3SecurityEvidence {
    pub ticker: String,
    #[serde(default)]
    pub security_id: Option<String>,
    pub listing_status: ListingStatus,
    pub trading_status: TradingStatus,
    pub verified_at: DateTime<Utc>,
    #[serde(default)]
    pub published_at: Option<DateTime<Utc>>,
    pub source: String,
    pub source_url: String,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct B3Snapshot {
    pub schema_version: u32,
    pub retrieved_at: DateTime<Utc>,
    #[serde(default)]
    pub published_at: Option<DateTime<Utc>>,
    pub source: String,
    pub source_url: String,
    pub securities: Vec<B3SecurityEvidence>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFreshness {
    Current,
    Historical,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub ticker: String,
    pub security_id: Option<String>,
    pub status: EligibilityStatus,
    pub last_known_status: Option<EligibilityStatus>,
    pub evidence_freshness: EvidenceFreshness,
    pub reason: String,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub evidence_published_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityCounts {
    pub processed: usize,
    pub eligible: usize,
    pub suspended: usize,
    pub delisted: usize,
    pub unknown: usize,
    pub review_required: usize,
    pub unmatched_fundamentus: usize,
    pub b3_securities_not_in_fundamentus: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityReport {
    pub generated_at: DateTime<Utc>,
    pub max_data_age_days: i64,
    pub snapshot_retrieved_at: Option<DateTime<Utc>>,
    pub snapshot_published_at: Option<DateTime<Utc>>,
    pub snapshot_age_days: Option<i64>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub official_individual_security_status_feed_integrated: bool,
    pub imported_individual_security_status_evidence_complete: bool,
    pub counts: EligibilityCounts,
    pub securities: Vec<SecurityFinding>,
    pub unmatched_fundamentus_tickers: Vec<String>,
    pub b3_securities_not_in_fundamentus: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum EligibilityError {
    #[error("unsupported B3 snapshot schema version {0}")]
    UnsupportedSchema(u32),
    #[error("B3 snapshot has an empty source or source URL")]
    MissingSource,
    #[error("B3 snapshot has no security records")]
    EmptySnapshot,
    #[error("B3 snapshot contains an empty ticker")]
    EmptyTicker,
    #[error("B3 snapshot contains a future retrieval or verification timestamp")]
    FutureTimestamp,
    #[error("B3 snapshot evidence URLs must use HTTPS on a B3 domain")]
    UnsupportedSourceUrl,
}

pub fn parse_snapshot(json: &str) -> Result<B3Snapshot, Box<dyn std::error::Error>> {
    let snapshot: B3Snapshot = serde_json::from_str(json)?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn validate_snapshot(snapshot: &B3Snapshot) -> Result<(), EligibilityError> {
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(EligibilityError::UnsupportedSchema(snapshot.schema_version));
    }
    if snapshot.source.trim().is_empty()
        || snapshot.source_url.trim().is_empty()
        || snapshot.securities.iter().any(|security| {
            security.source.trim().is_empty() || security.source_url.trim().is_empty()
        })
    {
        return Err(EligibilityError::MissingSource);
    }
    if snapshot.securities.is_empty() {
        return Err(EligibilityError::EmptySnapshot);
    }
    if !is_official_b3_url(&snapshot.source_url)
        || snapshot
            .securities
            .iter()
            .any(|security| !is_official_b3_url(&security.source_url))
    {
        return Err(EligibilityError::UnsupportedSourceUrl);
    }
    let now = Utc::now();
    if snapshot
        .securities
        .iter()
        .any(|security| security.ticker.trim().is_empty())
    {
        return Err(EligibilityError::EmptyTicker);
    }
    if snapshot.retrieved_at > now
        || snapshot
            .published_at
            .is_some_and(|published_at| published_at > now)
        || snapshot.securities.iter().any(|security| {
            security.verified_at > now
                || security
                    .published_at
                    .is_some_and(|published_at| published_at > now)
        })
    {
        return Err(EligibilityError::FutureTimestamp);
    }
    Ok(())
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS b3_eligibility_snapshot (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            retrieved_at TEXT NOT NULL,
            snapshot_json TEXT NOT NULL
        );",
    )
}

pub fn save_snapshot(conn: &Connection, snapshot: &B3Snapshot) -> rusqlite::Result<()> {
    validate_snapshot(snapshot).map_err(|error| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error.to_string())))
    })?;
    conn.execute(
        "INSERT INTO b3_eligibility_snapshot(id, retrieved_at, snapshot_json)
         VALUES(1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET
             retrieved_at = excluded.retrieved_at,
             snapshot_json = excluded.snapshot_json",
        rusqlite::params![
            snapshot.retrieved_at.to_rfc3339(),
            serde_json::to_string(snapshot)
                .map_err(|error| { rusqlite::Error::ToSqlConversionFailure(Box::new(error)) })?,
        ],
    )?;
    Ok(())
}

pub fn load_snapshot(conn: &Connection) -> rusqlite::Result<Option<B3Snapshot>> {
    let json = conn
        .query_row(
            "SELECT snapshot_json FROM b3_eligibility_snapshot WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    json.map(|json| {
        parse_snapshot(&json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                json.len(),
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(error.to_string())),
            )
        })
    })
    .transpose()
}

pub fn build_report(
    fundamentus_tickers: &[String],
    snapshot: Option<&B3Snapshot>,
    max_data_age_days: i64,
) -> EligibilityReport {
    let generated_at = Utc::now();
    let mut warnings = Vec::new();
    let mut findings = Vec::new();
    let mut unmatched_fundamentus_tickers = Vec::new();
    let mut b3_securities_not_in_fundamentus = Vec::new();
    let mut snapshot_age_days = None;

    if max_data_age_days < 0 {
        warnings
            .push("Maximum data age is negative; all timestamped evidence is stale.".to_string());
    }

    let by_ticker = snapshot.map(|snapshot| {
        let mut records: BTreeMap<String, Vec<&B3SecurityEvidence>> = BTreeMap::new();
        for security in &snapshot.securities {
            records
                .entry(normalize_ticker(&security.ticker))
                .or_default()
                .push(security);
        }
        records
    });

    if let Some(snapshot) = snapshot {
        snapshot_age_days = Some((generated_at - snapshot.retrieved_at).num_days());
        warnings.extend(snapshot.warnings.clone());
        if snapshot_age_days.is_some_and(|age| age > max_data_age_days) {
            warnings.push(format!(
                "The cached B3 snapshot is {} days old; the configured maximum is {} days.",
                snapshot_age_days.unwrap_or_default(),
                max_data_age_days
            ));
        }
        warnings.push(
            "B3 trading status is not fetched or independently verified by Midas; imported evidence is operator-supplied."
                .to_string(),
        );
    } else {
        warnings.push("No B3 eligibility snapshot has been imported.".to_string());
    }

    for ticker in fundamentus_tickers {
        let normalized = normalize_ticker(ticker);
        let matches = by_ticker
            .as_ref()
            .and_then(|records| records.get(&normalized))
            .cloned()
            .unwrap_or_default();
        match matches.as_slice() {
            [] => {
                unmatched_fundamentus_tickers.push(ticker.clone());
                findings.push(finding_without_evidence(
                    ticker,
                    EligibilityStatus::Unknown,
                    "No exact ticker match was found in the imported B3 snapshot.",
                ));
            }
            [security] => {
                let finding = classify_security(
                    security,
                    snapshot.and_then(|snapshot| snapshot.published_at),
                    max_data_age_days,
                    generated_at,
                );
                findings.push(finding);
            }
            _ => {
                findings.push(finding_without_evidence(
                    ticker,
                    EligibilityStatus::ReviewRequired,
                    "Multiple B3 records share this normalized ticker; an exact security identifier is required.",
                ));
            }
        }
    }

    if let Some(snapshot) = snapshot {
        let fundamentus_set = fundamentus_tickers
            .iter()
            .map(|ticker| normalize_ticker(ticker))
            .collect::<BTreeSet<_>>();
        for security in &snapshot.securities {
            let normalized = normalize_ticker(&security.ticker);
            if !fundamentus_set.contains(&normalized) {
                b3_securities_not_in_fundamentus.push(security.ticker.clone());
            }
        }
    }

    let imported_individual_security_status_evidence_complete = !findings.is_empty()
        && findings.iter().all(|finding| {
            finding.evidence_freshness == EvidenceFreshness::Current
                && matches!(
                    finding.status,
                    EligibilityStatus::Eligible
                        | EligibilityStatus::Suspended
                        | EligibilityStatus::Delisted
                )
        });
    let counts = counts_for(&findings, unmatched_fundamentus_tickers.len());
    EligibilityReport {
        generated_at,
        max_data_age_days,
        snapshot_retrieved_at: snapshot.map(|snapshot| snapshot.retrieved_at),
        snapshot_published_at: snapshot.and_then(|snapshot| snapshot.published_at),
        snapshot_age_days,
        source: snapshot.map(|snapshot| snapshot.source.clone()),
        source_url: snapshot.map(|snapshot| snapshot.source_url.clone()),
        official_individual_security_status_feed_integrated: false,
        imported_individual_security_status_evidence_complete,
        counts: EligibilityCounts {
            b3_securities_not_in_fundamentus: b3_securities_not_in_fundamentus.len(),
            ..counts
        },
        securities: findings,
        unmatched_fundamentus_tickers,
        b3_securities_not_in_fundamentus,
        warnings,
    }
}

pub fn eligible_tickers(report: &EligibilityReport) -> BTreeSet<String> {
    report
        .securities
        .iter()
        .filter(|finding| finding.status == EligibilityStatus::Eligible)
        .map(|finding| normalize_ticker(&finding.ticker))
        .collect()
}

pub fn listed_tickers(snapshot: &B3Snapshot, max_data_age_days: i64) -> BTreeSet<String> {
    if !snapshot_is_current(snapshot, max_data_age_days) {
        return BTreeSet::new();
    }

    let now = Utc::now();
    let mut by_ticker: BTreeMap<String, Vec<&B3SecurityEvidence>> = BTreeMap::new();
    for security in &snapshot.securities {
        by_ticker
            .entry(normalize_ticker(&security.ticker))
            .or_default()
            .push(security);
    }
    by_ticker
        .into_iter()
        .filter_map(|(ticker, matches)| {
            let [security] = matches.as_slice() else {
                return None;
            };
            (security.listing_status == ListingStatus::Listed
                && is_official_b3_url(&security.source_url)
                && !is_stale(security.verified_at, max_data_age_days, now)
                && !security
                    .published_at
                    .is_some_and(|date| is_stale(date, max_data_age_days, now)))
            .then_some(ticker)
        })
        .collect()
}

pub fn snapshot_is_current(snapshot: &B3Snapshot, max_data_age_days: i64) -> bool {
    let now = Utc::now();
    max_data_age_days >= 0
        && is_official_b3_url(&snapshot.source_url)
        && !is_stale(snapshot.retrieved_at, max_data_age_days, now)
        && !snapshot
            .published_at
            .is_some_and(|date| is_stale(date, max_data_age_days, now))
}

fn is_stale(date: DateTime<Utc>, max_data_age_days: i64, now: DateTime<Utc>) -> bool {
    now - date > Duration::days(max_data_age_days)
}

fn classify_security(
    security: &B3SecurityEvidence,
    snapshot_published_at: Option<DateTime<Utc>>,
    max_data_age_days: i64,
    now: DateTime<Utc>,
) -> SecurityFinding {
    let published_at = security.published_at.or(snapshot_published_at);
    let too_old = |date: DateTime<Utc>| now - date > Duration::days(max_data_age_days);
    let stale = too_old(security.verified_at) || published_at.is_some_and(too_old);
    let evidence_status = match (security.listing_status, security.trading_status) {
        (ListingStatus::Delisted, TradingStatus::Unknown) => Some(EligibilityStatus::Delisted),
        (ListingStatus::Listed, TradingStatus::Suspended | TradingStatus::Restricted) => {
            Some(EligibilityStatus::Suspended)
        }
        (ListingStatus::Listed, TradingStatus::Normal) => Some(EligibilityStatus::Eligible),
        (ListingStatus::Unknown, TradingStatus::Suspended | TradingStatus::Restricted) => {
            Some(EligibilityStatus::Suspended)
        }
        _ => None,
    };
    let (status, freshness, reason) = if !is_official_b3_url(&security.source_url) {
        (
            EligibilityStatus::ReviewRequired,
            EvidenceFreshness::Unavailable,
            "The supplied evidence URL is not on an official B3 domain.".to_string(),
        )
    } else if stale {
        (
            EligibilityStatus::Unknown,
            EvidenceFreshness::Historical,
            format!(
                "Evidence supports {} historically but is older than the configured maximum age.",
                status_label(evidence_status.unwrap_or(EligibilityStatus::Unknown))
            ),
        )
    } else if let Some(status) = evidence_status {
        (
            status,
            EvidenceFreshness::Current,
            match status {
                EligibilityStatus::Eligible => {
                    "Imported evidence explicitly reports a listed security with normal trading status."
                }
                EligibilityStatus::Suspended => {
                    "Imported evidence reports suspended or restricted trading for this security."
                }
                EligibilityStatus::Delisted => {
                    "Imported evidence explicitly reports this security as delisted."
                }
                _ => unreachable!(),
            }
            .to_string(),
        )
    } else if security.listing_status == ListingStatus::Listed
        || security.trading_status != TradingStatus::Unknown
    {
        (
            EligibilityStatus::ReviewRequired,
            EvidenceFreshness::Current,
            "A listing match exists, but the imported evidence does not establish individual-security trading status."
                .to_string(),
        )
    } else {
        (
            EligibilityStatus::Unknown,
            EvidenceFreshness::Unavailable,
            "The imported evidence does not establish listing or current trading status."
                .to_string(),
        )
    };
    let mut limitations = security.limitations.clone();
    if published_at.is_none() {
        limitations.push("The source publication timestamp was not supplied.".to_string());
    }
    SecurityFinding {
        ticker: security.ticker.clone(),
        security_id: security.security_id.clone(),
        status,
        last_known_status: stale.then_some(evidence_status).flatten(),
        evidence_freshness: freshness,
        reason,
        source: Some(security.source.clone()),
        source_url: Some(security.source_url.clone()),
        evidence_published_at: published_at,
        verified_at: Some(security.verified_at),
        limitations,
    }
}

fn finding_without_evidence(
    ticker: &str,
    status: EligibilityStatus,
    reason: &str,
) -> SecurityFinding {
    SecurityFinding {
        ticker: ticker.to_string(),
        security_id: None,
        status,
        last_known_status: None,
        evidence_freshness: EvidenceFreshness::Unavailable,
        reason: reason.to_string(),
        source: None,
        source_url: None,
        evidence_published_at: None,
        verified_at: None,
        limitations: vec!["No security-level evidence is available.".to_string()],
    }
}

fn counts_for(findings: &[SecurityFinding], unmatched: usize) -> EligibilityCounts {
    let mut counts = EligibilityCounts {
        processed: findings.len(),
        eligible: 0,
        suspended: 0,
        delisted: 0,
        unknown: 0,
        review_required: 0,
        unmatched_fundamentus: unmatched,
        b3_securities_not_in_fundamentus: 0,
    };
    for finding in findings {
        match finding.status {
            EligibilityStatus::Eligible => counts.eligible += 1,
            EligibilityStatus::Suspended => counts.suspended += 1,
            EligibilityStatus::Delisted => counts.delisted += 1,
            EligibilityStatus::Unknown => counts.unknown += 1,
            EligibilityStatus::ReviewRequired => counts.review_required += 1,
        }
    }
    counts
}

fn normalize_ticker(ticker: &str) -> String {
    ticker.trim().to_ascii_uppercase()
}

fn is_official_b3_url(url: &str) -> bool {
    let Some(host) = url
        .strip_prefix("https://")
        .and_then(|url| url.split('/').next())
    else {
        return false;
    };
    let host = host.split(':').next().unwrap_or_default();
    host == "b3.com.br" || host.ends_with(".b3.com.br")
}

fn status_label(status: EligibilityStatus) -> &'static str {
    match status {
        EligibilityStatus::Eligible => "eligible",
        EligibilityStatus::Suspended => "suspended",
        EligibilityStatus::Delisted => "delisted",
        EligibilityStatus::Unknown => "unknown",
        EligibilityStatus::ReviewRequired => "requiring review",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn security(
        ticker: &str,
        listing_status: ListingStatus,
        trading_status: TradingStatus,
    ) -> B3SecurityEvidence {
        B3SecurityEvidence {
            ticker: ticker.to_string(),
            security_id: Some(format!("ISIN-{ticker}")),
            listing_status,
            trading_status,
            verified_at: Utc::now(),
            published_at: Some(Utc::now()),
            source: "B3 test file".to_string(),
            source_url: "https://www.b3.com.br/test-file".to_string(),
            limitations: Vec::new(),
        }
    }

    fn snapshot(securities: Vec<B3SecurityEvidence>) -> B3Snapshot {
        B3Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            retrieved_at: Utc::now(),
            published_at: Some(Utc::now()),
            source: "B3 test file".to_string(),
            source_url: "https://www.b3.com.br/test-file".to_string(),
            securities,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn classifies_only_explicit_current_security_level_evidence() {
        let snapshot = snapshot(vec![
            security("AAA3", ListingStatus::Listed, TradingStatus::Normal),
            security("BBB4", ListingStatus::Listed, TradingStatus::Suspended),
            security("CCC3", ListingStatus::Listed, TradingStatus::Unknown),
            security("DDD3", ListingStatus::Delisted, TradingStatus::Unknown),
        ]);
        let report = build_report(
            &["AAA3".into(), "BBB4".into(), "CCC3".into(), "DDD3".into()],
            Some(&snapshot),
            1,
        );
        assert_eq!(report.counts.eligible, 1);
        assert_eq!(report.counts.suspended, 1);
        assert_eq!(report.counts.review_required, 1);
        assert_eq!(report.counts.delisted, 1);
        assert!(!report.imported_individual_security_status_evidence_complete);
    }

    #[test]
    fn stale_status_is_historical_and_never_eligible() {
        let mut stale = security("AAA3", ListingStatus::Listed, TradingStatus::Normal);
        stale.verified_at = Utc::now() - Duration::days(10);
        stale.published_at = Some(stale.verified_at);
        let report = build_report(&["AAA3".into()], Some(&snapshot(vec![stale])), 2);
        assert_eq!(report.securities[0].status, EligibilityStatus::Unknown);
        assert_eq!(
            report.securities[0].last_known_status,
            Some(EligibilityStatus::Eligible)
        );
        assert_eq!(
            report.securities[0].evidence_freshness,
            EvidenceFreshness::Historical
        );
    }

    #[test]
    fn matching_is_exact_and_duplicates_require_review() {
        let snapshot = snapshot(vec![
            security("AB3", ListingStatus::Listed, TradingStatus::Normal),
            security("AB3", ListingStatus::Listed, TradingStatus::Normal),
            security("AB4", ListingStatus::Listed, TradingStatus::Normal),
        ]);
        let report = build_report(&["AB3".into(), "AB".into()], Some(&snapshot), 1);
        assert_eq!(
            report.securities[0].status,
            EligibilityStatus::ReviewRequired
        );
        assert_eq!(report.securities[1].status, EligibilityStatus::Unknown);
        assert_eq!(report.b3_securities_not_in_fundamentus, vec!["AB4"]);
    }

    #[test]
    fn listing_presence_without_trading_status_is_not_eligible() {
        let snapshot = snapshot(vec![security(
            "AAA3",
            ListingStatus::Listed,
            TradingStatus::Unknown,
        )]);
        let report = build_report(&["AAA3".into()], Some(&snapshot), 1);
        assert_eq!(
            report.securities[0].status,
            EligibilityStatus::ReviewRequired
        );
        assert!(!report.imported_individual_security_status_evidence_complete);
    }

    #[test]
    fn listed_tickers_keeps_only_fresh_unique_exact_listing_matches() {
        let mut stale = security("CCC3", ListingStatus::Listed, TradingStatus::Normal);
        stale.verified_at = Utc::now() - Duration::days(3);
        let snapshot = snapshot(vec![
            security("AAA3", ListingStatus::Listed, TradingStatus::Normal),
            security("BBB4", ListingStatus::Delisted, TradingStatus::Unknown),
            security("DDD3", ListingStatus::Listed, TradingStatus::Unknown),
            security("DDD3", ListingStatus::Listed, TradingStatus::Normal),
            stale,
        ]);
        assert_eq!(
            listed_tickers(&snapshot, 1),
            BTreeSet::from(["AAA3".to_string()])
        );
    }

    #[test]
    fn individual_security_status_is_verified_only_for_resolved_fresh_matches() {
        let snapshot = snapshot(vec![security(
            "AAA3",
            ListingStatus::Listed,
            TradingStatus::Normal,
        )]);
        let report = build_report(&["AAA3".into()], Some(&snapshot), 1);
        assert!(report.imported_individual_security_status_evidence_complete);
    }

    #[test]
    fn snapshot_round_trips_through_the_cache() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        let snapshot = snapshot(vec![security(
            "AAA3",
            ListingStatus::Listed,
            TradingStatus::Normal,
        )]);
        save_snapshot(&connection, &snapshot).unwrap();
        assert_eq!(load_snapshot(&connection).unwrap(), Some(snapshot));
    }

    #[test]
    fn parser_rejects_non_b3_evidence_urls() {
        let mut snapshot = snapshot(vec![security(
            "AAA3",
            ListingStatus::Listed,
            TradingStatus::Normal,
        )]);
        snapshot.securities[0].source_url = "https://example.org/status".to_string();
        assert!(matches!(
            validate_snapshot(&snapshot),
            Err(EligibilityError::UnsupportedSourceUrl)
        ));
    }
}
