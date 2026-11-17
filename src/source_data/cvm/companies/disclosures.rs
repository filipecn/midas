use super::{
    CvmError, CvmResource, DownloadOutcome, ResourceDownloader, ResourceFormat, find_resource,
};
use crate::source_data::cvm::CkanClient;
use std::path::Path;

/// CKAN-backed access to yearly IPE, FRE, and FCA metadata archives.
pub struct CompanyDisclosureSource<'a> {
    ckan: &'a CkanClient,
    downloader: &'a ResourceDownloader,
}

impl<'a> CompanyDisclosureSource<'a> {
    pub fn new(ckan: &'a CkanClient, downloader: &'a ResourceDownloader) -> Self {
        Self { ckan, downloader }
    }

    pub fn ipe_resource(&self, year: i32) -> Result<CvmResource, CvmError> {
        find_resource(
            self.ckan,
            "cia_aberta-doc-ipe",
            Some(year),
            ResourceFormat::Zip,
        )
    }

    pub fn fre_resource(&self, year: i32) -> Result<CvmResource, CvmError> {
        find_resource(
            self.ckan,
            "cia_aberta-doc-fre",
            Some(year),
            ResourceFormat::Zip,
        )
    }

    pub fn fca_resource(&self, year: i32) -> Result<CvmResource, CvmError> {
        find_resource(
            self.ckan,
            "cia_aberta-doc-fca",
            Some(year),
            ResourceFormat::Zip,
        )
    }

    pub fn download_ipe(
        &self,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.ipe_resource(year)?, destination, force)
    }

    pub fn download_fre(
        &self,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.fre_resource(year)?, destination, force)
    }

    pub fn download_fca(
        &self,
        year: i32,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.downloader
            .download(&self.fca_resource(year)?, destination, force)
    }
}
