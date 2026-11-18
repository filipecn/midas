//! Typed access to CVM FII structured periodic report archives.
use super::{
    CkanClient, CvmError, CvmResource, DownloadOutcome, ResourceDownloader, ResourceFormat,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Read, path::Path};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FiiMonthlyGeneralRow {
    #[serde(rename = "Tipo_Fundo_Classe", default)]
    pub fund_type: Option<String>,
    #[serde(rename = "CNPJ_Fundo_Classe", default)]
    pub cnpj: String,
    #[serde(rename = "Data_Referencia")]
    pub reference_date: NaiveDate,
    #[serde(rename = "Versao")]
    pub version: String,
    #[serde(rename = "Data_Entrega", default)]
    pub submission_date: Option<NaiveDate>,
    #[serde(rename = "Nome_Fundo_Classe", default)]
    pub name: Option<String>,
    #[serde(rename = "Data_Funcionamento", default)]
    pub operation_start_date: Option<NaiveDate>,
    #[serde(rename = "Codigo_ISIN", default)]
    pub isin: Option<String>,
    #[serde(rename = "Mandato", default)]
    pub mandate: Option<String>,
    #[serde(rename = "Segmento_Atuacao", default)]
    pub segment: Option<String>,
    #[serde(rename = "Tipo_Gestao", default)]
    pub management_type: Option<String>,
    #[serde(rename = "Nome_Administrador", default)]
    pub administrator: Option<String>,
    #[serde(rename = "CNPJ_Administrador", default)]
    pub administrator_cnpj: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FiiMonthlyAssetsRow {
    #[serde(rename = "CNPJ_Fundo_Classe")]
    pub cnpj: String,
    #[serde(rename = "Data_Referencia")]
    pub reference_date: NaiveDate,
    #[serde(rename = "Versao")]
    pub version: String,
    #[serde(flatten)]
    pub values: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct FiiMonthlyComplementRow {
    #[serde(rename = "CNPJ_Fundo_Classe")]
    pub cnpj: String,
    #[serde(rename = "Data_Referencia")]
    pub reference_date: NaiveDate,
    #[serde(rename = "Versao")]
    pub version: String,
    #[serde(rename = "Data_Informacao_Numero_Cotistas", default)]
    pub investor_count_date: Option<NaiveDate>,
    #[serde(flatten)]
    pub values: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiiMonthlyReport {
    pub general: FiiMonthlyGeneralRow,
    pub assets: Option<FiiMonthlyAssetsRow>,
    pub complement: Option<FiiMonthlyComplementRow>,
}

/// Parsed rows retain the three official monthly-report tables independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiiMonthlyArchive {
    pub general: Vec<FiiMonthlyGeneralRow>,
    pub assets: Vec<FiiMonthlyAssetsRow>,
    pub complement: Vec<FiiMonthlyComplementRow>,
}

impl FiiMonthlyArchive {
    pub fn parse_zip(
        archive_path: impl AsRef<Path>,
        extraction_dir: impl AsRef<Path>,
        year: i32,
    ) -> Result<Self, CvmError> {
        let archive = super::extract_zip(archive_path, extraction_dir)?;
        let find = |suffix: &str| {
            archive.files.iter().find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(suffix))
            })
        };
        let general_name = format!("inf_mensal_fii_geral_{year}.csv");
        let assets_name = format!("inf_mensal_fii_ativo_passivo_{year}.csv");
        let complement_name = format!("inf_mensal_fii_complemento_{year}.csv");
        let options = super::CsvOptions {
            encoding: super::CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };
        let general_file = find(&general_name)
            .ok_or_else(|| CvmError::Api(format!("monthly ZIP is missing {general_name}")))?;
        let assets_file = find(&assets_name)
            .ok_or_else(|| CvmError::Api(format!("monthly ZIP is missing {assets_name}")))?;
        let complement_file = find(&complement_name)
            .ok_or_else(|| CvmError::Api(format!("monthly ZIP is missing {complement_name}")))?;
        Ok(Self {
            general: parse_fii_monthly_general(std::fs::File::open(general_file)?, options)
                .collect::<Result<_, _>>()?,
            assets: parse_fii_monthly_assets(std::fs::File::open(assets_file)?, options)
                .collect::<Result<_, _>>()?,
            complement: parse_fii_monthly_complement(
                std::fs::File::open(complement_file)?,
                options,
            )
            .collect::<Result<_, _>>()?,
        })
    }
}

