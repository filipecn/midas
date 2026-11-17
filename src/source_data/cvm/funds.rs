//! Typed access to the main CVM investment-fund open datasets.
use super::{
    CkanClient, CsvOptions, CvmError, CvmResource, DownloadOutcome, ResourceDownloader,
    ResourceFormat, ResourceParser,
};
use chrono::NaiveDate;
use serde::Deserialize;
use std::{io::Read, path::Path};

macro_rules! csv_parser {
    ($fn_name:ident, $type:ty) => {
        pub fn $fn_name<R: Read + 'static>(
            reader: R,
            options: CsvOptions,
        ) -> impl Iterator<Item = Result<$type, csv::Error>> {
            ResourceParser::csv_rows(reader, options)
        }
    };
}

/// CVM fund/class registration (CAD) row.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FundRegistration {
    #[serde(rename = "TP_FUNDO", default)]
    pub fund_type: String,
    #[serde(rename = "CNPJ_FUNDO", default)]
    pub cnpj: String,
    #[serde(rename = "DENOM_SOCIAL", default)]
    pub name: String,
    #[serde(rename = "DT_REG", default)]
    pub registration_date: Option<NaiveDate>,
    #[serde(rename = "DT_CONST", default)]
    pub constitution_date: Option<NaiveDate>,
    #[serde(rename = "CD_CVM", default)]
    pub cvm_code: Option<String>,
    #[serde(rename = "DT_CANCEL", default)]
    pub cancellation_date: Option<NaiveDate>,
    #[serde(rename = "SIT", default)]
    pub status: Option<String>,
    #[serde(rename = "CLASSE", default)]
    pub class: Option<String>,
    #[serde(rename = "VL_PATRIM_LIQ", default)]
    pub net_asset_value: Option<String>,
    #[serde(rename = "DT_PATRIM_LIQ", default)]
    pub net_asset_value_date: Option<NaiveDate>,
}

/// Daily fund/class valuation, quota, flows and investor count.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FundDailyInformation {
    #[serde(rename = "TP_FUNDO_CLASSE", default)]
    pub fund_type: String,
    #[serde(rename = "CNPJ_FUNDO_CLASSE", default)]
    pub cnpj: String,
    #[serde(rename = "ID_SUBCLASSE", default)]
    pub subclass_id: Option<String>,
    #[serde(rename = "DT_COMPTC", default)]
    pub date: Option<NaiveDate>,
    #[serde(rename = "VL_TOTAL", default)]
    pub total_value: Option<String>,
    #[serde(rename = "VL_QUOTA", default)]
    pub quota_value: Option<String>,
    #[serde(rename = "VL_PATRIM_LIQ", default)]
    pub net_asset_value: Option<String>,
    #[serde(rename = "CAPTC_DIA", default)]
    pub daily_applications: Option<String>,
    #[serde(rename = "RESG_DIA", default)]
    pub daily_redemptions: Option<String>,
    #[serde(rename = "NR_COTST", default)]
    pub investor_count: Option<u64>,
}

/// One position/transaction row from the monthly CDA portfolio disclosure.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FundPortfolioHolding {
    #[serde(rename = "TP_FUNDO_CLASSE", default)]
    pub fund_type: String,
    #[serde(rename = "CNPJ_FUNDO_CLASSE", default)]
    pub cnpj: String,
    #[serde(rename = "DENOM_SOCIAL", default)]
    pub fund_name: String,
    #[serde(rename = "DT_COMPTC", default)]
    pub date: Option<NaiveDate>,
    #[serde(rename = "ID_DOC", default)]
    pub document_id: Option<String>,
    #[serde(rename = "VL_PATRIM_LIQ", default)]
    pub net_asset_value: Option<String>,
    #[serde(rename = "TP_APLIC", default)]
    pub application_type: Option<String>,
    #[serde(rename = "TP_ATIVO", default)]
    pub asset_type: Option<String>,
    #[serde(rename = "TP_NEGOC", default)]
    pub negotiation_type: Option<String>,
    #[serde(rename = "QT_POS_FINAL", default)]
    pub closing_quantity: Option<String>,
    #[serde(rename = "VL_MERC_POS_FINAL", default)]
    pub closing_market_value: Option<String>,
    #[serde(rename = "VL_CUSTO_POS_FINAL", default)]
    pub closing_cost_value: Option<String>,
    #[serde(rename = "CD_ATIVO", default)]
    pub asset_code: Option<String>,
    #[serde(rename = "DS_ATIVO", default)]
    pub asset_description: Option<String>,
    #[serde(rename = "CPF_CNPJ_EMISSOR", default)]
    pub issuer_document: Option<String>,
    #[serde(rename = "EMISSOR", default)]
    pub issuer_name: Option<String>,
}

