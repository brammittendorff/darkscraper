use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{debug, warn};
use url::Url;

use darkscraper_core::{CrawlError, FetchConfig, FetchResponse, NetworkDriver};

/// Freenet (Hyphanet) driver.
///
/// Freesites are accessed through FProxy, a local HTTP gateway.
/// URLs use cryptographic keys rather than domain names:
///   freenet:USK@<key>/<name>/<version>/
///   freenet:SSK@<key>/<name>
///   freenet:CHK@<key>
///   freenet:KSK@<keyword>
///
/// The driver rewrites these into FProxy gateway URLs:
///   http://freenet1:8888/USK@<key>/<name>/<version>/
pub struct FreenetDriver {
    clients: Vec<reqwest::Client>,
    proxy_bases: Vec<String>,
    counter: AtomicUsize,
    max_concurrency: usize,
    min_delay: Duration,
}

impl FreenetDriver {
    pub fn new(
        proxy_addrs: &[String],
        max_concurrency: usize,
        min_delay_seconds: u64,
        connect_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> Result<Self, CrawlError> {
        if proxy_addrs.is_empty() {
            return Err(CrawlError::Proxy(
                "no freenet http proxies configured".into(),
            ));
        }

        let mut clients = Vec::with_capacity(proxy_addrs.len());
        let mut proxy_bases = Vec::with_capacity(proxy_addrs.len());
        for addr in proxy_addrs {
            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(connect_timeout_seconds))
                .timeout(Duration::from_secs(request_timeout_seconds))
                .read_timeout(Duration::from_secs(request_timeout_seconds))
                .pool_max_idle_per_host(4)
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .map_err(|e| CrawlError::Network(e.to_string()))?;

            clients.push(client);
            proxy_bases.push(format!("http://{}", addr));
        }

        Ok(Self {
            clients,
            proxy_bases,
            counter: AtomicUsize::new(rand::random::<usize>()),
            max_concurrency,
            min_delay: Duration::from_secs(min_delay_seconds),
        })
    }

    fn next_index(&self) -> usize {
        self.counter.fetch_add(1, Ordering::Relaxed) % self.clients.len()
    }

    /// Convert a freenet: URI into an FProxy gateway URL.
    /// Input:  freenet:USK@<key>/<name>/<ver>/
    /// Output: http://freenet1:8888/USK@<key>/<name>/<ver>/
    fn to_proxy_url(&self, url: &Url, idx: usize) -> String {
        // The URL is stored as freenet:<key_path>
        // url.path() gives us everything after the scheme
        let key_path = &url.as_str()["freenet:".len()..];
        format!("{}/{}", self.proxy_bases[idx], key_path)
    }
}

#[async_trait]
impl NetworkDriver for FreenetDriver {
    fn name(&self) -> &str {
        "freenet"
    }

    fn can_handle(&self, url: &Url) -> bool {
        url.scheme() == "freenet"
    }

    async fn fetch(&self, url: &Url, config: &FetchConfig) -> Result<FetchResponse, CrawlError> {
        let start = Instant::now();
        let idx = self.next_index();
        let proxy_url = self.to_proxy_url(url, idx);
        let client = &self.clients[idx];
        debug!(url = %url, proxy_url = %proxy_url, "fetching via freenet");

        let resp = client
            .get(&proxy_url)
            .header("Accept", "text/html,application/xhtml+xml,*/*")
            .send()
            .await
            .map_err(|e| {
                warn!(url = %url, error = %e, "freenet fetch failed");
                CrawlError::Network(e.to_string())
            })?;

        let status = resp.status().as_u16();
        let final_url = url.clone();

        let mut headers = HashMap::new();
        for (k, v) in resp.headers() {
            if let Ok(val) = v.to_str() {
                headers.insert(k.as_str().to_string(), val.to_string());
            }
        }

        let content_type = headers.get("content-type").cloned();

        let body = resp
            .bytes()
            .await
            .map_err(|e| CrawlError::Network(e.to_string()))?;

        if body.len() > config.max_body_size {
            return Err(CrawlError::BodyTooLarge {
                size: body.len(),
                max: config.max_body_size,
            });
        }

        // FProxy returns error pages as 200 OK HTML. Detect them by title
        // and convert to errors so they don't get stored as real freesites.
        if let Ok(text) = std::str::from_utf8(&body) {
            if let Some(start_idx) = text.find("<title>") {
                if let Some(end_idx) = text[start_idx..].find("</title>") {
                    let title = &text[start_idx + 7..start_idx + end_idx];
                    let is_error = title.contains("not found")
                        || title.contains("Not found")
                        || title.contains("Invalid Key")
                        || title.contains("Set Up Freenet")
                        || title.contains("Route not found")
                        || title.contains("Data not found")
                        || title.contains("Permanent Redirect");
                    if is_error {
                        warn!(url = %url, title = title, "FProxy error page, skipping");
                        return Err(CrawlError::Network(format!("FProxy error page: {}", title)));
                    }
                }
            }
        }

        let elapsed = start.elapsed();

        Ok(FetchResponse {
            url: url.clone(),
            final_url,
            status,
            headers,
            body: body.to_vec(),
            content_type,
            fetched_at: chrono::Utc::now(),
            network: "freenet".to_string(),
            response_time_ms: elapsed.as_millis() as u64,
        })
    }

    fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    fn default_delay(&self) -> Duration {
        self.min_delay
    }
}
