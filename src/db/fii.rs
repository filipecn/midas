//! Append-preserving storage for point-in-time FII filing observations.
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, params};

use crate::investing::fii::{FiiId, FilingObservation};

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
        ON fii_filing_observations(fund_id, available_at, reference_date);",
    )
}

/// Inserts an immutable filing revision. Re-ingesting the same source revision is idempotent.
pub fn save_filing(conn: &Connection, filing: &FilingObservation) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO fii_filing_observations
        (fund_id, reference_date, available_at, ingested_at, source, dataset, filing_version, revision_id, payload_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![
        filing.fund_id.0, filing.reference_date.to_string(), filing.available_at.to_rfc3339(),
        filing.ingested_at.to_rfc3339(), filing.source, filing.dataset, filing.filing_version,
        filing.revision_id, filing.payload_json
    ])?;
    Ok(())
}

/// Returns only filings publicly available by `as_of`; later revisions remain queryable separately.
pub fn filings_as_of(
    conn: &Connection,
    fund_id: &FiiId,
    as_of: DateTime<Utc>,
) -> rusqlite::Result<Vec<FilingObservation>> {
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
        Ok(FilingObservation {
            fund_id: fund_id.clone(),
            reference_date: NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|_| rusqlite::Error::InvalidQuery)?,
            available_at: DateTime::parse_from_rfc3339(&available).map_err(|_| rusqlite::Error::InvalidQuery)?.with_timezone(&Utc),
            ingested_at: DateTime::parse_from_rfc3339(&ingested).map_err(|_| rusqlite::Error::InvalidQuery)?.with_timezone(&Utc),
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
}
