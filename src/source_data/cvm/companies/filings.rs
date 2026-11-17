use super::super::{CsvOptions, CvmError, DownloadOutcome, ResourceDownloader, ResourceParser};
use chrono::NaiveDate;
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorporateDocumentKind {
    MaterialFact,
    MarketCommunication,
    Other,
}

/// Metadata row from `ipe_cia_aberta_YYYY.csv`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct IpeDocument {
    #[serde(rename = "CNPJ_Companhia", default)]
    pub cnpj: String,
    #[serde(rename = "Nome_Companhia", default)]
    pub company_name: String,
    #[serde(rename = "Codigo_CVM", default)]
    pub cvm_code: String,
    #[serde(rename = "Data_Referencia", default)]
    pub reference_date: Option<NaiveDate>,
    #[serde(rename = "Categoria", default)]
    pub category: String,
    #[serde(rename = "Tipo", default)]
    pub document_type: Option<String>,
    #[serde(rename = "Especie", default)]
    pub species: Option<String>,
    #[serde(rename = "Assunto", default)]
    pub subject: Option<String>,
    #[serde(rename = "Data_Entrega", default)]
    pub submission_date: Option<NaiveDate>,
    #[serde(rename = "Tipo_Apresentacao", default)]
    pub presentation_type: Option<String>,
    #[serde(rename = "Protocolo_Entrega", default)]
    pub protocol: Option<String>,
    #[serde(rename = "Versao", default)]
    pub version: Option<u32>,
    #[serde(rename = "Link_Download", default)]
    pub download_url: Option<String>,
}

impl IpeDocument {
    pub fn kind(&self) -> CorporateDocumentKind {
        let metadata = format!(
            "{} {} {}",
            self.category,
            self.document_type.as_deref().unwrap_or_default(),
            self.species.as_deref().unwrap_or_default()
        )
        .to_lowercase();
        if metadata.contains("fato relevante") {
            CorporateDocumentKind::MaterialFact
        } else if metadata.contains("comunicado ao mercado") {
            CorporateDocumentKind::MarketCommunication
        } else {
            CorporateDocumentKind::Other
        }
    }
}

macro_rules! corporate_document_row {
    ($name:ident, $category:literal) => {
        /// Document metadata row from the CVM FRE or FCA annual CSV.
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
        pub struct $name {
            #[serde(rename = "CNPJ_CIA", default)]
            pub cnpj: String,
            #[serde(rename = "DT_REFER", default)]
            pub reference_date: Option<NaiveDate>,
            #[serde(rename = "VERSAO", default)]
            pub version: Option<u32>,
            #[serde(rename = "DENOM_CIA", default)]
            pub company_name: String,
            #[serde(rename = "CD_CVM", default)]
            pub cvm_code: String,
            #[serde(rename = "CATEG_DOC", default)]
            pub category: String,
            #[serde(rename = "ID_DOC", default)]
            pub document_id: Option<String>,
            #[serde(rename = "DT_RECEB", default)]
            pub submission_date: Option<NaiveDate>,
            #[serde(rename = "LINK_DOC", default)]
            pub download_url: Option<String>,
        }

        impl $name {
            pub const DATASET_NAME: &'static str = $category;

            pub fn download(
                &self,
                downloader: &ResourceDownloader,
                destination: impl AsRef<Path>,
                force: bool,
            ) -> Result<DownloadOutcome, CvmError> {
                download_linked_document(
                    downloader,
                    self.download_url.as_deref(),
                    destination,
                    force,
                )
            }
        }
    };
}

corporate_document_row!(FreDocument, "cia_aberta-doc-fre");
corporate_document_row!(FcaDocument, "cia_aberta-doc-fca");

impl IpeDocument {
    pub fn download(
        &self,
        downloader: &ResourceDownloader,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        download_linked_document(downloader, self.download_url.as_deref(), destination, force)
    }
}

pub fn download_linked_document(
    downloader: &ResourceDownloader,
    url: Option<&str>,
    destination: impl AsRef<Path>,
    force: bool,
) -> Result<DownloadOutcome, CvmError> {
    let url = url
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CvmError::Api("filing does not include a document URL".into()))?;
    downloader.download_url(url, destination, force)
}

pub fn parse_ipe_rows<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
) -> csv::DeserializeRecordsIntoIter<Box<dyn Read>, IpeDocument> {
    ResourceParser::csv_rows(reader, options)
}

pub fn parse_fre_rows<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
) -> csv::DeserializeRecordsIntoIter<Box<dyn Read>, FreDocument> {
    ResourceParser::csv_rows(reader, options)
}

