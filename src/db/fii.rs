//! Append-preserving storage for point-in-time FII filing observations.
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, params};
use thiserror::Error;

use crate::investing::fii::{
    Distribution, DistributionType, FiiId, FilingObservation, MarketObservation,
};

#[derive(Debug, Error)]
pub enum FiiStoreError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error("filing revision key already exists with different content: {0}")]
    RevisionConflict(String),
    #[error("stored filing contains invalid {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS fii_filing_observations (
        id INTEGER PRIMARY KEY,
        fund_id TEXT NOT NULL,
        reference_date TEXT NOT NULL,
        available_at TEXT NOT NULL,
        ingested_at TEXT NOT NULL,
        source TEXT NOT NULL,
        dataset TEXT NOT NULL,
        filing_version TEXT,
        revision_id TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        UNIQUE(fund_id, source, dataset, reference_date, revision_id)
    );
    CREATE INDEX IF NOT EXISTS fii_filing_point_in_time
        ON fii_filing_observations(fund_id, available_at, reference_date);
    CREATE TABLE IF NOT EXISTS fii_market_observations (
        id INTEGER PRIMARY KEY,
        fund_id TEXT NOT NULL,
        reference_date TEXT NOT NULL,
        available_at TEXT NOT NULL,
        ingested_at TEXT NOT NULL,
        source TEXT NOT NULL,
        revision_id TEXT NOT NULL,
        raw_price REAL,
        adjusted_price REAL,
        trading_volume REAL,
        financial_volume REAL,
        market_cap REAL,
        shares_outstanding REAL,
        UNIQUE(fund_id, reference_date, source, revision_id)
    );
    CREATE INDEX IF NOT EXISTS fii_market_point_in_time
        ON fii_market_observations(fund_id, available_at, reference_date);
    CREATE TABLE IF NOT EXISTS fii_distributions (
        id INTEGER PRIMARY KEY,
        fund_id TEXT NOT NULL,
        declared_on TEXT,
        ex_date TEXT,
        payment_date TEXT,
        per_share REAL NOT NULL,
        distribution_type TEXT NOT NULL,
        reference_period TEXT,
        source TEXT NOT NULL,
        published_at TEXT NOT NULL,
        ingested_at TEXT NOT NULL,
        revision_id TEXT NOT NULL,
        UNIQUE(fund_id, source, payment_date, revision_id)
    );
    CREATE INDEX IF NOT EXISTS fii_distribution_point_in_time
        ON fii_distributions(fund_id, published_at, payment_date);",
    )
}

pub fn save_market_observation(
    conn: &Connection,
    observation: &MarketObservation,
) -> Result<(), FiiStoreError> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO fii_market_observations
         (fund_id, reference_date, available_at, ingested_at, source, revision_id, raw_price,
          adjusted_price, trading_volume, financial_volume, market_cap, shares_outstanding)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            observation.fund_id.0,
            observation.reference_date.to_string(),
            observation.available_at.to_rfc3339(),
            observation.ingested_at.to_rfc3339(),
            observation.source,
            observation.revision_id,
            observation.raw_price,
            observation.adjusted_price,
            observation.trading_volume,
            observation.financial_volume,
            observation.market_cap,
            observation.shares_outstanding,
        ],
    )?;
    if inserted == 0 {
        let existing: (
            String,
            String,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
        ) = conn.query_row(
            "SELECT available_at, ingested_at, raw_price, adjusted_price, trading_volume,
                    financial_volume, market_cap, shares_outstanding FROM fii_market_observations
             WHERE fund_id = ?1 AND reference_date = ?2 AND source = ?3 AND revision_id = ?4",
            params![
                observation.fund_id.0,
                observation.reference_date.to_string(),
                observation.source,
                observation.revision_id
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )?;
        let expected = (
            observation.available_at.to_rfc3339(),
            observation.ingested_at.to_rfc3339(),
            observation.raw_price,
            observation.adjusted_price,
            observation.trading_volume,
            observation.financial_volume,
            observation.market_cap,
            observation.shares_outstanding,
        );
        if existing != expected {
            return Err(FiiStoreError::RevisionConflict(format!(
                "{} / {} / {} / {}",
                observation.fund_id.0,
                observation.source,
                observation.reference_date,
                observation.revision_id
            )));
        }
    }
    Ok(())
}