macro_rules! row_parser {
    ($name:ident, $ty:ty) => {
        pub fn $name<R: Read + 'static>(
            reader: R,
            options: super::CsvOptions,
        ) -> impl Iterator<Item = Result<$ty, csv::Error>> {
            let reader: Box<dyn Read> = match options.encoding {
                super::CsvEncoding::Utf8 => Box::new(reader),
                super::CsvEncoding::Windows1252 | super::CsvEncoding::Latin1 => Box::new(
                    super::parse::EncodingReader::new(reader, encoding_rs::WINDOWS_1252),
                ),
            };
            let mut csv_reader = csv::ReaderBuilder::new()
                .delimiter(options.delimiter)
                .has_headers(options.has_headers)
                .from_reader(reader);
            let headers = csv_reader.headers().cloned();
            let records = csv_reader.into_records();
            records.map(move |record| {
                let record = record?;
                let headers = headers
                    .as_ref()
                    .map_err(|error| csv::Error::from(std::io::Error::other(error.to_string())))?;
                let row = headers
                    .iter()
                    .zip(record.iter())
                    .map(|(key, value)| {
                        (key.to_owned(), serde_json::Value::String(value.to_owned()))
                    })
                    .collect::<serde_json::Map<_, _>>();
                serde_json::from_value(serde_json::Value::Object(row))
                    .map_err(|error| csv::Error::from(std::io::Error::other(error.to_string())))
            })
        }
    };
}

row_parser!(parse_fii_monthly_general, FiiMonthlyGeneralRow);
row_parser!(parse_fii_monthly_assets, FiiMonthlyAssetsRow);
row_parser!(parse_fii_monthly_complement, FiiMonthlyComplementRow);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiiReportDataset {
    Monthly,
    Quarterly,
    Annual,
    FinancialStatements,
}

impl FiiReportDataset {
    fn package(self) -> &'static str {
        match self {
            Self::Monthly => "fii-doc-inf_mensal",
            Self::Quarterly => "fii-doc-inf_trimestral",
            Self::Annual => "fii-doc-inf_anual",
            Self::FinancialStatements => "fii-doc-dfin",
        }
    }
}

pub struct FiiReportDataSource<'a> {
    ckan: &'a CkanClient,
    downloader: &'a ResourceDownloader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedReportSnapshot {
    pub download: DownloadOutcome,
    /// CKAN resource modification time for this exact archive snapshot.
    pub available_at: chrono::DateTime<chrono::Utc>,
    pub resource_id: String,
}

impl<'a> FiiReportDataSource<'a> {
    pub fn new(ckan: &'a CkanClient, downloader: &'a ResourceDownloader) -> Self {
        Self { ckan, downloader }
    }

    pub fn resource(&self, dataset: FiiReportDataset, year: i32) -> Result<CvmResource, CvmError> {
        let package = dataset.package();
        let catalog = self.ckan.dataset(package)?;
        let mut matches = catalog
            .resources
            .into_iter()
            .filter(|resource| {
                let format = resource.resource_format();
                let format_matches = match dataset {
                    FiiReportDataset::FinancialStatements => match format {
                        ResourceFormat::Csv => true,
                        ResourceFormat::Other(label) => label.eq_ignore_ascii_case("txt"),
                        _ => false,
                    },
                    _ => format == ResourceFormat::Zip,
                };
                format_matches
                    && resource
                        .name
                        .as_deref()
                        .is_some_and(|name| name.contains(&year.to_string()))
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(CvmError::Api(format!(
                "dataset {package} has {} ZIP resources for year {year}",
                matches.len()
            )));
        }
        Ok(matches.remove(0))
    }

    pub fn download(
        &self,
        dataset: FiiReportDataset,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.resource(dataset, year)?, destination, force)
    }

    pub fn download_snapshot(
        &self,
        dataset: FiiReportDataset,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadedReportSnapshot, CvmError> {
        let resource = self.resource(dataset, year)?;
        let modified = resource.last_modified.as_deref().ok_or_else(|| {
            CvmError::Api(format!(
                "CVM resource {} has no last_modified timestamp",
                resource.id
            ))
        })?;
        let available_at = chrono::DateTime::parse_from_rfc3339(modified)
            .map_err(|error| CvmError::Api(format!("invalid last_modified timestamp: {error}")))?
            .with_timezone(&chrono::Utc);
        let download = self.downloader.download(&resource, destination, force)?;
        Ok(DownloadedReportSnapshot {
            download,
            available_at,
            resource_id: resource.id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_data::cvm::CsvEncoding;

    fn options() -> super::super::CsvOptions {
        super::super::CsvOptions {
            encoding: CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        }
    }

    #[test]
    fn parses_monthly_general_and_preserves_unknown_fields() {
        let csv = "Tipo_Fundo_Classe;CNPJ_Fundo_Classe;Data_Referencia;Versao;Data_Entrega;Nome_Fundo_Classe;Mandato;Campo_Novo\nFundo;00.332.266/0001-31;2025-01-01;1;2025-02-17;FUNDO TESTE;Renda;preservado\n";
        let row = parse_fii_monthly_general(csv.as_bytes(), options())
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(row.cnpj, "00.332.266/0001-31");
        assert_eq!(row.submission_date.unwrap().to_string(), "2025-02-17");
        assert_eq!(row.extra["Campo_Novo"], "preservado");
    }

    #[test]
    fn monthly_asset_values_remain_exact_source_strings() {
        let csv = "CNPJ_Fundo_Classe;Data_Referencia;Versao;Patrimonio_Liquido;Cotas_Emitidas\n00.332.266/0001-31;2025-01-01;1;487055921.63;2800149\n";
        let row = parse_fii_monthly_complement(csv.as_bytes(), options())
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(row.values["Patrimonio_Liquido"], "487055921.63");
    }
}
