//! CVM monthly FII report mapping, validation, persistence, and point-in-time reads.
use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    db::fii::{self, FiiStoreError},
    investing::fii::{Fii, FiiId, FiiMonthlyMetrics, FilingObservation, MarketObservation},
    source_data::cvm::fii_reports::{
        FiiMonthlyArchive, FiiMonthlyAssetsRow, FiiMonthlyComplementRow, FiiMonthlyGeneralRow,
        FiiMonthlyReport, FiiReportDataSource,
    },
    source_data::cvm::{CkanClient, CvmError, ResourceDownloader},
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn archive(version: &str, submission: Option<NaiveDate>) -> FiiMonthlyArchive {
        let general = FiiMonthlyGeneralRow {
            fund_type: Some("Fundo".into()),
            cnpj: "00.332.266/0001-31".into(),
            reference_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            version: version.into(),
            submission_date: submission,
            name: Some("FII Teste".into()),
            operation_start_date: None,
            isin: Some("BRTESTCTF000".into()),
            mandate: Some("Renda".into()),
            segment: Some("Shoppings".into()),
            management_type: Some("Passiva".into()),
            administrator: Some("Administradora".into()),
            administrator_cnpj: Some("72600026000181".into()),
            extra: HashMap::new(),
        };
        let complement = FiiMonthlyComplementRow {
            cnpj: general.cnpj.clone(),
            reference_date: general.reference_date,
            version: general.version.clone(),
            investor_count_date: Some(NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()),
            values: HashMap::from([
                ("Patrimonio_Liquido".into(), "487055921.63".into()),
                ("Cotas_Emitidas".into(), "2800149".into()),
                ("Valor_Patrimonial_Cotas".into(), "173.93928738435".into()),
                ("Percentual_Dividend_Yield_Mes".into(), "0.002871".into()),
            ]),
        };
        FiiMonthlyArchive {
            general: vec![general],
            assets: Vec::new(),
            complement: vec![complement],
        }
    }

    #[test]
    fn cnpj_normalization_keeps_a_stable_formatted_identifier() {
        assert_eq!(
            normalize_cnpj("00.332.266/0001-31").unwrap(),
            "00332266000131"
        );
        assert!(normalize_cnpj("too-short").is_err());
    }

    #[test]
    fn cvm_monthly_archive_runs_through_ingestion_point_in_time_and_nav_indicators() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::fii::init(&conn).unwrap();
        let snapshot_available = Utc.with_ymd_and_hms(2025, 2, 20, 12, 0, 0).unwrap();
        let ingested = Utc.with_ymd_and_hms(2025, 2, 21, 12, 0, 0).unwrap();
        let archive = archive("1", Some(NaiveDate::from_ymd_opt(2025, 2, 17).unwrap()));
        let summary =
            ingest_monthly_archive(&conn, &archive, snapshot_available, ingested).unwrap();
        assert_eq!(summary[0].persisted_rows, 2);
        assert_eq!(summary[0].unmatched_asset_rows, 0);

        let id = FiiId("cnpj:00332266000131".into());
        let before_snapshot = Utc.with_ymd_and_hms(2025, 2, 20, 11, 59, 59).unwrap();
        assert!(
            monthly_metrics_as_of(&conn, &id, before_snapshot)
                .unwrap()
                .is_empty()
        );
        let as_of = Utc.with_ymd_and_hms(2025, 2, 22, 0, 0, 0).unwrap();
        let mut metrics = monthly_metrics_as_of(&conn, &id, as_of).unwrap().remove(0);
        assert_eq!(metrics.fund.cnpj.as_deref(), Some("00332266000131"));
        assert_eq!(metrics.nav_per_share, Some(173.93928738435));
        assert_eq!(metrics.reported_monthly_distribution_yield, Some(0.002871));

        let market = MarketObservation {
            fund_id: id,
            reference_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
            available_at: Utc.with_ymd_and_hms(2025, 2, 3, 12, 0, 0).unwrap(),
            ingested_at: ingested,
            source: "fixture_market".into(),
            revision_id: "fixture-1".into(),
            raw_price: Some(170.0),
            adjusted_price: None,
            trading_volume: None,
            financial_volume: None,
            market_cap: None,
            shares_outstanding: None,
        };
        apply_market_observation(&mut metrics, &market, as_of);
        assert!((metrics.price_to_nav.unwrap() - 170.0 / 173.93928738435).abs() < 1e-12);
    }

    #[test]
    fn missing_submission_date_uses_archive_snapshot_with_diagnostic() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::fii::init(&conn).unwrap();
        let archive = archive("1", None);
        let snapshot_available = Utc.with_ymd_and_hms(2025, 2, 20, 12, 0, 0).unwrap();
        ingest_monthly_archive(
            &conn,
            &archive,
            snapshot_available,
            Utc.with_ymd_and_hms(2025, 2, 21, 12, 0, 0).unwrap(),
        )
        .unwrap();
        let rows = monthly_metrics_as_of(
            &conn,
            &FiiId("cnpj:00332266000131".into()),
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(rows[0].available_at, snapshot_available);
        assert!(
            rows[0]
                .diagnostics
                .iter()
                .any(|item| item.contains("Data_Entrega is missing"))
        );
    }

    #[test]
    fn ambiguous_fund_and_class_rows_are_preserved_without_guessing_companion_join() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::fii::init(&conn).unwrap();
        let mut source = archive("1", None);
        let mut class_row = source.general[0].clone();
        class_row.fund_type = Some("Classe".into());
        class_row.name = Some("FII Teste - Classe Única".into());
        source.general.push(class_row);
        let available = Utc::now();
        let summaries = ingest_monthly_archive(&conn, &source, available, available).unwrap();
        assert_eq!(summaries[0].persisted_rows, 2);
        assert_eq!(summaries[0].unmatched_complement_rows, 1);
        let metrics = monthly_metrics_as_of(&conn, &summaries[0].fund_id, available).unwrap();
        assert!(metrics.iter().any(|row| {
            row.diagnostics
                .iter()
                .any(|item| item.contains("source key is ambiguous"))
        }));
    }
}