pub fn market_observations_as_of(
    conn: &Connection,
    fund_id: &FiiId,
    as_of: DateTime<Utc>,
) -> Result<Vec<MarketObservation>, FiiStoreError> {
    let mut statement = conn.prepare(
        "SELECT reference_date, available_at, ingested_at, source, revision_id, raw_price,
                adjusted_price, trading_volume, financial_volume, market_cap, shares_outstanding
         FROM fii_market_observations WHERE fund_id = ?1 AND available_at <= ?2
         ORDER BY reference_date, available_at, source, revision_id",
    )?;
    let rows = statement.query_map(params![fund_id.0, as_of.to_rfc3339()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<f64>>(5)?,
            row.get::<_, Option<f64>>(6)?,
            row.get::<_, Option<f64>>(7)?,
            row.get::<_, Option<f64>>(8)?,
            row.get::<_, Option<f64>>(9)?,
            row.get::<_, Option<f64>>(10)?,
        ))
    })?;
    rows.map(|row| {
        let (
            date,
            available,
            ingested,
            source,
            revision_id,
            raw_price,
            adjusted_price,
            trading_volume,
            financial_volume,
            market_cap,
            shares_outstanding,
        ) = row?;
        Ok(MarketObservation {
            fund_id: fund_id.clone(),
            reference_date: parse_date("reference_date", date)?,
            available_at: parse_timestamp("available_at", available)?,
            ingested_at: parse_timestamp("ingested_at", ingested)?,
            source,
            revision_id,
            raw_price,
            adjusted_price,
            trading_volume,
            financial_volume,
            market_cap,
            shares_outstanding,
        })
    })
    .collect()
}

/// Historical distribution storage requires a source publication date; unknown publication
/// time must not be treated as if the event were known at its ex-date or payment date.
pub fn save_distribution(
    conn: &Connection,
    distribution: &Distribution,
) -> Result<(), FiiStoreError> {
    let published_at =
        distribution
            .published_at
            .ok_or_else(|| FiiStoreError::InvalidStoredValue {
                field: "published_at",
                value: "missing publication timestamp".into(),
            })?;
    if !distribution.per_share.is_finite() || distribution.per_share < 0.0 {
        return Err(FiiStoreError::InvalidStoredValue {
            field: "per_share",
            value: distribution.per_share.to_string(),
        });
    }
    if distribution.payment_date.is_none() {
        return Err(FiiStoreError::InvalidStoredValue {
            field: "payment_date",
            value: "missing event identity date".into(),
        });
    }
    let kind = serde_json::to_string(&distribution.distribution_type).map_err(|error| {
        FiiStoreError::InvalidStoredValue {
            field: "distribution_type",
            value: error.to_string(),
        }
    })?;
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO fii_distributions
         (fund_id, declared_on, ex_date, payment_date, per_share, distribution_type, reference_period,
          source, published_at, ingested_at, revision_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            distribution.fund_id.0,
            distribution.declared_on.map(|date| date.to_string()),
            distribution.ex_date.map(|date| date.to_string()),
            distribution.payment_date.map(|date| date.to_string()),
            distribution.per_share,
            kind,
            distribution.reference_period,
            distribution.source,
            published_at.to_rfc3339(),
            distribution.ingested_at.to_rfc3339(),
            distribution.revision_id,
        ],
    )?;
    if inserted == 0 {
        let existing: (
            Option<String>,
            Option<String>,
            Option<String>,
            f64,
            String,
            Option<String>,
            String,
            String,
            String,
        ) = conn.query_row(
            "SELECT declared_on, ex_date, payment_date, per_share, distribution_type,
                    reference_period, published_at, ingested_at, revision_id FROM fii_distributions
             WHERE fund_id = ?1 AND source = ?2 AND payment_date = ?3 AND revision_id = ?4",
            params![
                distribution.fund_id.0,
                distribution.source,
                distribution.payment_date.map(|date| date.to_string()),
                distribution.revision_id
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )?;
        let expected = (
            distribution.declared_on.map(|date| date.to_string()),
            distribution.ex_date.map(|date| date.to_string()),
            distribution.payment_date.map(|date| date.to_string()),
            distribution.per_share,
            kind,
            distribution.reference_period.clone(),
            published_at.to_rfc3339(),
            distribution.ingested_at.to_rfc3339(),
            distribution.revision_id.clone(),
        );
        if existing != expected {
            return Err(FiiStoreError::RevisionConflict(format!(
                "{} / {} / {:?} / {}",
                distribution.fund_id.0,
                distribution.source,
                distribution.payment_date,
                distribution.revision_id
            )));
        }
    }
    Ok(())
}