/// A balance-sheet account row from the monthly fund balancete.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FundBalanceRow {
    #[serde(rename = "TP_FUNDO_CLASSE", default)]
    pub fund_type: String,
    #[serde(rename = "CNPJ_FUNDO_CLASSE", default)]
    pub cnpj: String,
    #[serde(rename = "DT_COMPTC", default)]
    pub date: Option<NaiveDate>,
    #[serde(rename = "PLANO_CONTA_BALCTE", default)]
    pub chart: Option<String>,
    #[serde(rename = "CD_CONTA_BALCTE", default)]
    pub account_code: Option<String>,
    #[serde(rename = "VL_SALDO_BALCTE", default)]
    pub balance: Option<String>,
}

/// Flexible metadata row for monthly FIDC information tables; columns vary by table.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FidcMonthlyRow {
    #[serde(rename = "TP_FUNDO_CLASSE", default)]
    pub fund_type: String,
    #[serde(rename = "CNPJ_FUNDO_CLASSE", default)]
    pub cnpj: String,
    #[serde(rename = "DENOM_SOCIAL", default)]
    pub fund_name: String,
    #[serde(rename = "DT_COMPTC", default)]
    pub date: Option<NaiveDate>,
    #[serde(flatten)]
    pub values: std::collections::HashMap<String, serde_json::Value>,
}

csv_parser!(parse_fund_registration_rows, FundRegistration);
csv_parser!(parse_fund_daily_rows, FundDailyInformation);
csv_parser!(parse_fund_portfolio_rows, FundPortfolioHolding);
csv_parser!(parse_fund_balance_rows, FundBalanceRow);
csv_parser!(parse_fidc_monthly_rows, FidcMonthlyRow);

/// Generic fund dataset row for profile, event and other table-shaped feeds.
/// Known columns remain available by their official names in `fields`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FundDatasetRow {
    #[serde(rename = "TP_FUNDO_CLASSE", default)]
    pub fund_type: Option<String>,
    #[serde(rename = "CNPJ_FUNDO_CLASSE", default)]
    pub cnpj: Option<String>,
    #[serde(rename = "CNPJ_FUNDO", default)]
    pub registered_fund_cnpj: Option<String>,
    #[serde(rename = "DENOM_SOCIAL", default)]
    pub fund_name: Option<String>,
    #[serde(rename = "DT_COMPTC", default)]
    pub date: Option<NaiveDate>,
    #[serde(flatten)]
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

csv_parser!(parse_fund_dataset_rows, FundDatasetRow);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FundDataset {
    Registration,
    Daily,
    Portfolio,
    Balance,
    FidcMonthly,
    ProfileMonthly,
    Eventual,
    Extract,
}

impl FundDataset {
    fn package(self) -> &'static str {
        match self {
            Self::Registration => "fi-cad",
            Self::Daily => "fi-doc-inf_diario",
            Self::Portfolio => "fi-doc-cda",
            Self::Balance => "fi-doc-balancete",
            Self::FidcMonthly => "fidc-doc-inf_mensal",
            Self::ProfileMonthly => "fi-doc-perfil_mensal",
            Self::Eventual => "fi-doc-eventual",
            Self::Extract => "fi-doc-extrato",
        }
    }
}

/// CKAN-backed resource lookup and download for CVM fund datasets.
pub struct FundDataSource<'a> {
    ckan: &'a CkanClient,
    downloader: &'a ResourceDownloader,
}

impl<'a> FundDataSource<'a> {
    pub fn new(ckan: &'a CkanClient, downloader: &'a ResourceDownloader) -> Self {
        Self { ckan, downloader }
    }

    pub fn resource(
        &self,
        dataset: FundDataset,
        year: Option<i32>,
        month: Option<u32>,
    ) -> Result<CvmResource, CvmError> {
        let package = dataset.package();
        let data = self.ckan.dataset(package)?;
        let year_text = year.map(|v| v.to_string());
        let month_text = month.map(|v| format!("{v:02}"));
        let mut resources: Vec<_> = data
            .resources
            .into_iter()
            .filter(|r| {
                let label =
                    format!("{} {}", r.name.as_deref().unwrap_or(""), r.url).to_ascii_lowercase();
                let relevant = year_text.as_ref().is_none_or(|y| label.contains(y))
                    && month_text.as_ref().is_none_or(|m| label.contains(m));
                let format_ok = match dataset {
                    FundDataset::Registration => r.resource_format() == ResourceFormat::Csv,
                    FundDataset::Daily
                    | FundDataset::Portfolio
                    | FundDataset::Balance
                    | FundDataset::FidcMonthly => r.resource_format() == ResourceFormat::Zip,
                    _ => matches!(
                        r.resource_format(),
                        ResourceFormat::Csv | ResourceFormat::Zip
                    ),
                };
                relevant && format_ok
            })
            .collect();
        resources.sort_by(|a, b| a.name.cmp(&b.name));
        resources.pop().ok_or_else(|| {
            CvmError::Api(format!(
                "dataset {package} has no resource for year {year:?}, month {month:?}"
            ))
        })
    }

