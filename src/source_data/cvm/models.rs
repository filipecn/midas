use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CvmResource {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub mimetype: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CvmDataset {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub resources: Vec<CvmResource>,
    #[serde(default)]
    pub metadata_created: Option<String>,
    #[serde(default)]
    pub metadata_modified: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CkanSearchResult {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub results: Vec<CvmDataset>,
    #[serde(default)]
    pub facets: HashMap<String, Value>,
    #[serde(default)]
    pub search_facets: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatasetSearchPage {
    pub count: usize,
    pub offset: usize,
    pub limit: usize,
    pub datasets: Vec<CvmDataset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceFormat {
    Csv,
    Json,
    Zip,
    Other(String),
    Unknown,
}

impl ResourceFormat {
    pub fn from_labels(format: Option<&str>, mimetype: Option<&str>) -> Self {
        let label = format
            .or(mimetype)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if label.contains("csv") || label == "text/plain" {
            Self::Csv
        } else if label.contains("json") {
            Self::Json
        } else if label.contains("zip") || label.contains("compressed") {
            Self::Zip
        } else if label.is_empty() {
            Self::Unknown
        } else {
            Self::Other(label)
        }
    }
}

impl CvmResource {
    pub fn resource_format(&self) -> ResourceFormat {
        let by_label =
            ResourceFormat::from_labels(self.format.as_deref(), self.mimetype.as_deref());
        if by_label != ResourceFormat::Unknown {
            return by_label;
        }
        let url = self.url.to_ascii_lowercase();
        if url.ends_with(".csv") || url.contains(".csv?") {
            ResourceFormat::Csv
        } else if url.ends_with(".json") || url.contains(".json?") {
            ResourceFormat::Json
        } else if url.ends_with(".zip") || url.contains(".zip?") {
            ResourceFormat::Zip
        } else {
            ResourceFormat::Unknown
        }
    }
}