pub fn distributions_as_of(
    conn: &Connection,
    fund_id: &FiiId,
    as_of: DateTime<Utc>,
) -> Result<Vec<Distribution>, FiiStoreError> {
    let mut statement = conn.prepare(
        "SELECT declared_on, ex_date, payment_date, per_share, distribution_type, reference_period,
                source, published_at, ingested_at, revision_id
         FROM fii_distributions WHERE fund_id = ?1 AND published_at <= ?2
         ORDER BY payment_date, published_at, revision_id",
    )?;
    let rows = statement.query_map(params![fund_id.0, as_of.to_rfc3339()], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
        ))
    })?;
    rows.map(|row| {
        let (
            declared,
            ex_date,
            payment,
            per_share,
            kind,
            reference_period,
            source,
            published,
            ingested,
            revision_id,
        ) = row?;
        Ok(Distribution {
            fund_id: fund_id.clone(),
            declared_on: declared
                .map(|value| parse_date("declared_on", value))
                .transpose()?,
            ex_date: ex_date
                .map(|value| parse_date("ex_date", value))
                .transpose()?,
            payment_date: payment
                .map(|value| parse_date("payment_date", value))
                .transpose()?,
            per_share,
            distribution_type: serde_json::from_str::<DistributionType>(&kind).map_err(
                |error| FiiStoreError::InvalidStoredValue {
                    field: "distribution_type",
                    value: error.to_string(),
                },
            )?,
            reference_period,
            source,
            published_at: Some(parse_timestamp("published_at", published)?),
            ingested_at: parse_timestamp("ingested_at", ingested)?,
            revision_id,
        })
    })
    .collect()
}

fn parse_date(field: &'static str, value: String) -> Result<NaiveDate, FiiStoreError> {
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .map_err(|_| FiiStoreError::InvalidStoredValue { field, value })
}

fn parse_timestamp(field: &'static str, value: String) -> Result<DateTime<Utc>, FiiStoreError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| FiiStoreError::InvalidStoredValue { field, value })
}

