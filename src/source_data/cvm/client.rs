use super::models::{CkanSearchResult, CvmDataset, CvmResource};
use super::{CvmError, DatasetSearchPage};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

const DEFAULT_API_URL: &str = "https://dados.cvm.gov.br/api/3/action";

#[derive(Debug, Clone)]
pub struct CkanConfig {
    pub api_url: String,
    pub request_timeout: Duration,
    pub max_attempts: usize,
}

impl Default for CkanConfig {
    fn default() -> Self {
        Self {
            api_url: DEFAULT_API_URL.to_owned(),
            request_timeout: Duration::from_secs(60),
            max_attempts: 3,
        }
    }
}

pub struct CkanClient {
    http: Client,
    config: CkanConfig,
}

#[derive(Debug, Deserialize)]
struct CkanEnvelope<T> {
    success: bool,
    result: Option<T>,
    error: Option<Value>,
}

impl CkanClient {
    pub fn new(config: CkanConfig) -> Result<Self, CvmError> {
        let http = Client::builder()
            .timeout(config.request_timeout)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self { http, config })
    }

    pub fn with_defaults() -> Result<Self, CvmError> {
        Self::new(CkanConfig::default())
    }

    /// List all catalog package names. CKAN package_list does not provide
    /// stable pagination across versions, so search_datasets is available for
    /// callers that need offset/limit pagination.
    pub fn list_datasets(&self) -> Result<Vec<String>, CvmError> {
        self.get_action("package_list", &[])
    }

    pub fn search_datasets(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<DatasetSearchPage, CvmError> {
        let rows = limit.max(1);
        let start = offset;
        let response: CkanSearchResult = self.get_action(
            "package_search",
            &[
                ("q", query.to_owned()),
                ("start", start.to_string()),
                ("rows", rows.to_string()),
            ],
        )?;
        Ok(DatasetSearchPage {
            count: response.count,
            offset,
            limit: rows,
            datasets: response.results,
        })
    }

    pub fn dataset(&self, id_or_name: &str) -> Result<CvmDataset, CvmError> {
        self.get_action("package_show", &[("id", id_or_name.to_owned())])
    }

    pub fn resource(&self, resource_id: &str) -> Result<CvmResource, CvmError> {
        self.get_action("resource_show", &[("id", resource_id.to_owned())])
    }

    /// Query CKAN DataStore when the selected resource has a DataStore table.
    /// Raw records remain JSON because each resource owns its own schema.
    pub fn datastore_search(
        &self,
        resource_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<DataStorePage, CvmError> {
        self.get_action(
            "datastore_search",
            &[
                ("resource_id", resource_id.to_owned()),
                ("offset", offset.to_string()),
                ("limit", limit.max(1).to_string()),
            ],
        )
    }

    pub fn api_url(&self) -> &str {
        &self.config.api_url
    }

    fn get_action<T: DeserializeOwned>(
        &self,
        action: &str,
        query: &[(&str, String)],
    ) -> Result<T, CvmError> {
        let url = format!("{}/{}", self.config.api_url.trim_end_matches('/'), action);
        let mut last_error = None;
        let attempts = self.config.max_attempts.max(1);

        for attempt in 0..attempts {
            let response = match self.http.get(&url).query(query).send() {
                Ok(response) => response,
                Err(error) => {
                    let retryable = error.is_timeout() || error.is_connect();
                    last_error = Some(CvmError::Http(error));
                    if retryable && attempt + 1 < attempts {
                        std::thread::sleep(Duration::from_millis(250 * (1 << attempt.min(5))));
                        continue;
                    }
                    break;
                }
            };

            let status = response.status();
            if (status.is_server_error() || status.as_u16() == 429) && attempt + 1 < attempts {
                std::thread::sleep(Duration::from_millis(250 * (1 << attempt.min(5))));
                continue;
            }
            let response = response.error_for_status()?;
            let envelope: CkanEnvelope<T> = response.json()?;
            if envelope.success {
                return envelope
                    .result
                    .ok_or_else(|| CvmError::Api(format!("{action} response omitted result")));
            }
            return Err(CvmError::Api(
                envelope
                    .error
                    .map_or_else(|| "unknown CKAN error".into(), |e| e.to_string()),
            ));
        }

        Err(last_error
            .unwrap_or_else(|| CvmError::Api(format!("{action} failed after {attempts} attempts"))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_catalog_metadata_and_preserves_unknown_fields() {
        let payload = r#"{
            "success": true,
            "result": {
                "id": "dataset-id", "name": "cia_aberta-doc-itr", "title": "ITR",
                "notes": "quarterly data", "custom_metadata": "preserved",
                "resources": [{"id":"resource-id", "name":"2026 data", "format":"ZIP",
                    "url":"https://dados.cvm.gov.br/example.zip", "size":42, "custom_resource":true}]
            }
        }"#;
        let response: CkanEnvelope<CvmDataset> = serde_json::from_str(payload).unwrap();
        let dataset = response.result.unwrap();
        assert_eq!(dataset.name, "cia_aberta-doc-itr");
        assert_eq!(dataset.extra["custom_metadata"], "preserved");
        assert_eq!(
            dataset.resources[0].resource_format(),
            super::super::ResourceFormat::Zip
        );
        assert_eq!(dataset.resources[0].extra["custom_resource"], true);
    }

    #[test]
    fn deserializes_search_results_and_pagination_count() {
        let payload = r#"{"count": 8, "results": [{"id":"id","name":"cia_aberta-cad","title":"Cadastro","resources":[]}], "facets":{}, "search_facets":{}}"#;
        let result: CkanSearchResult = serde_json::from_str(payload).unwrap();
        assert_eq!(result.count, 8);
        assert_eq!(result.results[0].name, "cia_aberta-cad");
    }

    /// Opt in with CVM_LIVE_TESTS=1. Ordinary tests stay network independent.
    #[test]
    fn live_ckan_catalog_discovery_and_resource_metadata() {
        if std::env::var("CVM_LIVE_TESTS").as_deref() != Ok("1") {
            return;
        }
        let client = CkanClient::with_defaults().unwrap();
        let datasets = client.list_datasets().unwrap();
        assert!(datasets.iter().any(|name| name == "cia_aberta-cad"));
        let page = client.search_datasets("cia_aberta", 0, 2).unwrap();
        assert!(page.count > 0);
        assert!(!page.datasets.is_empty());
        let itr = client.dataset("cia_aberta-doc-itr").unwrap();
        assert!(!itr.resources.is_empty());
        let first = client.resource(&itr.resources[0].id).unwrap();
        assert!(!first.url.is_empty());

        let dir = std::env::temp_dir().join(format!("midas-cvm-live-{}", std::process::id()));
        let downloader = super::super::ResourceDownloader::default();
        let companies = super::super::companies::CompanyDataSource::new(&client, &downloader);
        let cad_path = dir.join("cad_cia_aberta.csv");
        assert!(
            companies
                .download_registration(&cad_path, true)
                .unwrap()
                .bytes_written
                > 0
        );
        let csv_options = super::super::CsvOptions {
            encoding: super::super::CsvEncoding::Windows1252,
            delimiter: b';',
            has_headers: true,
        };
        let cad_file = std::fs::File::open(&cad_path).unwrap();
        let company = super::super::companies::parse_registration_rows(cad_file, csv_options)
            .next()
            .unwrap()
            .unwrap();
        assert!(!company.cnpj.is_empty());
        assert!(!company.legal_name.is_empty());

        let itr_path = dir.join("itr_2026.zip");
        assert!(
            companies
                .download_itr(2026, &itr_path, true)
                .unwrap()
                .bytes_written
                > 0
        );
        let itr_files = super::super::extract_zip(&itr_path, dir.join("itr"))
            .unwrap()
            .files;
        let itr_bpa = itr_files
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("BPA_con_2026.csv")
            })
            .expect("official ITR archive contains the consolidated BPA CSV");
        let itr_file = std::fs::File::open(itr_bpa).unwrap();
        let itr_row = super::super::companies::parse_itr_rows(itr_file, csv_options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(itr_row.kind(), super::super::companies::StatementKind::Bpa);
        assert_eq!(
            itr_row.scope(),
            super::super::companies::StatementScope::Consolidated
        );
        assert!(itr_row.reference_date.is_some());
        assert!(itr_row.financial_period_end.is_some());
        assert!(itr_row.account_value.is_some());
        assert_eq!(itr_row.submission_date, None);
        let itr_bpa_ind = itr_files
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("BPA_ind_2026.csv")
            })
            .expect("official ITR archive contains the individual BPA CSV");
        let itr_individual = super::super::companies::parse_itr_rows(
            std::fs::File::open(itr_bpa_ind).unwrap(),
            csv_options,
        )
        .next()
        .unwrap()
        .unwrap();
        assert_eq!(
            itr_individual.scope(),
            super::super::companies::StatementScope::Individual
        );

        let dfp_path = dir.join("dfp_2026.zip");
        assert!(
            companies
                .download_dfp(2026, &dfp_path, true)
                .unwrap()
                .bytes_written
                > 0
        );
        let dfp_files = super::super::extract_zip(&dfp_path, dir.join("dfp"))
            .unwrap()
            .files;
        let dfp_bpa = dfp_files
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("BPA_con_2026.csv")
            })
            .expect("official DFP archive contains the consolidated BPA CSV");
        let dfp_file = std::fs::File::open(dfp_bpa).unwrap();
        let dfp_row = super::super::companies::parse_dfp_rows(dfp_file, csv_options)
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(dfp_row.kind(), super::super::companies::StatementKind::Bpa);
        assert_eq!(
            dfp_row.scope(),
            super::super::companies::StatementScope::Consolidated
        );
        assert!(dfp_row.account_value.is_some());
        let dfp_bpa_ind = dfp_files
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("BPA_ind_2026.csv")
            })
            .expect("official DFP archive contains the individual BPA CSV");
        let dfp_individual = super::super::companies::parse_dfp_rows(
            std::fs::File::open(dfp_bpa_ind).unwrap(),
            csv_options,
        )
        .next()
        .unwrap()
        .unwrap();
        assert_eq!(
            dfp_individual.scope(),
            super::super::companies::StatementScope::Individual
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DataStorePage {
    #[serde(default)]
    pub records: Vec<Value>,
    #[serde(default)]
    pub fields: Vec<Value>,
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub records_truncated: bool,
}
