use crate::{Error, Progress, Result};
use reqwest::{Method, Response};
use std::{sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, sync::Mutex, time::Instant};

/// HTTP policy shared across all sources and clones of a client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub user_agent: String,
    /// Start-to-start spacing, also applied to retries and fallback endpoints.
    pub min_spacing: Duration,
    /// Retries after the first attempt.
    pub retries: u32,
    pub backoff: Duration,
    /// Request timeout includes reading the body; Overpass needs > 300 s.
    pub timeout: Duration,
    pub response_cap: usize,
    pub download_cap: usize,
}
impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "simorgh-springs (+https://github.com/bboxy450/simorgh-springs)".into(),
            min_spacing: Duration::from_secs(2),
            retries: 3,
            backoff: Duration::from_secs(2),
            timeout: Duration::from_secs(360),
            response_cap: 64 * 1024 * 1024,
            download_cap: 512 * 1024 * 1024,
        }
    }
}

/// Polite HTTP client; no credentials or persistence.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    config: ClientConfig,
    next: Arc<Mutex<Instant>>,
}
impl Client {
    pub fn new(config: ClientConfig) -> Result<Self> {
        if config.user_agent.trim().is_empty()
            || config.timeout.is_zero()
            || config.response_cap == 0
            || config.download_cap == 0
        {
            return Err(Error::Invalid(
                "nonempty User-Agent, positive timeout and caps required".into(),
            ));
        }
        let http = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout)
            // Followed redirects would bypass the shared request spacing.
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            config,
            next: Arc::new(Mutex::new(Instant::now())),
        })
    }
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
    async fn response(
        &self,
        url: &str,
        form: Option<&[(&str, &str)]>,
        progress: &Progress,
    ) -> Result<Response> {
        let mut next = self.next.lock().await;
        tokio::time::sleep_until(*next).await;
        *next = Instant::now() + self.config.min_spacing;
        // Hold through response headers so all clones dispatch in schedule order.
        let mut request = self
            .http
            .request(
                if form.is_some() {
                    Method::POST
                } else {
                    Method::GET
                },
                url,
            )
            .header(reqwest::header::ACCEPT, "application/json, text/plain, */*");
        if let Some(form) = form {
            request = request.form(form);
        }
        progress.request();
        let response = request.send().await;
        drop(next);
        let response = response?;
        if !response.status().is_success() {
            return Err(Error::Status(response.status().as_u16()));
        }
        Ok(response)
    }
    fn retryable(error: &Error) -> bool {
        match error {
            Error::Http(e) => e.is_timeout() || e.is_connect() || e.is_body() || e.is_request(),
            Error::Status(s) => matches!(s, 408 | 429 | 500..=599),
            _ => false,
        }
    }
    async fn pause(&self, attempt: u32) {
        tokio::time::sleep(self.config.backoff.saturating_mul(1u32 << attempt.min(10))).await;
    }
    pub(crate) async fn bytes(
        &self,
        url: &str,
        form: Option<&[(&str, &str)]>,
        progress: &Progress,
    ) -> Result<Vec<u8>> {
        for attempt in 0..=self.config.retries {
            let result = async {
                let mut response = self.response(url, form, progress).await?;
                let cap = self.config.response_cap;
                if response.content_length().is_some_and(|n| n > cap as u64) {
                    return Err(Error::ResponseLimit(cap));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await? {
                    if bytes.len() + chunk.len() > cap {
                        return Err(Error::ResponseLimit(cap));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(bytes)
            }
            .await;
            match result {
                Err(ref e) if attempt < self.config.retries && Self::retryable(e) => {
                    self.pause(attempt).await
                }
                result => return result,
            }
        }
        unreachable!()
    }
    pub(crate) async fn download(
        &self,
        url: &str,
        progress: &Progress,
    ) -> Result<tempfile::NamedTempFile> {
        for attempt in 0..=self.config.retries {
            let result = async {
                let temp = tempfile::NamedTempFile::new()?;
                let mut file = tokio::fs::File::from_std(temp.reopen()?);
                let mut response = self.response(url, None, progress).await?;
                let cap = self.config.download_cap;
                if response.content_length().is_some_and(|n| n > cap as u64) {
                    return Err(Error::ResponseLimit(cap));
                }
                let mut length = 0;
                while let Some(chunk) = response.chunk().await? {
                    length += chunk.len();
                    if length > cap {
                        return Err(Error::ResponseLimit(cap));
                    }
                    file.write_all(&chunk).await?;
                }
                file.flush().await?;
                Ok(temp)
            }
            .await;
            match result {
                Err(ref e) if attempt < self.config.retries && Self::retryable(e) => {
                    self.pause(attempt).await
                }
                result => return result,
            }
        }
        unreachable!()
    }
}