/// Inserts an immutable filing revision. Re-ingesting the same source revision is idempotent.
pub fn save_filing(conn: &Connection, filing: &FilingObservation) -> Result<(), FiiStoreError> {
    let inserted = conn.execute("INSERT OR IGNORE INTO fii_filing_observations
        (fund_id, reference_date, available_at, ingested_at, source, dataset, filing_version, revision_id, payload_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![
        filing.fund_id.0, filing.reference_date.to_string(), filing.available_at.to_rfc3339(),
        filing.ingested_at.to_rfc3339(), filing.source, filing.dataset, filing.filing_version,
        filing.revision_id, filing.payload_json
    ])?;
    if inserted == 0 {
        let existing: String = conn.query_row(
            "SELECT payload_json FROM fii_filing_observations
             WHERE fund_id = ?1 AND source = ?2 AND dataset = ?3 AND reference_date = ?4 AND revision_id = ?5",
            params![filing.fund_id.0, filing.source, filing.dataset, filing.reference_date.to_string(), filing.revision_id],
            |row| row.get(0),
        )?;
        if existing != filing.payload_json {
            return Err(FiiStoreError::RevisionConflict(format!(
                "{} / {} / {} / {} / {}",
                filing.fund_id.0,
                filing.source,
                filing.dataset,
                filing.reference_date,
                filing.revision_id
            )));
        }
    }
    Ok(())
}

/// Returns only filings publicly available by `as_of`; later revisions remain queryable separately.
pub fn filings_as_of(
    conn: &Connection,
    fund_id: &FiiId,
    as_of: DateTime<Utc>,
) -> Result<Vec<FilingObservation>, FiiStoreError> {
    let mut statement = conn.prepare("SELECT reference_date, available_at, ingested_at, source, dataset, filing_version, revision_id, payload_json
        FROM fii_filing_observations WHERE fund_id = ?1 AND available_at <= ?2
        ORDER BY reference_date, available_at, revision_id")?;
    let rows = statement.query_map(params![fund_id.0, as_of.to_rfc3339()], |row| {
        let date: String = row.get(0)?;
        let available: String = row.get(1)?;
        let ingested: String = row.get(2)?;
        Ok((
            date,
            available,
            ingested,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;
    rows.map(|row| {
        let (date, available, ingested, source, dataset, filing_version, revision_id, payload_json) = row?;
        let reference_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| FiiStoreError::InvalidStoredValue { field: "reference_date", value: date })?;
        let available_at = DateTime::parse_from_rfc3339(&available).map_err(|_| FiiStoreError::InvalidStoredValue { field: "available_at", value: available })?.with_timezone(&Utc);
        let ingested_at = DateTime::parse_from_rfc3339(&ingested).map_err(|_| FiiStoreError::InvalidStoredValue { field: "ingested_at", value: ingested })?.with_timezone(&Utc);
        Ok(FilingObservation {
            fund_id: fund_id.clone(),
            reference_date,
            available_at,
            ingested_at,
            source, dataset, filing_version, revision_id, payload_json,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn filing(revision: &str, available: u32) -> FilingObservation {
        FilingObservation {
            fund_id: FiiId("cnpj:00.000.000/0001-00".into()),
            reference_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            available_at: Utc.with_ymd_and_hms(2026, 4, available, 12, 0, 0).unwrap(),
            ingested_at: Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap(),
            source: "CVM".into(),
            dataset: "informe_test".into(),
            filing_version: Some(revision.into()),
            revision_id: revision.into(),
            payload_json: format!("{{\"revision\":\"{revision}\"}}"),
        }
    }
    #[test]
    fn point_in_time_query_excludes_later_publications_and_retains_revisions() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        save_filing(&conn, &filing("1", 10)).unwrap();
        save_filing(&conn, &filing("2", 20)).unwrap();
        let at_april_15 = filings_as_of(
            &conn,
            &FiiId("cnpj:00.000.000/0001-00".into()),
            Utc.with_ymd_and_hms(2026, 4, 15, 0, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(at_april_15.len(), 1);
        assert_eq!(at_april_15[0].revision_id, "1");
        let at_year_end = filings_as_of(
            &conn,
            &FiiId("cnpj:00.000.000/0001-00".into()),
            Utc.with_ymd_and_hms(2026, 12, 31, 0, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(at_year_end.len(), 2);
    }

    #[test]
    fn repeated_revision_is_idempotent_but_conflicting_payload_is_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let first = filing("1", 10);
        save_filing(&conn, &first).unwrap();
        save_filing(&conn, &first).unwrap();
        let mut changed = first.clone();
        changed.payload_json = "{\"revision\":\"changed\"}".into();
        assert!(matches!(
            save_filing(&conn, &changed),
            Err(FiiStoreError::RevisionConflict(_))
        ));
    }

    #[test]
    fn market_queries_filter_by_snapshot_availability_and_keep_price_basis_separate() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let fund_id = FiiId("cnpj:00332266000131".into());
        let observation = MarketObservation {
            fund_id: fund_id.clone(),
            reference_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
            available_at: Utc.with_ymd_and_hms(2025, 2, 20, 12, 0, 0).unwrap(),
            ingested_at: Utc.with_ymd_and_hms(2025, 2, 21, 12, 0, 0).unwrap(),
            source: "fixture".into(),
            revision_id: "snapshot-1".into(),
            raw_price: Some(170.0),
            adjusted_price: Some(168.0),
            trading_volume: Some(100.0),
            financial_volume: None,
            market_cap: None,
            shares_outstanding: None,
        };
        save_market_observation(&conn, &observation).unwrap();
        assert!(
            market_observations_as_of(
                &conn,
                &fund_id,
                Utc.with_ymd_and_hms(2025, 2, 20, 11, 59, 59).unwrap(),
            )
            .unwrap()
            .is_empty()
        );
        let loaded = market_observations_as_of(
            &conn,
            &fund_id,
            Utc.with_ymd_and_hms(2025, 2, 22, 0, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(loaded[0].raw_price, Some(170.0));
        assert_eq!(loaded[0].adjusted_price, Some(168.0));
    }

    #[test]
    fn distribution_history_requires_publication_time_for_point_in_time_use() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let fund_id = FiiId("cnpj:00332266000131".into());
        let base = Distribution {
            fund_id: fund_id.clone(),
            declared_on: None,
            ex_date: Some(NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()),
            payment_date: Some(NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()),
            per_share: 0.75,
            distribution_type: DistributionType::Unknown,
            reference_period: None,
            source: "fixture".into(),
            published_at: None,
            ingested_at: Utc.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap(),
            revision_id: "1".into(),
        };
        assert!(matches!(
            save_distribution(&conn, &base),
            Err(FiiStoreError::InvalidStoredValue {
                field: "published_at",
                ..
            })
        ));
        let published = Distribution {
            published_at: Some(Utc.with_ymd_and_hms(2025, 1, 10, 12, 0, 0).unwrap()),
            ..base
        };
        save_distribution(&conn, &published).unwrap();
        assert!(
            distributions_as_of(
                &conn,
                &fund_id,
                Utc.with_ymd_and_hms(2025, 1, 10, 11, 59, 0).unwrap()
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            distributions_as_of(
                &conn,
                &fund_id,
                Utc.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap()
            )
            .unwrap()[0]
                .per_share,
            0.75
        );
    }
}
