use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{debug, warn};
use url::Url;

use darkscraper_core::{CrawlError, FetchConfig, FetchResponse, NetworkDriver};

/// Hyphanet (formerly Freenet) driver.
///
/// Freesites are accessed through FProxy, a local HTTP gateway.
/// URLs use cryptographic keys rather than domain names:
///   hyphanet:USK@<key>/<name>/<version>/
///   hyphanet:SSK@<key>/<name>
///   hyphanet:CHK@<key>
///   hyphanet:KSK@<keyword>
///
/// For backwards compatibility, the legacy freenet: scheme is also accepted.
///
/// The driver rewrites these into FProxy gateway URLs:
///   http://hyphanet1:8888/USK@<key>/<name>/<version>/
pub struct HyphanetDriver {
    clients: Vec<reqwest::Client>,
    proxy_bases: Vec<String>,
    counter: AtomicUsize,
    max_concurrency: usize,
    min_delay: Duration,
}

impl HyphanetDriver {
    /// Extract domain (site name) from Hyphanet USK/SSK URL
    /// Example: USK@key/clean-spider/37/ -> "clean-spider"
    fn extract_domain(url: &Url) -> String {
        let path = url.path();
        let parts: Vec<&str> = path.split('/').collect();
        // Format: USK@key/sitename/edition/ or SSK@key/sitename/
        if parts.len() >= 2 {
            if let Some(sitename) = parts.get(1) {
                if !sitename.is_empty() {
                    return sitename.to_string();
                }
            }
        }
        // Fallback: use first 20 chars of key as identifier
        if let Some(first) = parts.first() {
            if first.len() > 20 {
                return first[..20].to_string();
            }
            return first.to_string();
        }
        "unknown".to_string()
    }

    pub fn new(
        proxy_addrs: &[String],
        max_concurrency: usize,
        min_delay_seconds: u64,
        connect_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> Result<Self, CrawlError> {
        if proxy_addrs.is_empty() {
            return Err(CrawlError::Proxy(
                "no hyphanet http proxies configured".into(),
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

    /// Convert a hyphanet: (or legacy freenet:) URI into an FProxy gateway URL.
    /// Input:  hyphanet:USK@<key>/<name>/<ver>/
    /// Output: http://hyphanet1:8888/USK@<key>/<name>/<ver>/
    fn to_proxy_url(&self, url: &Url, idx: usize) -> String {
        // The URL is stored as hyphanet:<key_path> or freenet:<key_path>
        // url.path() gives us everything after the scheme
        let raw = url.as_str();
        let key_path = if raw.starts_with("hyphanet:") {
            &raw["hyphanet:".len()..]
        } else {
            &raw["freenet:".len()..]
        };
        format!("{}/{}", self.proxy_bases[idx], key_path)
    }
}

#[async_trait]
impl NetworkDriver for HyphanetDriver {
    fn name(&self) -> &str {
        "hyphanet"
    }

    fn can_handle(&self, url: &Url) -> bool {
        url.scheme() == "freenet" || url.scheme() == "hyphanet"
    }

    async fn fetch(&self, url: &Url, config: &FetchConfig, retry_count: u32) -> Result<FetchResponse, CrawlError> {
        let start = Instant::now();
        let idx = self.next_index();
        let proxy_url = self.to_proxy_url(url, idx);
        let client = &self.clients[idx];

        // Progressive timeout: 10s base + 10s per retry
        let timeout_secs = 10 + (retry_count * 10);
        let timeout = Duration::from_secs(timeout_secs as u64);

        debug!(url = %url, proxy_url = %proxy_url, timeout_secs, retry_count, "fetching via hyphanet");

        let resp = client
            .get(&proxy_url)
            .header("Accept", "text/html,application/xhtml+xml,*/*")
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                warn!(url = %url, error = %e, timeout_secs, "hyphanet fetch failed");
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
        let domain = Self::extract_domain(url);

        Ok(FetchResponse {
            url: url.clone(),
            final_url,
            status,
            headers,
            body: body.to_vec(),
            content_type,
            fetched_at: chrono::Utc::now(),
            network: "hyphanet".to_string(),
            response_time_ms: elapsed.as_millis() as u64,
            domain,
        })
    }

    fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    fn default_delay(&self) -> Duration {
        self.min_delay
    }

    fn retry_policy(&self) -> (bool, u64) {
        // Hyphanet is extremely slow and can have transient failures
        // Keys are permanent but may take multiple attempts to fetch
        // Retry periodically as network conditions improve
        (false, 10800) // no startup clear, retry every 3 hours
    }

    fn max_retries(&self) -> u32 {
        // Hyphanet needs many more retries due to:
        // - Extremely slow routing (minutes per request)
        // - High failure rate for initial attempts
        // - Network warming up over time
        // - Keys often exist but take time to fetch
        12 // 12 retries * 30s timeout = ~6 minutes total attempt time
    }

    fn classify_error(&self, error: &str) -> &'static str {
        let error_lower = error.to_lowercase();

        // Hyphanet-specific permanent failures (dead)
        if error_lower.contains("invalid key") ||
           error_lower.contains("malformed") ||
           error_lower.contains("bad key") ||
           error_lower.contains("404") {
            return "dead"; // Key is malformed or truly doesn't exist
        }

        // Hyphanet-specific temporary failures (unreachable)
        // These often succeed on retry as network conditions improve
        if error_lower.contains("route not found") ||
           error_lower.contains("data not found") ||
           error_lower.contains("recently failed") ||
           error_lower.contains("timeout") ||
           error_lower.contains("connection") ||
           error_lower.contains("fproxy error") {
            return "unreachable"; // Network routing issue, retry later
        }

        // Default: unreachable (Hyphanet is flaky, assume temporary)
        "unreachable"
    }
}
