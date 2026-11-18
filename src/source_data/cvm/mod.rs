//! Generic access to the official CVM CKAN catalog and its downloadable resources.

mod archive;
mod client;
mod download;
mod models;
mod parse;

pub mod companies;
pub mod fii_reports;
pub mod funds;

pub use archive::{ExtractedArchive, extract_zip};
pub use client::DataStorePage;
pub use client::{CkanClient, CkanConfig};
pub use download::{DownloadOutcome, ResourceDownloader};
pub use models::{CkanSearchResult, CvmDataset, CvmResource, DatasetSearchPage, ResourceFormat};
pub use parse::{CsvEncoding, CsvOptions, ResourceParser};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CvmError {
    #[error("CVM CKAN request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("CVM CKAN returned an API error: {0}")]
    Api(String),
    #[error("CVM resource I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("CVM resource JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CVM resource CSV is invalid: {0}")]
    Csv(#[from] csv::Error),
    #[error("CVM ZIP archive is invalid: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("unsupported CVM resource format: {0}")]
    UnsupportedFormat(String),
    #[error("unsafe path in CVM ZIP archive: {0}")]
    UnsafeArchivePath(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ckan_csv_json_and_zip_formats_case_insensitively() {
        assert_eq!(
            ResourceFormat::from_labels(Some("CSV"), None),
            ResourceFormat::Csv
        );
        assert_eq!(
            ResourceFormat::from_labels(None, Some("application/json")),
            ResourceFormat::Json
        );
        assert_eq!(
            ResourceFormat::from_labels(Some("zip"), None),
            ResourceFormat::Zip
        );
        assert_eq!(
            ResourceFormat::from_labels(Some("parquet"), None),
            ResourceFormat::Other("parquet".into())
        );
    }
}
