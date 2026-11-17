use super::{CvmError, CvmResource};
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct ResourceDownloader {
    http: Client,
    max_attempts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome {
    pub path: PathBuf,
    pub bytes_written: u64,
    pub already_present: bool,
}

impl Default for ResourceDownloader {
    fn default() -> Self {
        Self::new(Duration::from_secs(600), 3).expect("valid default HTTP client")
    }
}

impl ResourceDownloader {
    pub fn new(timeout: Duration, max_attempts: usize) -> Result<Self, CvmError> {
        let http = Client::builder()
            .timeout(timeout)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self {
            http,
            max_attempts: max_attempts.max(1),
        })
    }

    /// Download a resource into `destination`, retaining the original file.
    /// An existing non-empty file is reused unless `force` is set.
    pub fn download(
        &self,
        resource: &CvmResource,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        self.download_url(&resource.url, destination, force)
    }

    /// Download an official linked document whose URL is carried inside a
    /// typed filing row rather than in the CKAN resource itself.
    pub fn download_url(
        &self,
        url: &str,
        destination: impl AsRef<Path>,
        force: bool,
    ) -> Result<DownloadOutcome, CvmError> {
        let destination = destination.as_ref();
        if destination.exists() && !force {
            let size = destination.metadata()?.len();
            if size > 0 {
                return Ok(DownloadOutcome {
                    path: destination.to_owned(),
                    bytes_written: size,
                    already_present: true,
                });
            }
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        let partial = destination.with_extension(format!(
            "{}.part",
            destination
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("download")
        ));

        for attempt in 0..self.max_attempts {
            let result = self.download_once(url, &partial);
            match result {
                Ok(bytes) => {
                    let backup = destination.with_extension("cvm-backup");
                    let had_destination = destination.exists();
                    if had_destination {
                        if backup.exists() {
                            fs::remove_file(&backup)?;
                        }
                        fs::rename(destination, &backup)?;
                    }
                    if let Err(error) = fs::rename(&partial, destination) {
                        if had_destination {
                            let _ = fs::rename(&backup, destination);
                        }
                        return Err(error.into());
                    }
                    if had_destination {
                        fs::remove_file(backup)?;
                    }
                    return Ok(DownloadOutcome {
                        path: destination.to_owned(),
                        bytes_written: bytes,
                        already_present: false,
                    });
                }
                Err(error) => {
                    let _ = fs::remove_file(&partial);
                    if attempt + 1 == self.max_attempts || !is_retryable(&error) {
                        return Err(error);
                    }
                    std::thread::sleep(Duration::from_millis(300 * (1 << attempt.min(5))));
                }
            }
        }
        unreachable!("at least one download attempt is configured")
    }

    fn download_once(&self, url: &str, partial: &Path) -> Result<u64, CvmError> {
        let mut response = self.http.get(url).send()?;
        let status = response.status();
        if status.is_server_error() || status.as_u16() == 429 {
            return Err(CvmError::Api(format!(
                "transient resource HTTP status {status}"
            )));
        }
        response.error_for_status_ref()?;
        let mut file = File::create(partial)?;
        let bytes = io::copy(&mut response, &mut file)?;
        file.flush()?;
        file.sync_all()?;
        Ok(bytes)
    }
}

fn is_retryable(error: &CvmError) -> bool {
    match error {
        CvmError::Http(error) => {
            error.is_timeout()
                || error.is_connect()
                || error
                    .status()
                    .is_some_and(|s| s.is_server_error() || s.as_u16() == 429)
        }
        CvmError::Api(message) => message.starts_with("transient resource HTTP status"),
        _ => false,
    }
}
