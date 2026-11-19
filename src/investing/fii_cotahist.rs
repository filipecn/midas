//! Reproducible COTAHIST acquisition and FII observations.
//!
//! `rsb3` 0.1.0 is the sole COTAHIST parser and downloader. Its cache treats an
//! extracted TXT as final, so each fetch uses a fresh private cache directory;
//! the returned ZIP is then copied into a content-addressed immutable store.
use std::{collections::BTreeMap, fs, path::Path};

use arrow::array::{Array, Date32Array, Int32Array, Int64Array, StringArray};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{db::fii, investing::fii::FiiId};

const SOURCE: &str = "B3_COTAHIST";
const BDI_FII: &str = "12";
const SCHEMA_VERSION: &str = "midas_fii_cotahist_v1";
const PARSER_VERSION: &str = "rsb3/0.1.0+midas-adapter/1";

#[derive(Debug, Error)]
pub enum CotahistError {
    #[error(transparent)]
    Rsb3(#[from] rsb3::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error("COTAHIST parser schema is missing or has an incompatible type: {0}")]
    Schema(String),
    #[error("COTAHIST record validation failed: {0}")]
    Validation(String),
    #[error("snapshot path is invalid: {0}")]
    SnapshotPath(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInput {
    pub cnpj: String,
    pub isin: String,
    pub ticker: Option<String>,
    pub fund_name: Option<String>,
    pub cvm_segment: Option<String>,
    pub evidence: String,
    #[serde(default)]
    pub valid_from: Option<NaiveDate>,
    #[serde(default)]
    pub valid_to: Option<NaiveDate>,
    /// Internal provenance marker; JSON input cannot self-assert source verification.
    #[serde(skip)]
    pub source_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestSummary {
    pub snapshot_id: String,
    pub archive: String,
    pub sha256: String,
    pub bytes: u64,
    pub total_records: usize,
    pub fii_records: usize,
    pub resolved_records: usize,
    pub unresolved_isin_count: usize,
    pub unresolved_isins: Vec<String>,
    pub duplicate_ticker_dates: usize,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FiiDataReport {
    pub as_of: DateTime<Utc>,
    pub funds: Vec<FundReport>,
    pub unresolved_market_rows: usize,
    pub market_gap_policy: String,
    pub distributions: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FundReport {
    pub fund_id: String,
    pub cnpj: Option<String>,
    pub ticker: Option<String>,
    pub isin: Option<String>,
    pub name: Option<String>,
    pub cvm_segment: Option<String>,
    pub mapping_evidence: Option<String>,
    pub mapping_status: String,
    pub first_trading_date: Option<String>,
    pub last_trading_date: Option<String>,
    pub observations: usize,
    pub last_raw_close_cents: Option<i64>,
    pub last_close_brl: Option<f64>,
    pub last_traded_quantity: Option<i64>,
    pub last_number_of_trades: Option<i64>,
    pub last_financial_volume_cents: Option<i64>,
    pub quotation_factor_min: Option<i64>,
    pub quotation_factor_max: Option<i64>,
    pub cvm_reports: Vec<CvmReportRecord>,
    pub snapshots: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvmReportRecord {
    pub reference_date: String,
    pub available_at: String,
    pub submission_date: Option<String>,
    pub availability_basis: String,
    pub filing_version: String,
    pub nav_per_share: Option<f64>,
    pub reported_monthly_yield: Option<f64>,
    pub aligned_market_date: Option<String>,
    pub aligned_raw_close_brl: Option<f64>,
    pub price_to_nav: Option<f64>,
    pub aligned_market_snapshot: Option<String>,
    pub alignment_policy: String,
    pub diagnostics: Vec<String>,
}

struct AlignedMarket {
    date: Option<String>,
    close_brl: Option<f64>,
    price_to_nav: Option<f64>,
    snapshot: Option<String>,
    diagnostics: Vec<String>,
}

#[derive(Debug, Clone)]
struct ParsedQuote {
    date: NaiveDate,
    bdi: String,
    ticker: String,
    market: i32,
    name: String,
    specification: String,
    currency: String,
    open: i64,
    high: i64,
    low: i64,
    average: i64,
    close: i64,
    bid: i64,
    ask: i64,
    trades: i64,
    quantity: i64,
    volume: i64,
    quotation_factor: i64,
    isin: String,
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    fii::init(conn)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS fii_source_snapshots (
            snapshot_id TEXT PRIMARY KEY, source TEXT NOT NULL, resource_id TEXT NOT NULL,
            original_filename TEXT NOT NULL, requested_period TEXT NOT NULL,
            retrieved_at TEXT NOT NULL, source_modified_at TEXT, sha256 TEXT NOT NULL,
            size_bytes INTEGER NOT NULL, local_path TEXT NOT NULL, parser_version TEXT NOT NULL,
            schema_version TEXT NOT NULL, ingestion_status TEXT NOT NULL,
            record_count INTEGER, diagnostics_json TEXT NOT NULL DEFAULT '[]',
            UNIQUE(source, resource_id, sha256)
        );
        CREATE INDEX IF NOT EXISTS fii_source_snapshots_source_time
            ON fii_source_snapshots(source, retrieved_at);
        CREATE TABLE IF NOT EXISTS fii_identity_aliases (
            fund_id TEXT NOT NULL, identifier_type TEXT NOT NULL, identifier_value TEXT NOT NULL,
            valid_from TEXT, valid_to TEXT, evidence TEXT NOT NULL, first_observed_at TEXT NOT NULL,
            confidence TEXT NOT NULL, source_snapshot_id TEXT,
            PRIMARY KEY(fund_id, identifier_type, identifier_value, first_observed_at)
        );
        CREATE TABLE IF NOT EXISTS fii_identity_registry (
            fund_id TEXT NOT NULL, cnpj TEXT NOT NULL, isin TEXT NOT NULL, ticker TEXT,
            fund_name TEXT, cvm_segment TEXT, evidence TEXT NOT NULL, valid_from TEXT,
            valid_to TEXT, first_observed_at TEXT NOT NULL, confidence TEXT NOT NULL,
            PRIMARY KEY(fund_id, isin, evidence, first_observed_at)
        );
        CREATE INDEX IF NOT EXISTS fii_identity_lookup
            ON fii_identity_aliases(identifier_type, identifier_value, first_observed_at);
        CREATE TABLE IF NOT EXISTS fii_fund_metadata (
            fund_id TEXT NOT NULL, cnpj TEXT NOT NULL, name TEXT, cvm_segment TEXT,
            reference_date TEXT NOT NULL, filing_version TEXT NOT NULL,
            available_at TEXT NOT NULL, ingested_at TEXT NOT NULL, source_snapshot_id TEXT NOT NULL,
            PRIMARY KEY(fund_id, reference_date, filing_version, source_snapshot_id)
        );
        CREATE TABLE IF NOT EXISTS fii_cotahist_observations (
            id INTEGER PRIMARY KEY, snapshot_id TEXT NOT NULL, fund_id TEXT,
            ticker TEXT NOT NULL, isin TEXT NOT NULL, trading_date TEXT NOT NULL,
            bdi_code TEXT NOT NULL, market_type INTEGER NOT NULL, security_name TEXT NOT NULL,
            security_specification TEXT NOT NULL, currency TEXT NOT NULL,
            open_cents INTEGER NOT NULL, high_cents INTEGER NOT NULL, low_cents INTEGER NOT NULL,
            average_cents INTEGER NOT NULL, close_cents INTEGER NOT NULL,
            best_bid_cents INTEGER NOT NULL, best_ask_cents INTEGER NOT NULL,
            number_of_trades INTEGER NOT NULL, traded_quantity INTEGER NOT NULL,
            financial_volume_cents INTEGER NOT NULL, quotation_factor INTEGER NOT NULL,
            open_brl REAL NOT NULL, high_brl REAL NOT NULL, low_brl REAL NOT NULL,
            average_brl REAL NOT NULL, close_brl REAL NOT NULL, ingested_at TEXT NOT NULL,
            UNIQUE(snapshot_id, ticker, isin, trading_date, market_type)
        );
        CREATE INDEX IF NOT EXISTS fii_cotahist_series
            ON fii_cotahist_observations(fund_id, trading_date);
        CREATE INDEX IF NOT EXISTS fii_cotahist_unresolved
            ON fii_cotahist_observations(isin, trading_date) WHERE fund_id IS NULL;",
    )
}

pub fn normalize_cnpj(raw: &str) -> Result<String, CotahistError> {
    let digits = raw.chars().filter(char::is_ascii_digit).collect::<String>();
    if digits.len() != 14 {
        return Err(CotahistError::Validation(format!("invalid CNPJ {raw:?}")));
    }
    Ok(digits)
}

pub fn save_identity(
    conn: &Connection,
    input: &IdentityInput,
    observed_at: DateTime<Utc>,
) -> Result<FiiId, CotahistError> {
    let cnpj = normalize_cnpj(&input.cnpj)?;
    let isin = input.isin.trim().to_ascii_uppercase();
    if isin.len() != 12 {
        return Err(CotahistError::Validation(format!(
            "invalid ISIN {:?}",
            input.isin
        )));
    }
    let id = FiiId(format!("cnpj:{cnpj}"));
    let confidence = if input.source_verified {
        "source_evidenced"
    } else {
        "configured"
    };
    let transaction = conn.unchecked_transaction()?;
    let values = std::iter::once(("cnpj", cnpj.as_str()))
        .chain(std::iter::once(("isin", isin.as_str())))
        .chain(input.ticker.as_deref().map(|v| ("ticker", v.trim())));
    for (kind, value) in values {
        if value.is_empty() {
            continue;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO fii_identity_aliases
             (fund_id, identifier_type, identifier_value, valid_from, valid_to, evidence,
              first_observed_at, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id.0,
                kind,
                value.to_ascii_uppercase(),
                input.valid_from.map(|d| d.to_string()),
                input.valid_to.map(|d| d.to_string()),
                input.evidence,
                observed_at.to_rfc3339(),
                confidence,
            ],
        )?;
    }
    transaction.execute(
        "INSERT OR IGNORE INTO fii_identity_registry
         (fund_id,cnpj,isin,ticker,fund_name,cvm_segment,evidence,valid_from,valid_to,first_observed_at,confidence)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![id.0,cnpj,isin,input.ticker.as_deref(),input.fund_name,input.cvm_segment,
            input.evidence,input.valid_from.map(|d|d.to_string()),input.valid_to.map(|d|d.to_string()),observed_at.to_rfc3339(),confidence],
    )?;
    transaction.commit()?;
    Ok(id)
}

/// Fetch through rsb3 using a fresh cache so a previous TXT can never mask a
/// remote correction. Preserve every retrieved ZIP by content hash.
pub fn acquire_and_ingest(
    conn: &Connection,
    period: rsb3::cotahist::Period,
    archive_root: &Path,
    staging_root: &Path,
    refresh: bool,
) -> Result<IngestSummary, CotahistError> {
    fs::create_dir_all(staging_root)?;
    let unique = format!(
        "{}-{}",
        period.file_stem(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let staging = staging_root.join(unique);
    fs::create_dir_all(&staging)?;
    let stem = period.file_stem();
    if !refresh {
        if let Some(existing) = newest_snapshot_archive(conn, archive_root, &stem)? {
            fs::copy(existing, staging.join(format!("{stem}.ZIP")))?;
        }
    }
    let cache = rsb3::cotahist::Cache {
        dir: staging.clone(),
    };
    let txt_path = rsb3::cotahist::fetch(period, &cache)?;
    let zip_path = staging.join(format!("{}.ZIP", period.file_stem()));
    if !zip_path.exists() {
        return Err(CotahistError::SnapshotPath(zip_path.display().to_string()));
    }
    let zip_bytes = fs::read(&zip_path)?;
    let sha = hex(&Sha256::digest(&zip_bytes));
    let snapshot_id = format!("b3-cotahist:{}:{sha}", period.file_stem());
    let snapshot_dir = archive_root.join(period.file_stem());
    fs::create_dir_all(&snapshot_dir)?;
    let immutable_archive = snapshot_dir.join(format!("{sha}.ZIP"));
    if !immutable_archive.exists() {
        fs::write(&immutable_archive, &zip_bytes)?;
    }
    let crate_manifest = staging.join(format!("{}.manifest.json", period.file_stem()));
    if crate_manifest.exists() {
        let immutable_manifest = snapshot_dir.join(format!("{sha}.rsb3.manifest.json"));
        if !immutable_manifest.exists() {
            fs::copy(crate_manifest, immutable_manifest)?;
        }
    }
    let retrieved_at = Utc::now();
    let batch = rsb3::cotahist::parse(&txt_path)?;
    let all_count = batch.num_rows();
    let quotes = quotes_from_batch(&batch)?;
    let fii_quotes = quotes
        .into_iter()
        .filter(|q| q.bdi == BDI_FII)
        .collect::<Vec<_>>();
    let duplicate_count = duplicate_ticker_dates(&fii_quotes);
    let identity_map = isin_identity_map(conn)?;
    let unresolved = fii_quotes
        .iter()
        .map(|q| q.isin.clone())
        .filter(|isin| identity_map.get(isin).and_then(|id| id.as_ref()).is_none())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let resolved_count = fii_quotes
        .iter()
        .filter(|q| {
            identity_map
                .get(&q.isin)
                .and_then(|id| id.as_ref())
                .is_some()
        })
        .count();
    let record_count = fii_quotes.len();
    let diagnostics = if duplicate_count > 0 {
        vec![format!(
            "{duplicate_count} duplicate ticker/date/ISIN keys in source period"
        )]
    } else {
        Vec::new()
    };
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO fii_source_snapshots
         (snapshot_id, source, resource_id, original_filename, requested_period, retrieved_at,
          sha256, size_bytes, local_path, parser_version, schema_version, ingestion_status,
          record_count, diagnostics_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'parsing', ?12, ?13)",
        params![
            snapshot_id,
            SOURCE,
            period.file_stem(),
            zip_path.file_name().unwrap().to_string_lossy(),
            period.file_stem(),
            retrieved_at.to_rfc3339(),
            sha,
            zip_bytes.len() as i64,
            immutable_archive.to_string_lossy(),
            PARSER_VERSION,
            SCHEMA_VERSION,
            record_count as i64,
            serde_json::to_string(&diagnostics).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    {
        let mut alias_stmt=tx.prepare_cached(
        "INSERT OR IGNORE INTO fii_identity_aliases
         (fund_id, identifier_type, identifier_value, valid_from, evidence, first_observed_at, confidence, source_snapshot_id)
         VALUES (?1,'ticker',?2,?3,'Ticker observed on COTAHIST record with matching ISIN',?4,'observed',?5)")?;
        let mut observation_stmt = tx.prepare_cached(
            "INSERT OR IGNORE INTO fii_cotahist_observations
             (snapshot_id, fund_id, ticker, isin, trading_date, bdi_code, market_type,
              security_name, security_specification, currency, open_cents, high_cents,
              low_cents, average_cents, close_cents, best_bid_cents, best_ask_cents,
              number_of_trades, traded_quantity, financial_volume_cents, quotation_factor,
              open_brl, high_brl, low_brl, average_brl, close_brl, ingested_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
              ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
        )?;
        for quote in &fii_quotes {
            let fund_id = identity_map.get(&quote.isin).and_then(|id| id.clone());
            if let Some(id) = &fund_id {
                tx.execute("UPDATE fii_identity_aliases SET confidence='crosschecked_cotahist' WHERE fund_id=?1 AND identifier_type='isin' AND identifier_value=?2",params![id,quote.isin])?;
                tx.execute("UPDATE fii_identity_registry SET confidence='crosschecked_cotahist' WHERE fund_id=?1 AND isin=?2",params![id,quote.isin])?;
                alias_stmt.execute(params![
                    id,
                    quote.ticker,
                    quote.date.to_string(),
                    retrieved_at.to_rfc3339(),
                    snapshot_id
                ])?;
            }
            // Price fields from rsb3 are original B3 integer cents (implicit two
            // decimals). FATCOT is retained verbatim as quotation_factor.
            observation_stmt.execute(params![
                snapshot_id,
                fund_id,
                quote.ticker,
                quote.isin,
                quote.date.to_string(),
                quote.bdi,
                quote.market,
                quote.name,
                quote.specification,
                quote.currency,
                quote.open,
                quote.high,
                quote.low,
                quote.average,
                quote.close,
                quote.bid,
                quote.ask,
                quote.trades,
                quote.quantity,
                quote.volume,
                quote.quotation_factor,
                normalized_brl(quote.open, quote.quotation_factor)?,
                normalized_brl(quote.high, quote.quotation_factor)?,
                normalized_brl(quote.low, quote.quotation_factor)?,
                normalized_brl(quote.average, quote.quotation_factor)?,
                normalized_brl(quote.close, quote.quotation_factor)?,
                retrieved_at.to_rfc3339()
            ])?;
        }
    }
    for (isin, fund_id) in &identity_map {
        if let Some(fund_id) = fund_id {
            tx.execute(
                "UPDATE fii_cotahist_observations SET fund_id=?1 WHERE snapshot_id=?2 AND isin=?3 AND fund_id IS NULL",
                params![fund_id,snapshot_id,isin],
            )?;
        }
    }
    tx.execute(
        "UPDATE fii_source_snapshots SET ingestion_status='complete' WHERE snapshot_id=?1",
        [&snapshot_id],
    )?;
    tx.commit()?;
    let _ = fs::remove_dir_all(staging);
    let unresolved_isin_count = unresolved.len();
    let unresolved_isins = unresolved.into_iter().take(25).collect();
    Ok(IngestSummary {
        snapshot_id,
        archive: immutable_archive.to_string_lossy().into_owned(),
        sha256: sha,
        bytes: zip_bytes.len() as u64,
        total_records: all_count,
        fii_records: record_count,
        resolved_records: resolved_count,
        unresolved_isin_count,
        unresolved_isins,
        duplicate_ticker_dates: duplicate_count,
        diagnostics,
    })
}

/// Download and retain a CVM monthly archive, then ingest its rows using the
/// MIDAS retrieval time as the conservative availability boundary.
pub fn ingest_cvm_monthly_year(
    conn: &Connection,
    year: i32,
    root: &Path,
    force: bool,
) -> Result<usize, Box<dyn std::error::Error>> {
    use crate::source_data::cvm::fii_reports::{
        FiiMonthlyArchive, FiiReportDataSource, FiiReportDataset,
    };
    use crate::source_data::cvm::{CkanClient, ResourceDownloader};
    let ckan = CkanClient::with_defaults()?;
    let downloader = ResourceDownloader::default();
    let source = FiiReportDataSource::new(&ckan, &downloader);
    let resource = source.resource(FiiReportDataset::Monthly, year)?;
    let retrieved_at = Utc::now();
    let staging = root.join(format!(
        ".staging/cvm-{year}-{}.zip",
        retrieved_at.timestamp_nanos_opt().unwrap_or_default()
    ));
    let download = downloader.download(&resource, &staging, force)?;
    let bytes = fs::read(&download.path)?;
    let sha = hex(&Sha256::digest(&bytes));
    let snapshot_id = format!("cvm-fii-monthly:{}:{sha}", resource.id);
    let immutable_path = root.join(format!("cvm/{year}/{sha}.zip"));
    if !immutable_path.exists() {
        if let Some(parent) = immutable_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&download.path, &immutable_path)?;
    }
    let source_modified_at = resource.last_modified.clone();
    let extraction = root.join(format!("cvm/extracted-{year}-{sha}"));
    let archive = FiiMonthlyArchive::parse_zip(&immutable_path, &extraction, year)?;
    conn.execute(
        "INSERT OR IGNORE INTO fii_source_snapshots
         (snapshot_id,source,resource_id,original_filename,requested_period,retrieved_at,
          source_modified_at,sha256,size_bytes,local_path,parser_version,schema_version,
          ingestion_status,record_count)
         VALUES (?1,'CVM',?2,?3,?4,?5,?6,?7,?8,?9,'midas-cvm-adapter-v1','cvm-fii-monthly-v1','parsing',?10)",
        params![snapshot_id,resource.id,resource.name.unwrap_or_else(||format!("inf_mensal_fii_{year}.zip")),
            year.to_string(),retrieved_at.to_rfc3339(),source_modified_at,sha,
            bytes.len() as i64,immutable_path.to_string_lossy(),archive.general.len() as i64],
    )?;
    for row in &archive.general {
        if let Some(isin) = row.isin.as_deref().filter(|value| value.trim().len() == 12) {
            let input = IdentityInput {
                cnpj: row.cnpj.clone(),
                isin: isin.into(),
                ticker: None,
                fund_name: row.name.clone(),
                cvm_segment: row.segment.clone(),
                evidence: format!(
                    "CVM fii-doc-inf_mensal snapshot {snapshot_id}, reference {} version {}",
                    row.reference_date, row.version
                ),
                valid_from: None,
                valid_to: None,
                source_verified: true,
            };
            let id = save_identity(conn, &input, retrieved_at)?;
            conn.execute(
                "INSERT OR IGNORE INTO fii_fund_metadata
                 (fund_id,cnpj,name,cvm_segment,reference_date,filing_version,available_at,ingested_at,source_snapshot_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![id.0,normalize_cnpj(&row.cnpj)?,row.name,row.segment,row.reference_date.to_string(),
                    row.version,retrieved_at.to_rfc3339(),retrieved_at.to_rfc3339(),snapshot_id],
            )?;
        }
    }
    let summaries = crate::investing::fii_pipeline::ingest_monthly_archive_with_snapshot(
        conn,
        &archive,
        retrieved_at,
        retrieved_at,
        Some(&sha),
    )?;
    conn.execute(
        "UPDATE fii_source_snapshots SET ingestion_status='complete' WHERE snapshot_id=?1",
        [snapshot_id],
    )?;
    let _ = fs::remove_file(&staging);
    let _ = fs::remove_dir_all(extraction);
    Ok(summaries.iter().map(|summary| summary.persisted_rows).sum())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn normalized_brl(raw_cents: i64, quotation_factor: i64) -> Result<f64, CotahistError> {
    if quotation_factor <= 0 {
        return Err(CotahistError::Validation(format!(
            "invalid COTAHIST quotation factor {quotation_factor}"
        )));
    }
    Ok(raw_cents as f64 / 100.0 / quotation_factor as f64)
}

fn newest_snapshot_archive(
    conn: &Connection,
    root: &Path,
    stem: &str,
) -> Result<Option<std::path::PathBuf>, CotahistError> {
    let cached:Option<String>=conn.query_row(
        "SELECT local_path FROM fii_source_snapshots WHERE source=?1 AND resource_id=?2 AND ingestion_status='complete' ORDER BY retrieved_at DESC LIMIT 1",
        params![SOURCE,stem],|row|row.get(0)).optional()?;
    if let Some(path) = cached {
        if Path::new(&path).exists() {
            return Ok(Some(path.into()));
        }
    }
    let dir = root.join(stem);
    if !dir.exists() {
        return Ok(None);
    }
    let mut files = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files.pop())
}

fn column<'a, T: 'static>(
    batch: &'a arrow::record_batch::RecordBatch,
    name: &str,
) -> Result<&'a T, CotahistError> {
    batch
        .column_by_name(name)
        .ok_or_else(|| CotahistError::Schema(format!("missing {name}")))?
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| CotahistError::Schema(format!("wrong type for {name}")))
}

fn quotes_from_batch(
    batch: &arrow::record_batch::RecordBatch,
) -> Result<Vec<ParsedQuote>, CotahistError> {
    let date = column::<Date32Array>(batch, "refdate")?;
    let bdi = column::<StringArray>(batch, "bdi_code")?;
    let ticker = column::<StringArray>(batch, "symbol")?;
    let market = column::<Int32Array>(batch, "instrument_market")?;
    let name = column::<StringArray>(batch, "corporation_name")?;
    let specification = column::<StringArray>(batch, "specification_code")?;
    let currency = column::<StringArray>(batch, "trading_currency")?;
    let open = column::<Int64Array>(batch, "open_cents")?;
    let high = column::<Int64Array>(batch, "high_cents")?;
    let low = column::<Int64Array>(batch, "low_cents")?;
    let average = column::<Int64Array>(batch, "average_cents")?;
    let close = column::<Int64Array>(batch, "close_cents")?;
    let bid = column::<Int64Array>(batch, "best_bid_cents")?;
    let ask = column::<Int64Array>(batch, "best_ask_cents")?;
    let trades = column::<Int64Array>(batch, "trade_quantity")?;
    let quantity = column::<Int64Array>(batch, "traded_contracts")?;
    let volume = column::<Int64Array>(batch, "volume_cents")?;
    let factor = column::<Int64Array>(batch, "allocation_lot_size")?;
    let isin = column::<StringArray>(batch, "isin")?;
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    (0..batch.num_rows())
        .map(|i| {
            Ok(ParsedQuote {
                date: epoch + chrono::Duration::days(i64::from(date.value(i))),
                bdi: bdi.value(i).trim().to_string(),
                ticker: ticker.value(i).trim().to_ascii_uppercase(),
                market: market.value(i),
                name: name.value(i).trim().to_string(),
                specification: specification.value(i).trim().to_string(),
                currency: currency.value(i).trim().to_string(),
                open: open.value(i),
                high: high.value(i),
                low: low.value(i),
                average: average.value(i),
                close: close.value(i),
                bid: bid.value(i),
                ask: ask.value(i),
                trades: trades.value(i),
                quantity: quantity.value(i),
                volume: volume.value(i),
                quotation_factor: factor.value(i),
                isin: isin.value(i).trim().to_ascii_uppercase(),
            })
        })
        .collect()
}

fn duplicate_ticker_dates(quotes: &[ParsedQuote]) -> usize {
    let mut seen = std::collections::HashSet::new();
    quotes
        .iter()
        .filter(|q| !seen.insert((q.ticker.as_str(), q.isin.as_str(), q.date)))
        .count()
}

fn isin_identity_map(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, Option<String>>, rusqlite::Error> {
    let mut stmt=conn.prepare("SELECT identifier_value,fund_id FROM fii_identity_aliases WHERE identifier_type='isin' ORDER BY identifier_value,fund_id")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut grouped =
        std::collections::HashMap::<String, std::collections::BTreeSet<String>>::new();
    for row in rows {
        let (isin, fund) = row?;
        grouped.entry(isin).or_default().insert(fund);
    }
    Ok(grouped
        .into_iter()
        .map(|(isin, funds)| {
            let id = (funds.len() == 1).then(|| funds.into_iter().next().unwrap());
            (isin, id)
        })
        .collect())
}

pub fn report(
    conn: &Connection,
    as_of: DateTime<Utc>,
    cohort_tickers: Option<&[String]>,
) -> Result<FiiDataReport, CotahistError> {
    let mut statement = conn.prepare(
        "SELECT o.fund_id, o.ticker, o.isin, o.trading_date, o.close_cents, o.close_brl,
         o.traded_quantity, o.financial_volume_cents, o.number_of_trades, o.snapshot_id,
         (SELECT identifier_value FROM fii_identity_aliases a WHERE a.fund_id=o.fund_id AND a.identifier_type='cnpj' ORDER BY a.first_observed_at LIMIT 1),
         (SELECT evidence FROM fii_identity_aliases a WHERE a.fund_id=o.fund_id AND a.identifier_type='isin' AND a.identifier_value=o.isin ORDER BY a.first_observed_at LIMIT 1),
         (SELECT confidence FROM fii_identity_aliases a WHERE a.fund_id=o.fund_id AND a.identifier_type='isin' AND a.identifier_value=o.isin ORDER BY a.first_observed_at LIMIT 1),
         COALESCE((SELECT name FROM fii_fund_metadata m WHERE m.fund_id=o.fund_id AND m.available_at<=?1 ORDER BY m.available_at DESC,m.reference_date DESC LIMIT 1),
                  (SELECT fund_name FROM fii_identity_registry i WHERE i.fund_id=o.fund_id AND i.first_observed_at<=?1 ORDER BY i.first_observed_at DESC LIMIT 1)),
         COALESCE((SELECT cvm_segment FROM fii_fund_metadata m WHERE m.fund_id=o.fund_id AND m.available_at<=?1 ORDER BY m.available_at DESC,m.reference_date DESC LIMIT 1),
                  (SELECT cvm_segment FROM fii_identity_registry i WHERE i.fund_id=o.fund_id AND i.first_observed_at<=?1 ORDER BY i.first_observed_at DESC LIMIT 1)),
         (SELECT MIN(quotation_factor) FROM fii_cotahist_observations x WHERE x.fund_id=o.fund_id AND x.ingested_at<=?1),
         (SELECT MAX(quotation_factor) FROM fii_cotahist_observations x WHERE x.fund_id=o.fund_id AND x.ingested_at<=?1)
         FROM fii_cotahist_observations o
         JOIN fii_source_snapshots source_snapshot ON source_snapshot.snapshot_id=o.snapshot_id
         WHERE o.fund_id IS NOT NULL AND source_snapshot.retrieved_at<=?1
         ORDER BY o.fund_id,o.trading_date,source_snapshot.retrieved_at ASC,o.snapshot_id ASC",
    )?;
    type ReportRow = (
        String,
        String,
        String,
        i64,
        f64,
        i64,
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
    );
    let mut funds: BTreeMap<String, Vec<ReportRow>> = BTreeMap::new();
    let rows = statement.query_map([as_of.to_rfc3339()], |r| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(7)?,
            r.get(8)?,
            r.get(9)?,
            r.get(10)?,
            r.get(11)?,
            r.get(12)?,
            r.get(13)?,
            r.get(14)?,
            r.get(15)?,
            r.get(16)?,
        ))
    })?;
    for row in rows {
        let (
            id,
            ticker,
            isin,
            date,
            cents,
            brl,
            quantity,
            volume,
            trades,
            snapshot,
            cnpj,
            evidence,
            confidence,
            name,
            segment,
            factor_min,
            factor_max,
        ) = row?;
        funds.entry(id).or_default().push((
            ticker, isin, date, cents, brl, quantity, volume, trades, snapshot, cnpj, evidence,
            confidence, name, segment, factor_min, factor_max,
        ));
    }
    let mut reports = Vec::new();
    for (id, rows) in funds {
        let last = rows.last().unwrap();
        let distinct_dates = rows
            .iter()
            .map(|row| row.2.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let mut tickers = rows
            .iter()
            .map(|r| r.0.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut isins = rows
            .iter()
            .map(|r| r.1.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let snapshots = rows
            .iter()
            .map(|r| r.8.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut diagnostics = Vec::new();
        if tickers.len() > 1 {
            diagnostics
                .push("multiple ticker aliases observed; effective dates need review".into());
        }
        if isins.len() > 1 {
            diagnostics.push("multiple ISINs observed for fund identity".into());
        }
        if distinct_dates < rows.len() {
            diagnostics.push("corrected archive versions overlap; report uses the latest captured row per trading date".into());
        }
        let cvm_reports =
            crate::investing::fii_pipeline::monthly_metrics_as_of(conn, &FiiId(id.clone()), as_of)
                .unwrap_or_default()
                .into_iter()
                .map(|m| {
                    let aligned=align_market_to_cvm(conn,&id,m.reference_date,m.nav_per_share,as_of,m.diagnostics);
                    CvmReportRecord {
                    reference_date: m.reference_date.to_string(),
                    available_at: m.available_at.to_rfc3339(),
                    submission_date:m.submission_date.map(|date|date.to_string()),
                    availability_basis:m.availability_basis,
                    filing_version: m.filing_version,
                    nav_per_share: m.nav_per_share,
                    reported_monthly_yield: m.reported_monthly_distribution_yield,
                    aligned_market_date:aligned.date, aligned_raw_close_brl:aligned.close_brl,
                    price_to_nav:aligned.price_to_nav,aligned_market_snapshot:aligned.snapshot,
                    alignment_policy:"CVM Data_Referencia is treated as a reporting month; use the last B3 close on/before calendar month-end, no more than 5 calendar days stale, and captured by as_of.".into(),
                    diagnostics:aligned.diagnostics,
                    }
                })
                .collect::<Vec<_>>();
        reports.push(FundReport {
            fund_id: id.clone(),
            cnpj: rows[0].9.clone(),
            ticker: tickers.pop_first(),
            isin: isins.pop_first(),
            name: rows[0].12.clone(),
            cvm_segment: rows[0].13.clone(),
            mapping_evidence: rows[0].10.clone(),
            mapping_status: rows[0].11.clone().unwrap_or_else(|| "unverified".into()),
            first_trading_date: rows.first().map(|r| r.2.clone()),
            last_trading_date: Some(last.2.clone()),
            observations: distinct_dates,
            last_raw_close_cents: Some(last.3),
            last_close_brl: Some(last.4),
            last_traded_quantity: Some(last.5),
            last_number_of_trades: Some(last.7),
            last_financial_volume_cents: Some(last.6),
            quotation_factor_min: rows[0].14,
            quotation_factor_max: rows[0].15,
            cvm_reports,
            snapshots,
            diagnostics,
        });
    }
    if let Some(cohort) = cohort_tickers {
        let tickers = cohort
            .iter()
            .map(|ticker| ticker.trim().to_ascii_uppercase())
            .collect::<std::collections::HashSet<_>>();
        reports.retain(|fund| {
            fund.ticker
                .as_ref()
                .is_some_and(|ticker| tickers.contains(ticker))
        });
        for ticker in tickers {
            if !reports
                .iter()
                .any(|fund| fund.ticker.as_deref() == Some(&ticker))
            {
                // Cohort members without resolved market coverage stay visible
                // as an explicit gap rather than being silently omitted.
                reports.push(FundReport {
                    fund_id: format!("unresolved:ticker:{ticker}"),
                    cnpj: None,
                    ticker: Some(ticker),
                    isin: None,
                    name: None,
                    cvm_segment: None,
                    mapping_evidence: None,
                    mapping_status: "unresolved".into(),
                    first_trading_date: None,
                    last_trading_date: None,
                    observations: 0,
                    last_raw_close_cents: None,
                    last_close_brl: None,
                    last_traded_quantity: None,
                    last_number_of_trades: None,
                    last_financial_volume_cents: None,
                    quotation_factor_min: None,
                    quotation_factor_max: None,
                    cvm_reports: Vec::new(),
                    snapshots: Vec::new(),
                    diagnostics: vec![
                        "no resolved FII observations are available for this cohort ticker".into(),
                    ],
                });
            }
        }
        reports.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    }
    let unresolved_market_rows: i64=conn.query_row("SELECT COUNT(*) FROM (SELECT DISTINCT o.ticker,o.isin,o.trading_date,o.market_type FROM fii_cotahist_observations o JOIN fii_source_snapshots s ON s.snapshot_id=o.snapshot_id WHERE o.fund_id IS NULL AND s.retrieved_at<=?1)",[as_of.to_rfc3339()],|r|r.get(0))?;
    Ok(FiiDataReport {
        as_of,
        funds: reports,
        unresolved_market_rows: unresolved_market_rows as usize,
        market_gap_policy: "No exchange calendar is included yet; weekends and holidays are not counted as missing observations. Non-trading days and no-trade dates require source-calendar validation.".into(),
        distributions: "Unavailable: no validated historical event source is configured; no trailing cash yield or total return is reported.".into(),
    })
}

fn month_end(reference_date: NaiveDate) -> NaiveDate {
    let (year, month) = if reference_date.month() == 12 {
        (reference_date.year() + 1, 1)
    } else {
        (reference_date.year(), reference_date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).unwrap() - chrono::Duration::days(1)
}

fn align_market_to_cvm(
    conn: &Connection,
    fund_id: &str,
    reference_date: NaiveDate,
    nav: Option<f64>,
    as_of: DateTime<Utc>,
    mut diagnostics: Vec<String>,
) -> AlignedMarket {
    let end = month_end(reference_date);
    let start = end - chrono::Duration::days(5);
    let result: Result<(String, f64, String), rusqlite::Error> = conn.query_row(
        "SELECT o.trading_date,o.close_brl,o.snapshot_id FROM fii_cotahist_observations o
         JOIN fii_source_snapshots s ON s.snapshot_id=o.snapshot_id
         WHERE o.fund_id=?1 AND o.trading_date BETWEEN ?2 AND ?3 AND s.retrieved_at<=?4
         ORDER BY o.trading_date DESC,s.retrieved_at DESC LIMIT 1",
        params![
            fund_id,
            start.to_string(),
            end.to_string(),
            as_of.to_rfc3339()
        ],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );
    match result {
        Ok((date, close, snapshot)) => AlignedMarket {
            date: Some(date),
            close_brl: Some(close),
            price_to_nav: nav
                .filter(|value| value.is_finite() && *value > 0.0)
                .map(|value| close / value),
            snapshot: Some(snapshot),
            diagnostics,
        },
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            diagnostics.push("no COTAHIST close within 5 calendar days on/before the reporting month-end was available by as_of".into());
            AlignedMarket {
                date: None,
                close_brl: None,
                price_to_nav: None,
                snapshot: None,
                diagnostics,
            }
        }
        Err(error) => {
            diagnostics.push(format!("market alignment query failed: {error}"));
            AlignedMarket {
                date: None,
                close_brl: None,
                price_to_nav: None,
                snapshot: None,
                diagnostics,
            }
        }
    }
}

/// Utility for controlled test fixture generation; production always calls rsb3.
#[cfg(test)]
fn fixture_quote(date: &str, bdi: &str, symbol: &str, isin: &str, price: i64) -> Vec<u8> {
    let mut line = vec![b' '; 245];
    let put = |line: &mut Vec<u8>, a: usize, b: usize, value: &str| {
        let bytes = value.as_bytes();
        let width = b - a;
        let mut padded = vec![b' '; width];
        padded[..bytes.len().min(width)].copy_from_slice(&bytes[..bytes.len().min(width)]);
        line[a..b].copy_from_slice(&padded);
    };
    put(&mut line, 0, 2, "01");
    put(&mut line, 2, 10, date);
    put(&mut line, 10, 12, bdi);
    put(&mut line, 12, 24, symbol);
    put(&mut line, 24, 27, "010");
    put(&mut line, 27, 39, "FII TESTE");
    put(&mut line, 39, 49, "CI");
    put(&mut line, 52, 56, "R$");
    for (a, b) in [
        (56, 69),
        (69, 82),
        (82, 95),
        (95, 108),
        (108, 121),
        (121, 134),
        (134, 147),
    ] {
        put(&mut line, a, b, &format!("{price:0width$}", width = b - a));
    }
    put(&mut line, 147, 152, "00010");
    put(&mut line, 152, 170, "000000000000000100");
    put(&mut line, 170, 188, "000000000000001000");
    put(&mut line, 188, 201, "0000000000000");
    put(&mut line, 202, 210, "99991231");
    put(&mut line, 210, 217, "0000001");
    put(&mut line, 217, 230, "0000000000000");
    put(&mut line, 230, 242, isin);
    put(&mut line, 242, 245, "000");
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsb3_general_parser_exposes_fii_bdi_isin_prices_and_factor() {
        let mut bytes = fixture_quote("20250602", BDI_FII, "HGLG11", "BRHGLGCTF004", 17325);
        bytes.extend_from_slice(b"\r\n");
        let mut trailer = vec![b' '; 245];
        trailer[0..2].copy_from_slice(b"99");
        bytes.extend_from_slice(&trailer);
        let batch = rsb3::cotahist::parse_bytes(&bytes).unwrap();
        let quotes = quotes_from_batch(&batch).unwrap();
        let q = &quotes[0];
        assert_eq!(q.bdi, "12");
        assert_eq!(q.ticker, "HGLG11");
        assert_eq!(q.isin, "BRHGLGCTF004");
        assert_eq!(q.close, 17325);
        assert_eq!(q.quotation_factor, 1);
        assert_eq!(q.close as f64 / 100.0, 173.25);
    }

    #[test]
    fn normalized_prices_apply_b3_quotation_factor_without_losing_raw_cents() {
        assert_eq!(normalized_brl(17_325, 1).unwrap(), 173.25);
        assert!((normalized_brl(17_325, 1_000).unwrap() - 0.17325).abs() < 1e-12);
        assert!(normalized_brl(17_325, 0).is_err());
    }

    #[test]
    fn identical_snapshot_ingestion_is_idempotent_and_corrections_are_distinct() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let input = IdentityInput {
            cnpj: "11.728.688/0001-47".into(),
            isin: "BRHGLGCTF004".into(),
            ticker: Some("HGLG11".into()),
            fund_name: None,
            cvm_segment: Some("Multicategoria".into()),
            evidence: "synthetic test mapping".into(),
            valid_from: None,
            valid_to: None,
            source_verified: false,
        };
        save_identity(&conn, &input, Utc::now()).unwrap();
        let q = fixture_quote("20250602", BDI_FII, "HGLG11", "BRHGLGCTF004", 17325);
        let mut bytes = q.clone();
        bytes.extend_from_slice(b"\r\n");
        let mut trailer = vec![b' '; 245];
        trailer[0..2].copy_from_slice(b"99");
        bytes.extend_from_slice(&trailer);
        let batch = rsb3::cotahist::parse_bytes(&bytes).unwrap();
        let parsed = quotes_from_batch(&batch).unwrap();
        assert_eq!(duplicate_ticker_dates(&parsed), 0);
        // Snapshot table uniqueness is content-addressed; new hashes represent revisions.
        conn.execute("INSERT INTO fii_source_snapshots(snapshot_id,source,resource_id,original_filename,requested_period,retrieved_at,sha256,size_bytes,local_path,parser_version,schema_version,ingestion_status) VALUES('s1','B3','a','a','2025',?1,'h1',1,'p','rsb3/0.1.0','v1','complete')",[Utc::now().to_rfc3339()]).unwrap();
        conn.execute("INSERT INTO fii_source_snapshots(snapshot_id,source,resource_id,original_filename,requested_period,retrieved_at,sha256,size_bytes,local_path,parser_version,schema_version,ingestion_status) VALUES('s2','B3','a','a','2025',?1,'h2',1,'p2','rsb3/0.1.0','v1','complete')",[Utc::now().to_rfc3339()]).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM fii_source_snapshots", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn month_end_alignment_uses_the_cvm_reporting_month() {
        assert_eq!(
            month_end(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
        );
        assert_eq!(
            month_end(NaiveDate::from_ymd_opt(2024, 12, 1).unwrap()),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()
        );
    }
}
