//! Source-agnostic FII research models. Provider rows stay in `source_data`.
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FiiId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fii {
    pub id: FiiId,
    pub cnpj: Option<String>,
    pub ticker: Option<String>,
    pub name: String,
    pub administrator: Option<String>,
    pub manager: Option<String>,
    pub listed_on: Option<NaiveDate>,
    pub delisted_on: Option<NaiveDate>,
    pub operational_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiiMonthlyMetrics {
    pub fund: Fii,
    pub source: String,
    pub dataset: String,
    pub reference_date: NaiveDate,
    pub available_at: DateTime<Utc>,
    pub ingested_at: DateTime<Utc>,
    pub submission_date: Option<NaiveDate>,
    pub availability_basis: String,
    pub filing_version: String,
    pub source_revision_id: String,
    pub reported_mandate: Option<String>,
    pub reported_segment: Option<String>,
    pub nav_per_share: Option<f64>,
    pub nav_method: Option<String>,
    /// Source-reported percentage value; its precise unit convention is not inferred.
    pub reported_monthly_distribution_yield: Option<f64>,
    pub price_to_nav: Option<f64>,
    pub market_price_date: Option<NaiveDate>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FiiCategory {
    Tijolo,
    Papel,
    Fof,
    Hibrido,
    Desenvolvimento,
    Outros,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationExposure {
    pub category: FiiCategory,
    /// Fraction of portfolio exposure, when reported by the source.
    pub exposure: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationPeriod {
    pub fund_id: FiiId,
    pub valid_from: NaiveDate,
    pub valid_to: Option<NaiveDate>,
    pub available_at: DateTime<Utc>,
    pub exposures: Vec<ClassificationExposure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionType {
    Ordinary,
    Extraordinary,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub fund_id: FiiId,
    pub declared_on: Option<NaiveDate>,
    pub ex_date: Option<NaiveDate>,
    pub payment_date: Option<NaiveDate>,
    pub per_share: f64,
    pub distribution_type: DistributionType,
    pub reference_period: Option<String>,
    pub source: String,
    pub published_at: Option<DateTime<Utc>>,
    pub ingested_at: DateTime<Utc>,
    pub revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketObservation {
    pub fund_id: FiiId,
    pub reference_date: NaiveDate,
    pub available_at: DateTime<Utc>,
    pub ingested_at: DateTime<Utc>,
    pub source: String,
    pub revision_id: String,
    pub raw_price: Option<f64>,
    pub adjusted_price: Option<f64>,
    pub trading_volume: Option<f64>,
    pub financial_volume: Option<f64>,
    pub market_cap: Option<f64>,
    pub shares_outstanding: Option<f64>,
}

/// Immutable source filing metadata and payload; revisions are separate records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilingObservation {
    pub fund_id: FiiId,
    pub reference_date: NaiveDate,
    pub available_at: DateTime<Utc>,
    pub ingested_at: DateTime<Utc>,
    pub source: String,
    pub dataset: String,
    pub filing_version: Option<String>,
    pub revision_id: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingDataPolicy {
    Skip,
    RequireAll,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FactorScore {
    pub raw: Option<f64>,
    pub normalized: Option<f64>,
}

/// Deterministic cross-sectional percentile normalization. Missing values remain missing.
pub fn percentile_scores(
    values: &[Option<f64>],
    policy: MissingDataPolicy,
) -> Option<Vec<FactorScore>> {
    let values = values
        .iter()
        .map(|value| value.filter(|value| value.is_finite()))
        .collect::<Vec<_>>();
    if policy == MissingDataPolicy::RequireAll && values.iter().any(Option::is_none) {
        return None;
    }
    let mut valid: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|x| (i, x)))
        .collect();
    if valid.is_empty() {
        return Some(
            values
                .iter()
                .map(|v| FactorScore {
                    raw: *v,
                    normalized: None,
                })
                .collect(),
        );
    }
    valid.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut scores = vec![None; values.len()];
    let mut start = 0;
    while start < valid.len() {
        let mut end = start + 1;
        while end < valid.len() && valid[end].1 == valid[start].1 {
            end += 1;
        }
        let pct = if valid.len() == 1 {
            50.0
        } else {
            100.0 * ((start + end - 1) as f64 / 2.0) / (valid.len() - 1) as f64
        };
        for (index, _) in &valid[start..end] {
            scores[*index] = Some(pct);
        }
        start = end;
    }
    Some(
        values
            .iter()
            .enumerate()
            .map(|(i, raw)| FactorScore {
                raw: *raw,
                normalized: scores[i],
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentile_normalization_preserves_missing_and_averages_ties() {
        let got = percentile_scores(
            &[Some(1.0), Some(1.0), None, Some(3.0)],
            MissingDataPolicy::Skip,
        )
        .unwrap();
        assert_eq!(got[0].normalized, Some(25.0));
        assert_eq!(got[1].normalized, Some(25.0));
        assert_eq!(got[2].normalized, None);
        assert_eq!(got[3].normalized, Some(100.0));
    }
    #[test]
    fn require_all_rejects_incomplete_inputs() {
        assert!(percentile_scores(&[Some(1.0), None], MissingDataPolicy::RequireAll).is_none());
    }

    #[test]
    fn non_finite_values_are_missing_for_both_policies() {
        let values = [Some(f64::NAN), Some(f64::INFINITY), Some(2.0)];
        let skipped = percentile_scores(&values, MissingDataPolicy::Skip).unwrap();
        assert_eq!(skipped[0].raw, None);
        assert_eq!(skipped[1].raw, None);
        assert_eq!(skipped[2].normalized, Some(50.0));
        assert!(percentile_scores(&values, MissingDataPolicy::RequireAll).is_none());
    }
}
