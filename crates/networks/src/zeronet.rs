use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{debug, warn};
use url::Url;

use darkscraper_core::{CrawlError, FetchConfig, FetchResponse, NetworkDriver};

pub struct ZeronetDriver {
    clients: Vec<reqwest::Client>,
    proxy_bases: Vec<String>,
    counter: AtomicUsize,
    max_concurrency: usize,
    min_delay: Duration,
}

impl ZeronetDriver {
    pub fn new(
        proxy_addrs: &[String],
        max_concurrency: usize,
        min_delay_seconds: u64,
        connect_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> Result<Self, CrawlError> {
        if proxy_addrs.is_empty() {
            return Err(CrawlError::Proxy(
                "no zeronet http proxies configured".into(),
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

    /// Convert a .bit URL to a ZeroNet proxy URL.
    /// e.g. http://talk.bit/page → http://zeronet1:43110/talk.bit/page
    /// ZeroNet expects the format: http://proxy:43110/{address}.bit{path}
    fn to_proxy_url(&self, url: &Url, idx: usize) -> String {
        let host = url.host_str().unwrap_or("");
        let path = url.path();
        let query = url.query().map(|q| format!("?{}", q)).unwrap_or_default();
        let fragment = url.fragment().map(|f| format!("#{}", f)).unwrap_or_default();
        format!("{}/{}{}{}{}", self.proxy_bases[idx], host, path, query, fragment)
    }
}

#[async_trait]
impl NetworkDriver for ZeronetDriver {
    fn name(&self) -> &str {
        "zeronet"
    }

    fn can_handle(&self, url: &Url) -> bool {
        url.host_str().map(|h| h.ends_with(".bit")).unwrap_or(false)
    }

    async fn fetch(&self, url: &Url, config: &FetchConfig, retry_count: u32) -> Result<FetchResponse, CrawlError> {
        let start = Instant::now();
        let idx = self.next_index();
        let proxy_url = self.to_proxy_url(url, idx);
        let client = &self.clients[idx];

        // Progressive timeout: 10s base + 10s per retry
        let timeout_secs = 10 + (retry_count * 10);
        let timeout = Duration::from_secs(timeout_secs as u64);

        debug!(url = %url, proxy_url = %proxy_url, timeout_secs, retry_count, "fetching via zeronet");

        let resp = client
            .get(&proxy_url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Encoding", "gzip, deflate")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("User-Agent", "Mozilla/5.0 (DarkScraper/1.0; +https://github.com/yourusername/darkscraper)")
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                warn!(url = %url, proxy_url = %proxy_url, error = %e, timeout_secs, "zeronet fetch failed");
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

        let elapsed = start.elapsed();
        let domain = url.host_str().unwrap_or("unknown").to_string();

        Ok(FetchResponse {
            url: url.clone(),
            final_url,
            status,
            headers,
            body: body.to_vec(),
            content_type,
            fetched_at: chrono::Utc::now(),
            network: "zeronet".to_string(),
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
        // ZeroNet network is mostly dead as of 2026 (sparse peers, low activity)
        // Sites are very often unavailable due to lack of seeds
        // Retry very infrequently to avoid wasting resources
        // Most failures are permanent (dead network, not dead sites)
        (false, 86400) // no startup clear, retry every 24 hours (network barely alive)
    }

    fn classify_error(&self, error: &str) -> &'static str {
        let error_lower = error.to_lowercase();

        // ZeroNet-specific permanent failures (dead)
        if error_lower.contains("404") ||
           error_lower.contains("not found") ||
           error_lower.contains("invalid address") ||
           error_lower.contains("site not found") ||
           error_lower.contains("invalid site") ||
           error_lower.contains("unknown site") {
            return "dead"; // Site truly doesn't exist
        }

        // ZeroNet-specific temporary failures (unreachable)
        // P2P network: peers might be offline but site exists
        if error_lower.contains("timeout") ||
           error_lower.contains("no peers") ||
           error_lower.contains("connection") ||
           error_lower.contains("error sending request") ||
           error_lower.contains("site loading") ||
           error_lower.contains("downloading") ||
           error_lower.contains("waiting for peers") ||
           error_lower.contains("503") || // Service temporarily unavailable
           error_lower.contains("gateway") {
            return "unreachable"; // Peers offline, retry later
        }

        // Default: unreachable (P2P network, assume temporary)
        "unreachable"
    }
}