pub fn parse_fca_rows<R: Read + 'static>(
    reader: R,
    options: CsvOptions,
) -> csv::DeserializeRecordsIntoIter<Box<dyn Read>, FcaDocument> {
    ResourceParser::csv_rows(reader, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_data::cvm::CsvEncoding;

    #[test]
    fn parses_ipe_dates_categories_versions_and_document_url() {
        let csv = "CNPJ_Companhia;Nome_Companhia;Codigo_CVM;Data_Referencia;Categoria;Tipo;Especie;Assunto;Data_Entrega;Tipo_Apresentacao;Protocolo_Entrega;Versao;Link_Download\n00.000.000/0001-91;BCO BRASIL S.A.;001023;2026-01-29;Fato Relevante;;;Aquisição;2026-02-05;AP;protocol-1;2;https://example.test/ipe.pdf\n";
        let options = CsvOptions {
            encoding: CsvEncoding::Utf8,
            delimiter: b';',
            has_headers: true,
        };
        let document = parse_ipe_rows(csv.as_bytes(), options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(document.kind(), CorporateDocumentKind::MaterialFact);
        assert_eq!(document.version, Some(2));
        assert_eq!(document.reference_date.unwrap().to_string(), "2026-01-29");
        assert_eq!(document.submission_date.unwrap().to_string(), "2026-02-05");
        assert!(document.download_url.unwrap().ends_with("ipe.pdf"));
    }

    #[test]
    fn parses_fre_and_fca_document_metadata_without_merging_document_ids() {
        let fre = "CNPJ_CIA;DT_REFER;VERSAO;DENOM_CIA;CD_CVM;CATEG_DOC;ID_DOC;DT_RECEB;LINK_DOC\n00.000.000/0001-91;2026-12-31;1;BCO BRASIL S.A.;001023;FRE WEB;158239;2026-05-29;https://example.test/fre.pdf\n";
        let options = CsvOptions {
            encoding: CsvEncoding::Utf8,
            delimiter: b';',
            has_headers: true,
        };
        let document = parse_fre_rows(fre.as_bytes(), options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(document.document_id.as_deref(), Some("158239"));
        assert_eq!(document.submission_date.unwrap().to_string(), "2026-05-29");
        let fca = fre
            .replace("FRE WEB;158239", "FCA;154534")
            .replace("fre.pdf", "fca.pdf");
        let registration = parse_fca_rows(std::io::Cursor::new(fca.into_bytes()), options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(registration.document_id.as_deref(), Some("154534"));
        assert!(registration.download_url.unwrap().ends_with("fca.pdf"));
    }

    /// Opt in with CVM_LIVE_TESTS=1. Ordinary tests stay network independent.
    #[test]
    fn live_ipe_fre_fca_metadata_and_linked_document_download() {
        if std::env::var("CVM_LIVE_TESTS").as_deref() != Ok("1") {
            return;
        }
        let ckan = crate::source_data::cvm::CkanClient::with_defaults().unwrap();
        let downloader = ResourceDownloader::default();
        let source = super::super::CompanyDisclosureSource::new(&ckan, &downloader);
        let dir =
            std::env::temp_dir().join(format!("midas-cvm-disclosures-{}", std::process::id()));
        let options = CsvOptions {
            encoding: CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };

        let ipe_zip = source
            .download_ipe(2026, dir.join("ipe.zip"), true)
            .unwrap();
        let ipe_files = crate::source_data::cvm::extract_zip(&ipe_zip.path, dir.join("ipe"))
            .unwrap()
            .files;
        let ipe_csv = ipe_files
            .iter()
            .find(|path| path.file_name().unwrap().to_string_lossy() == "ipe_cia_aberta_2026.csv")
            .unwrap();
        let mut fact = None;
        let mut communication = None;
        for row in parse_ipe_rows(std::fs::File::open(ipe_csv).unwrap(), options) {
            let row = row.unwrap();
            match row.kind() {
                CorporateDocumentKind::MaterialFact if fact.is_none() => fact = Some(row),
                CorporateDocumentKind::MarketCommunication if communication.is_none() => {
                    communication = Some(row)
                }
                _ => {}
            }
            if fact.is_some() && communication.is_some() {
                break;
            }
        }
        let fact = fact.expect("IPE archive contains a Fato Relevante");
        assert!(fact.reference_date.is_some());
        assert!(fact.submission_date.is_some());
        let linked = fact
            .download(&downloader, dir.join("ipe-document.bin"), true)
            .unwrap();
        assert!(linked.bytes_written > 0);
        assert!(
            communication.is_some(),
            "IPE archive contains Comunicados ao Mercado"
        );

        let fre_zip = source
            .download_fre(2026, dir.join("fre.zip"), true)
            .unwrap();
        let fre_files = crate::source_data::cvm::extract_zip(&fre_zip.path, dir.join("fre"))
            .unwrap()
            .files;
        let fre_csv = fre_files
            .iter()
            .find(|path| path.file_name().unwrap().to_string_lossy() == "fre_cia_aberta_2026.csv")
            .unwrap();
        let fre = parse_fre_rows(std::fs::File::open(fre_csv).unwrap(), options)
            .next()
            .unwrap()
            .unwrap();
        assert!(fre.document_id.is_some());
        assert!(fre.submission_date.is_some());
        assert!(
            fre.download(&downloader, dir.join("fre-document.bin"), true)
                .unwrap()
                .bytes_written
                > 0
        );

        let fca_zip = source
            .download_fca(2026, dir.join("fca.zip"), true)
            .unwrap();
        let fca_files = crate::source_data::cvm::extract_zip(&fca_zip.path, dir.join("fca"))
            .unwrap()
            .files;
        let fca_csv = fca_files
            .iter()
            .find(|path| path.file_name().unwrap().to_string_lossy() == "fca_cia_aberta_2026.csv")
            .unwrap();
        let fca = parse_fca_rows(std::fs::File::open(fca_csv).unwrap(), options)
            .next()
            .unwrap()
            .unwrap();
        assert!(fca.document_id.is_some());
        assert!(fca.submission_date.is_some());
        assert!(
            fca.download(&downloader, dir.join("fca-document.bin"), true)
                .unwrap()
                .bytes_written
                > 0
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
