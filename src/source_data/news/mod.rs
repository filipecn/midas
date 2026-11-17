//! RSS news ingestion. This module normalizes feed entries for later filtering,
//! storage, and sentiment analysis; it does not fetch article page contents.

pub mod sentiment;

use std::time::Duration;

use chrono::{DateTime, Utc};
use feed_rs::{model::Entry, parser};
use reqwest::{Url, blocking::Client};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const G1_ECONOMIA_RSS: &str = "https://g1.globo.com/rss/g1/economia/";
pub const CNBC_FINANCE_RSS: &str = "https://www.cnbc.com/id/10000664/device/rss/rss.html";
pub const CNBC_TOP_NEWS_RSS: &str = "https://www.cnbc.com/id/100003114/device/rss/rss.html";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsArticle {
    pub source: String,
    pub title: String,
    pub link: Option<String>,
    pub summary: Option<String>,
    pub published: Option<DateTime<Utc>>,
    pub authors: Vec<String>,
}

#[derive(Debug, Error)]
pub enum NewsError {
    #[error("news request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("news feed XML is invalid: {0}")]
    Feed(#[from] feed_rs::parser::ParseFeedError),
    #[error("invalid news feed URL: {0}")]
    InvalidUrl(String),
}

#[derive(Debug, Clone)]
pub struct NewsClient {
    client: Client,
}

impl Default for NewsClient {
    fn default() -> Self {
        Self::new().expect("default news HTTP client configuration is valid")
    }
}

impl NewsClient {
    pub fn new() -> Result<Self, NewsError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self { client })
    }

    /// Fetch and parse any RSS, Atom, or RDF feed URL.
    pub fn fetch_feed(
        &self,
        source: impl Into<String>,
        url: &str,
    ) -> Result<Vec<NewsArticle>, NewsError> {
        let url = Url::parse(url).map_err(|_| NewsError::InvalidUrl(url.to_owned()))?;
        let body = self.client.get(url).send()?.error_for_status()?.bytes()?;
        let feed = parser::parse(body.as_ref())?;
        let source = source.into();
        Ok(feed
            .entries
            .iter()
            .map(|entry| normalize_entry(&source, entry))
            .collect())
    }

    /// Search Google News for a ticker or phrase. Search operators such as `OR`
    /// and `when:1d` are passed through unchanged.
    pub fn google_news_search(&self, query: &str) -> Result<Vec<NewsArticle>, NewsError> {
        let mut url = Url::parse("https://news.google.com/rss/search")
            .expect("Google News search URL is valid");
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("hl", "pt-BR")
            .append_pair("gl", "BR")
            .append_pair("ceid", "BR:pt-419");
        self.fetch_feed("Google News", url.as_str())
    }

    pub fn g1_economia(&self) -> Result<Vec<NewsArticle>, NewsError> {
        self.fetch_feed("G1 Economia", G1_ECONOMIA_RSS)
    }

    pub fn cnbc_finance(&self) -> Result<Vec<NewsArticle>, NewsError> {
        self.fetch_feed("CNBC Finance", CNBC_FINANCE_RSS)
    }

    pub fn cnbc_top_news(&self) -> Result<Vec<NewsArticle>, NewsError> {
        self.fetch_feed("CNBC Top News", CNBC_TOP_NEWS_RSS)
    }
}

fn normalize_entry(source: &str, entry: &Entry) -> NewsArticle {
    NewsArticle {
        source: source.to_owned(),
        title: entry
            .title
            .as_ref()
            .map(|text| text.content.clone())
            .unwrap_or_default(),
        link: entry.links.first().map(|link| link.href.clone()),
        summary: entry
            .summary
            .as_ref()
            .map(|text| text.content.clone())
            .or_else(|| {
                entry
                    .content
                    .as_ref()
                    .and_then(|content| content.body.clone())
            }),
        published: entry.published.or(entry.updated),
        authors: entry
            .authors
            .iter()
            .map(|author| author.name.clone())
            .collect(),
    }
}
