mod company;
mod dfp;
mod disclosures;
mod filings;
mod financial;
mod itr;

pub use company::{CompanyRegistration, parse_registration_rows};
pub use dfp::parse_dfp_rows;
pub use disclosures::CompanyDisclosureSource;
pub use filings::{
    CorporateDocumentKind, FcaDocument, FreDocument, IpeDocument, download_linked_document,
    parse_fca_rows, parse_fre_rows, parse_ipe_rows,
};
pub use financial::{
    CvmDecimal, DfpStatementRow, ExerciseOrder, ItrStatementRow, StatementKind, StatementScope,
};
pub use itr::parse_itr_rows;

use super::{
    CkanClient, CvmError, CvmResource, DownloadOutcome, ResourceDownloader, ResourceFormat,
};
use std::path::Path;

/// CKAN-backed access to the open-company CAD, ITR, and DFP resources.
/// Resource URLs are always taken from current dataset metadata.
pub struct CompanyDataSource<'a> {
    ckan: &'a CkanClient,
    downloader: &'a ResourceDownloader,
}

impl<'a> CompanyDataSource<'a> {
    pub fn new(ckan: &'a CkanClient, downloader: &'a ResourceDownloader) -> Self {
        Self { ckan, downloader }
    }

    pub fn registration_resource(&self) -> Result<CvmResource, CvmError> {
        find_resource(self.ckan, "cia_aberta-cad", None, ResourceFormat::Csv)
    }

    pub fn itr_resource(&self, year: i32) -> Result<CvmResource, CvmError> {
        find_resource(
            self.ckan,
            "cia_aberta-doc-itr",
            Some(year),
            ResourceFormat::Zip,
        )
    }

    pub fn dfp_resource(&self, year: i32) -> Result<CvmResource, CvmError> {
        find_resource(
            self.ckan,
            "cia_aberta-doc-dfp",
            Some(year),
            ResourceFormat::Zip,
        )
    }

    pub fn download_registration(
        &self,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.registration_resource()?, destination, force)
    }

    pub fn download_itr(
        &self,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.itr_resource(year)?, destination, force)
    }

    pub fn download_dfp(
        &self,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.dfp_resource(year)?, destination, force)
    }
}

fn find_resource(
    ckan: &CkanClient,
    dataset_name: &str,
    year: Option<i32>,
    expected_format: ResourceFormat,
) -> Result<CvmResource, CvmError> {
    let dataset = ckan.dataset(dataset_name)?;
    let year_label = year.map(|value| value.to_string());
    dataset
        .resources
        .into_iter()
        .find(|resource| {
            resource.resource_format() == expected_format
                && year_label.as_ref().is_none_or(|year| {
                    resource.name.as_deref().unwrap_or_default().contains(year)
                        || resource.url.contains(year)
                })
        })
        .ok_or_else(|| {
            CvmError::Api(format!(
                "dataset {dataset_name} has no {expected_format:?} resource for year {year:?}"
            ))
        })
}