const SOURCE: &str = "CVM";
const GENERAL_DATASET: &str = "fii_inf_mensal_geral";
const ASSETS_DATASET: &str = "fii_inf_mensal_ativo_passivo";
const COMPLEMENT_DATASET: &str = "fii_inf_mensal_complemento";

#[derive(Debug, Error)]
pub enum FiiPipelineError {
    #[error("FII filing is invalid: {0}")]
    InvalidFiling(String),
    #[error("FII filing serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Cvm(#[from] CvmError),
    #[error(transparent)]
    Store(#[from] FiiStoreError),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionSummary {
    pub fund_id: FiiId,
    pub persisted_rows: usize,
    pub unmatched_asset_rows: usize,
    pub unmatched_complement_rows: usize,
}

/// Download one CKAN-backed annual CVM archive and persist its monthly FII rows.
/// `available_at` is the CKAN `last_modified` for the exact archive snapshot;
/// the row-level `Data_Entrega` is preserved in the source payload separately.
pub fn ingest_monthly_year(
    conn: &Connection,
    ckan: &CkanClient,
    downloader: &ResourceDownloader,
    year: i32,
    archive_path: impl AsRef<std::path::Path>,
    extraction_dir: impl AsRef<std::path::Path>,
    force_download: bool,
    ingested_at: DateTime<Utc>,
) -> Result<Vec<IngestionSummary>, FiiPipelineError> {
    let source = FiiReportDataSource::new(ckan, downloader);
    let snapshot = source.download_snapshot(
        crate::source_data::cvm::fii_reports::FiiReportDataset::Monthly,
        year,
        archive_path,
        force_download,
    )?;
    let archive = FiiMonthlyArchive::parse_zip(snapshot.download.path, extraction_dir, year)?;
    ingest_monthly_archive(conn, &archive, snapshot.available_at, ingested_at)
}

/// Normalize formatted or unformatted numeric CNPJs without guessing a fund identity.
pub fn normalize_cnpj(raw: &str) -> Result<String, FiiPipelineError> {
    let normalized = raw
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    if normalized.len() != 14 || !normalized.is_ascii() {
        return Err(FiiPipelineError::InvalidFiling(format!(
            "CNPJ must contain 14 ASCII letters or digits: {raw:?}"
        )));
    }
    Ok(normalized)
}

fn report_key(cnpj: &str, reference_date: NaiveDate, version: &str) -> (String, NaiveDate, String) {
    (
        cnpj.chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_uppercase(),
        reference_date,
        version.trim().to_owned(),
    )
}

fn validate_general(row: &FiiMonthlyGeneralRow) -> Result<FiiId, FiiPipelineError> {
    if row.version.trim().is_empty() {
        return Err(FiiPipelineError::InvalidFiling(
            "CVM filing version is missing".into(),
        ));
    }
    Ok(FiiId(format!("cnpj:{}", normalize_cnpj(&row.cnpj)?)))
}

pub fn ingest_monthly_archive(
    conn: &Connection,
    archive: &FiiMonthlyArchive,
    source_snapshot_available_at: DateTime<Utc>,
    ingested_at: DateTime<Utc>,
) -> Result<Vec<IngestionSummary>, FiiPipelineError> {
    ingest_monthly_archive_with_snapshot(
        conn,
        archive,
        source_snapshot_available_at,
        ingested_at,
        None,
    )
}

/// Snapshot-aware importer. Including the artifact hash in the revision key
/// preserves corrected archives even when CVM keeps its filing version number.
pub fn ingest_monthly_archive_with_snapshot(
    conn: &Connection,
    archive: &FiiMonthlyArchive,
    source_snapshot_available_at: DateTime<Utc>,
    ingested_at: DateTime<Utc>,
    snapshot_revision: Option<&str>,
) -> Result<Vec<IngestionSummary>, FiiPipelineError> {
    conn.execute_batch("SAVEPOINT fii_monthly_ingest")?;
    let result = ingest_monthly_archive_inner(
        conn,
        archive,
        source_snapshot_available_at,
        ingested_at,
        snapshot_revision,
    );
    match result {
        Ok(summaries) => {
            conn.execute_batch("RELEASE SAVEPOINT fii_monthly_ingest")?;
            Ok(summaries)
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT fii_monthly_ingest; RELEASE SAVEPOINT fii_monthly_ingest",
            );
            Err(error)
        }
    }
}

