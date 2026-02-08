use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{debug, warn};
use url::Url;

use darkscraper_core::{CrawlError, FetchConfig, FetchResponse, NetworkDriver};

/// Lokinet driver for .loki sites (SNApps).
///
/// Uses a SOCKS5 proxy (e.g. lokinet-socks image exposing port 1080)
/// to route traffic through the Lokinet overlay network.
pub struct LokinetDriver {
    clients: Vec<reqwest::Client>,
    counter: AtomicUsize,
    max_concurrency: usize,
    min_delay: Duration,
}

impl LokinetDriver {
    pub fn new(
        socks_addrs: &[String],
        max_concurrency: usize,
        min_delay_seconds: u64,
        connect_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> Result<Self, CrawlError> {
        if socks_addrs.is_empty() {
            return Err(CrawlError::Proxy(
                "no lokinet socks proxies configured".into(),
            ));
        }

        let mut clients = Vec::with_capacity(socks_addrs.len());
        for addr in socks_addrs {
            let proxy = reqwest::Proxy::all(format!("socks5h://{}", addr))
                .map_err(|e| CrawlError::Proxy(e.to_string()))?;

            let client = reqwest::Client::builder()
                .proxy(proxy)
                .connect_timeout(Duration::from_secs(connect_timeout_seconds))
                .timeout(Duration::from_secs(request_timeout_seconds))
                .read_timeout(Duration::from_secs(request_timeout_seconds))
                .pool_max_idle_per_host(4)
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .map_err(|e| CrawlError::Network(e.to_string()))?;

            clients.push(client);
        }

        Ok(Self {
            clients,
            counter: AtomicUsize::new(rand::random::<usize>()),
            max_concurrency,
            min_delay: Duration::from_secs(min_delay_seconds),
        })
    }

    fn next_client(&self) -> &reqwest::Client {
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.clients.len();
        &self.clients[idx]
    }
}

#[async_trait]
impl NetworkDriver for LokinetDriver {
    fn name(&self) -> &str {
        "lokinet"
    }

    fn can_handle(&self, url: &Url) -> bool {
        url.host_str()
            .map(|h| h.ends_with(".loki"))
            .unwrap_or(false)
    }

    async fn fetch(&self, url: &Url, config: &FetchConfig) -> Result<FetchResponse, CrawlError> {
        let start = Instant::now();
        let client = self.next_client();
        debug!(url = %url, "fetching via lokinet");

        let resp = client.get(url.as_str()).send().await.map_err(|e| {
            warn!(url = %url, error = %e, "lokinet fetch failed");
            CrawlError::Network(e.to_string())
        })?;

        let status = resp.status().as_u16();
        let final_url = Url::parse(resp.url().as_str()).unwrap_or_else(|_| url.clone());

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

        Ok(FetchResponse {
            url: url.clone(),
            final_url,
            status,
            headers,
            body: body.to_vec(),
            content_type,
            fetched_at: chrono::Utc::now(),
            network: "lokinet".to_string(),
            response_time_ms: elapsed.as_millis() as u64,
        })
    }

    fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    fn default_delay(&self) -> Duration {
        self.min_delay
    }

    fn retry_policy(&self) -> (bool, u64) {
        // Lokinet can have transient network issues, clear on startup
        // But no periodic retries (network is usually stable once working)
        (true, 0) // clear_on_startup=true, no periodic retries
    }

    fn max_pages_per_domain(&self) -> usize {
        // Lokinet has blockchain explorers (blocks.loki) with infinite pages
        // Limit heavily to ensure discovery of other SNApps
        50 // Lower limit: prevent block explorers from dominating
    }

    fn classify_error(&self, error: &str) -> &'static str {
        let error_lower = error.to_lowercase();

        // Lokinet-specific permanent failures (dead)
        if error_lower.contains("404") ||
           error_lower.contains("not found") ||
           error_lower.contains("invalid") ||
           error_lower.contains("nxdomain") {
            return "dead"; // Address doesn't exist
        }

        // Lokinet-specific temporary failures (unreachable)
        // Usually due to SNApp being offline or routing issues
        if error_lower.contains("error sending request") ||
           error_lower.contains("connection") ||
           error_lower.contains("timeout") ||
           error_lower.contains("refused") {
            return "unreachable"; // SNApp offline or network issue
        }

        // Default: unreachable (Lokinet can be flaky)
        "unreachable"
    }
}