    pub fn download(
        &self,
        dataset: FundDataset,
        year: Option<i32>,
        month: Option<u32>,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.resource(dataset, year, month)?, destination, force)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_data::cvm::CsvEncoding;
    #[test]
    fn parses_daily_class_data_and_portfolio_rows() {
        let options = CsvOptions {
            encoding: CsvEncoding::Utf8,
            delimiter: b';',
            has_headers: true,
        };
        let daily = "TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST\nCLASSES - FIF;00.017.024/0001-53;;2026-11-03;1131424.27;45.36;1228659.83;0.00;0.00;1\n";
        let row = parse_fund_daily_rows(daily.as_bytes(), options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            row.date,
            Some(NaiveDate::from_ymd_opt(2026, 11, 3).unwrap())
        );
        assert_eq!(row.cnpj, "00.017.024/0001-53");
        let cda = "TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;DENOM_SOCIAL;DT_COMPTC;ID_DOC;VL_PATRIM_LIQ;TP_APLIC;TP_ATIVO;EMISSOR_LIGADO;TP_NEGOC;QT_VENDA_NEGOC;VL_VENDA_NEGOC;QT_AQUIS_NEGOC;VL_AQUIS_NEGOC;QT_POS_FINAL;VL_MERC_POS_FINAL;VL_CUSTO_POS_FINAL;DT_CONFID_APLIC;CD_ATIVO;DS_ATIVO;DT_VENC;PF_PJ_EMISSOR;CPF_CNPJ_EMISSOR;EMISSOR;RISCO_EMISSOR;CD_SELIC;DT_INI_VIGENCIA;CD_PAIS;PAIS;CD_BV_MERC;BV_MERC\nCLASSES - FIDC;10.292.322/0001-05;Fundo;2026-10-31;1374040;647370441.85;Cotas; ; ; ;0;0;0;0;3;100.00;90.00;;;;PJ;123;Issuer;;;;;;;;\n";
        assert_eq!(
            parse_fund_portfolio_rows(cda.as_bytes(), options)
                .next()
                .unwrap()
                .unwrap()
                .closing_market_value
                .as_deref(),
            Some("100.00")
        );
    }

    #[test]
    fn live_fund_registration_daily_portfolio_balance_and_fidc_resources() {
        if std::env::var("CVM_LIVE_TESTS").as_deref() != Ok("1") {
            return;
        }
        let ckan = CkanClient::with_defaults().unwrap();
        let downloader = ResourceDownloader::default();
        let source = FundDataSource::new(&ckan, &downloader);
        let dir = std::env::temp_dir().join(format!("midas-cvm-funds-{}", std::process::id()));
        let options = CsvOptions {
            encoding: super::super::CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };
        let cad = source
            .download(
                FundDataset::Registration,
                None,
                None,
                dir.join("cad.csv"),
                true,
            )
            .unwrap();
        let registration =
            parse_fund_registration_rows(std::fs::File::open(cad.path).unwrap(), options)
                .next()
                .unwrap()
                .unwrap();
        assert!(!registration.cnpj.is_empty());
        for (dataset, month, filename) in [
            (FundDataset::Daily, 11, "inf_diario_fi_202611.csv"),
            (FundDataset::Portfolio, 10, "cda_fie_202610.csv"),
            (FundDataset::Balance, 10, "balancete_fi_202610.csv"),
            (
                FundDataset::FidcMonthly,
                10,
                "inf_mensal_fidc_tab_III_202610.csv",
            ),
        ] {
            let outcome = source
                .download(
                    dataset,
                    Some(2026),
                    Some(month),
                    dir.join(format!("{:?}.zip", dataset)),
                    true,
                )
                .unwrap();
            let extracted = crate::source_data::cvm::extract_zip(
                outcome.path,
                dir.join(format!("{:?}", dataset)),
            )
            .unwrap();
            let path = extracted
                .files
                .iter()
                .find(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy() == filename)
                })
                .unwrap_or_else(|| {
                    extracted
                        .files
                        .iter()
                        .find(|p| p.extension().is_some_and(|e| e == "csv"))
                        .expect("CSV inside archive")
                });
            let file = std::fs::File::open(path).unwrap();
            match dataset {
                FundDataset::Daily => {
                    assert!(parse_fund_daily_rows(file, options).next().unwrap().is_ok())
                }
                FundDataset::Portfolio => assert!(
                    parse_fund_portfolio_rows(file, options)
                        .next()
                        .unwrap()
                        .is_ok()
                ),
                FundDataset::Balance => assert!(
                    parse_fund_balance_rows(file, options)
                        .next()
                        .unwrap()
                        .is_ok()
                ),
                FundDataset::FidcMonthly => {
                    let row = parse_fidc_monthly_rows(file, options).next().unwrap();
                    assert!(row.is_ok(), "FIDC row parse failed: {:?}", row.err());
                }
                _ => unreachable!(),
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