fn ingest_monthly_archive_inner(
    conn: &Connection,
    archive: &FiiMonthlyArchive,
    source_snapshot_available_at: DateTime<Utc>,
    ingested_at: DateTime<Utc>,
    snapshot_revision: Option<&str>,
) -> Result<Vec<IngestionSummary>, FiiPipelineError> {
    let mut by_fund: BTreeMap<FiiId, IngestionSummary> = BTreeMap::new();
    let mut matched_assets = HashMap::new();
    let mut matched_complement = HashMap::new();
    let assets_by_key = archive
        .assets
        .iter()
        .map(|row| (report_key(&row.cnpj, row.reference_date, &row.version), row))
        .collect::<HashMap<_, _>>();
    let complement_by_key = archive
        .complement
        .iter()
        .map(|row| (report_key(&row.cnpj, row.reference_date, &row.version), row))
        .collect::<HashMap<_, _>>();
    let general_key_counts = archive.general.iter().fold(
        HashMap::<(String, NaiveDate, String), usize>::new(),
        |mut counts, row| {
            *counts
                .entry(report_key(&row.cnpj, row.reference_date, &row.version))
                .or_default() += 1;
            counts
        },
    );

    for general in &archive.general {
        let fund_id = validate_general(general)?;
        let key = report_key(&general.cnpj, general.reference_date, &general.version);
        let summary = by_fund
            .entry(fund_id.clone())
            .or_insert_with(|| IngestionSummary {
                fund_id: fund_id.clone(),
                persisted_rows: 0,
                unmatched_asset_rows: 0,
                unmatched_complement_rows: 0,
            });
        let available_at = source_snapshot_available_at;
        let filing_version = Some(general.version.clone());
        let general_payload = serde_json::to_string(general)?;
        let row_revision = if general_key_counts.get(&key).copied().unwrap_or_default() > 1 {
            let digest = Sha256::digest(general_payload.as_bytes());
            format!(
                "{}:row:{}",
                general.version,
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            )
        } else {
            general.version.clone()
        };
        let revision_id = snapshot_revision
            .map(|snapshot| format!("{row_revision}:snapshot:{snapshot}"))
            .unwrap_or(row_revision);
        fii::save_filing(
            conn,
            &FilingObservation {
                fund_id: fund_id.clone(),
                reference_date: general.reference_date,
                available_at,
                ingested_at,
                source: SOURCE.into(),
                dataset: GENERAL_DATASET.into(),
                filing_version: filing_version.clone(),
                revision_id: revision_id.clone(),
                payload_json: general_payload,
            },
        )?;
        summary.persisted_rows += 1;

        let can_join_detail = general_key_counts.get(&key).copied().unwrap_or_default() == 1;
        if can_join_detail {
            if let Some(assets) = assets_by_key.get(&key) {
                matched_assets.insert(key.clone(), true);
                fii::save_filing(
                    conn,
                    &FilingObservation {
                        fund_id: fund_id.clone(),
                        reference_date: general.reference_date,
                        available_at,
                        ingested_at,
                        source: SOURCE.into(),
                        dataset: ASSETS_DATASET.into(),
                        filing_version: filing_version.clone(),
                        revision_id: revision_id.clone(),
                        payload_json: serde_json::to_string(assets)?,
                    },
                )?;
                summary.persisted_rows += 1;
            }
        }
        if can_join_detail {
            if let Some(complement) = complement_by_key.get(&key) {
                matched_complement.insert(key, true);
                fii::save_filing(
                    conn,
                    &FilingObservation {
                        fund_id: fund_id.clone(),
                        reference_date: general.reference_date,
                        available_at,
                        ingested_at,
                        source: SOURCE.into(),
                        dataset: COMPLEMENT_DATASET.into(),
                        filing_version,
                        revision_id: revision_id.clone(),
                        payload_json: serde_json::to_string(complement)?,
                    },
                )?;
                summary.persisted_rows += 1;
            }
        }
    }

    for row in &archive.assets {
        let key = report_key(&row.cnpj, row.reference_date, &row.version);
        if !matched_assets.contains_key(&key) {
            let fund_id = FiiId(format!("cnpj:{}", normalize_cnpj(&row.cnpj)?));
            by_fund
                .entry(fund_id.clone())
                .or_insert_with(|| IngestionSummary {
                    fund_id,
                    persisted_rows: 0,
                    unmatched_asset_rows: 0,
                    unmatched_complement_rows: 0,
                })
                .unmatched_asset_rows += 1;
        }
    }
    for row in &archive.complement {
        let key = report_key(&row.cnpj, row.reference_date, &row.version);
        if !matched_complement.contains_key(&key) {
            let fund_id = FiiId(format!("cnpj:{}", normalize_cnpj(&row.cnpj)?));
            by_fund
                .entry(fund_id.clone())
                .or_insert_with(|| IngestionSummary {
                    fund_id,
                    persisted_rows: 0,
                    unmatched_asset_rows: 0,
                    unmatched_complement_rows: 0,
                })
                .unmatched_complement_rows += 1;
        }
    }
    Ok(by_fund.into_values().collect())
}

fn monthly_reports_as_of(
    conn: &Connection,
    fund_id: &FiiId,
    as_of: DateTime<Utc>,
) -> Result<Vec<(FiiMonthlyReport, DateTime<Utc>, DateTime<Utc>)>, FiiPipelineError> {
    let filings = fii::filings_as_of(conn, fund_id, as_of)?;
    let mut grouped: BTreeMap<
        (NaiveDate, String),
        (
            Option<FiiMonthlyGeneralRow>,
            Option<FiiMonthlyAssetsRow>,
            Option<FiiMonthlyComplementRow>,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    > = BTreeMap::new();
    for filing in filings {
        if filing.source != SOURCE {
            continue;
        }
        let key = (filing.reference_date, filing.revision_id.clone());
        let entry = grouped.entry(key).or_insert((
            None,
            None,
            None,
            filing.available_at,
            filing.ingested_at,
        ));
        entry.3 = entry.3.max(filing.available_at);
        entry.4 = entry.4.max(filing.ingested_at);
        match filing.dataset.as_str() {
            GENERAL_DATASET => entry.0 = Some(serde_json::from_str(&filing.payload_json)?),
            ASSETS_DATASET => entry.1 = Some(serde_json::from_str(&filing.payload_json)?),
            COMPLEMENT_DATASET => entry.2 = Some(serde_json::from_str(&filing.payload_json)?),
            _ => {}
        }
    }
    let mut by_reference =
        BTreeMap::<NaiveDate, (String, FiiMonthlyReport, DateTime<Utc>, DateTime<Utc>)>::new();
    for ((reference_date, version), (general, assets, complement, available_at, ingested_at)) in
        grouped
    {
        if let Some(general) = general {
            let candidate = (
                version,
                FiiMonthlyReport {
                    general,
                    assets,
                    complement,
                },
                available_at,
                ingested_at,
            );
            let replace = by_reference.get(&reference_date).is_none_or(|current| {
                candidate.2 > current.2
                    || (candidate.2 == current.2
                        && compare_versions(&candidate.0, &current.0).is_gt())
            });
            if replace {
                by_reference.insert(reference_date, candidate);
            }
        }
    }
    Ok(by_reference
        .into_values()
        .map(|(_, report, available_at, ingested_at)| (report, available_at, ingested_at))
        .collect())
}

fn parse_finite(raw: Option<&str>) -> Option<f64> {
    raw?.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

pub fn monthly_metrics_as_of(
    conn: &Connection,
    fund_id: &FiiId,
    as_of: DateTime<Utc>,
) -> Result<Vec<FiiMonthlyMetrics>, FiiPipelineError> {
    let mut duplicate_versions = HashMap::<(NaiveDate, String), usize>::new();
    for filing in fii::filings_as_of(conn, fund_id, as_of)? {
        if filing.source == SOURCE && filing.dataset == GENERAL_DATASET {
            *duplicate_versions
                .entry((
                    filing.reference_date,
                    filing.filing_version.unwrap_or_default(),
                ))
                .or_default() += 1;
        }
    }
    let mut results = Vec::new();
    for (report, available_at, ingested_at) in monthly_reports_as_of(conn, fund_id, as_of)? {
        let mut diagnostics = Vec::new();
        if duplicate_versions
            .get(&(
                report.general.reference_date,
                report.general.version.clone(),
            ))
            .copied()
            .unwrap_or_default()
            > 1
        {
            diagnostics.push("multiple CVM general rows share this CNPJ, reference period, and version; companion tables are not joined because the source key is ambiguous".into());
        }
        diagnostics.push(if report.general.submission_date.is_some() {
            "Data_Entrega is a date only; availability uses the first MIDAS capture timestamp"
                .into()
        } else {
            "Data_Entrega is missing; availability uses the first MIDAS capture timestamp".into()
        });
        let complement = report.complement.as_ref();
        let nav_per_share = complement
            .and_then(|row| {
                parse_finite(
                    row.values
                        .get("Valor_Patrimonial_Cotas")
                        .map(String::as_str),
                )
            })
            .filter(|value| *value > 0.0)
            .or_else(|| {
                let equity = complement.and_then(|row| {
                    parse_finite(row.values.get("Patrimonio_Liquido").map(String::as_str))
                });
                let shares = complement.and_then(|row| {
                    parse_finite(row.values.get("Cotas_Emitidas").map(String::as_str))
                });
                match (equity, shares) {
                    (Some(equity), Some(shares)) if equity > 0.0 && shares > 0.0 => {
                        Some(equity / shares)
                    }
                    _ => None,
                }
            });
        if complement.is_none() {
            diagnostics.push("monthly complement table is missing".into());
        }
        if nav_per_share.is_none() {
            diagnostics.push("NAV per share is unavailable or invalid".into());
        }
        let reported_monthly_distribution_yield = complement.and_then(|row| {
            parse_finite(
                row.values
                    .get("Percentual_Dividend_Yield_Mes")
                    .map(String::as_str),
            )
        });
        if reported_monthly_distribution_yield.is_none() {
            diagnostics.push(
                "source-reported monthly distribution yield is unavailable or invalid".into(),
            );
        }
        let cnpj = normalize_cnpj(&report.general.cnpj)?;
        results.push(FiiMonthlyMetrics {
            fund: Fii {
                id: fund_id.clone(),
                cnpj: Some(cnpj),
                ticker: None,
                name: report.general.name.clone().unwrap_or_default(),
                administrator: report.general.administrator.clone(),
                manager: None,
                listed_on: None,
                delisted_on: None,
                operational_status: None,
            },
            source: SOURCE.into(),
            dataset: "fii-doc-inf_mensal".into(),
            reference_date: report.general.reference_date,
            available_at,
            ingested_at,
            submission_date: report.general.submission_date,
            availability_basis: "midas_first_capture".into(),
            filing_version: report.general.version.clone(),
            reported_mandate: report.general.mandate.clone(),
            reported_segment: report.general.segment.clone(),
            nav_per_share,
            reported_monthly_distribution_yield,
            price_to_nav: None,
            market_price_date: None,
            diagnostics,
        });
    }
    Ok(results)
}

/// Add P/NAV using a raw market price whose observation and availability dates are admissible.
pub fn apply_market_observation(
    metrics: &mut FiiMonthlyMetrics,
    market: &MarketObservation,
    as_of: DateTime<Utc>,
) {
    if market.fund_id != metrics.fund.id
        || market.available_at > as_of
        || market.reference_date > as_of.date_naive()
    {
        metrics
            .diagnostics
            .push("market observation is outside the requested point-in-time window".into());
        return;
    }
    metrics.market_price_date = Some(market.reference_date);
    metrics.price_to_nav = match (
        market.raw_price.filter(|v| v.is_finite() && *v > 0.0),
        metrics.nav_per_share,
    ) {
        (Some(price), Some(nav)) if nav > 0.0 => Some(price / nav),
        _ => {
            metrics
                .diagnostics
                .push("raw price or NAV per share is unavailable for P/NAV".into());
            None
        }
    };
}
