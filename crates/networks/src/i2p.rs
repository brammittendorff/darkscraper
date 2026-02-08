use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, warn};
use url::Url;

use darkscraper_core::{CrawlError, FetchConfig, FetchResponse, NetworkDriver};

pub struct I2pDriver {
    clients: Vec<reqwest::Client>,
    counter: AtomicUsize,
    max_concurrency: usize,
    min_delay: Duration,
}

impl I2pDriver {
    /// Extract I2P base32 cryptographic address from HTTP response
    /// Checks headers (X-I2P-DestB32) and HTML content for .b32.i2p addresses
    /// Returns the full b32.i2p URL if found
    pub fn extract_base32_address(
        headers: &HashMap<String, String>,
        body: &str,
        _base_url: &Url,
    ) -> Option<String> {
        // Check X-I2P-DestB32 or X-I2P-Dest-B32 header
        if let Some(dest_b32) = headers.get("x-i2p-destb32").or_else(|| headers.get("x-i2p-dest-b32")) {
            if dest_b32.len() >= 52 && dest_b32.contains(".b32.i2p") {
                return Some(format!("http://{}/", dest_b32.trim()));
            }
        }

        // Look for base32 addresses in the HTML content
        // Pattern: [52-56 base32 chars].b32.i2p
        if let Ok(re) = Regex::new(r"([a-z2-7]{52,56})\.b32\.i2p") {
            if let Some(caps) = re.captures(body) {
                if let Some(b32_addr) = caps.get(0) {
                    return Some(format!("http://{}/", b32_addr.as_str()));
                }
            }
        }

        None
    }

    pub fn new(
        proxy_addrs: &[String],
        max_concurrency: usize,
        min_delay_seconds: u64,
        connect_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> Result<Self, CrawlError> {
        if proxy_addrs.is_empty() {
            return Err(CrawlError::Proxy("no i2p http proxies configured".into()));
        }

        let mut clients = Vec::with_capacity(proxy_addrs.len());
        for addr in proxy_addrs {
            let proxy = reqwest::Proxy::all(format!("http://{}", addr))
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
impl NetworkDriver for I2pDriver {
    fn name(&self) -> &str {
        "i2p"
    }

    fn can_handle(&self, url: &Url) -> bool {
        url.host_str().map(|h| h.ends_with(".i2p")).unwrap_or(false)
    }

    async fn fetch(&self, url: &Url, config: &FetchConfig) -> Result<FetchResponse, CrawlError> {
        let start = Instant::now();
        let client = self.next_client();
        debug!(url = %url, "fetching via i2p");

        let resp = client.get(url.as_str()).send().await.map_err(|e| {
            warn!(url = %url, error = %e, "i2p fetch failed");
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
        let domain = url.host_str().unwrap_or("unknown").to_string();

        Ok(FetchResponse {
            url: url.clone(),
            final_url,
            status,
            headers,
            body: body.to_vec(),
            content_type,
            fetched_at: chrono::Utc::now(),
            network: "i2p".to_string(),
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
        // I2P is a P2P network that needs time to integrate peers
        // Clear dead URLs on startup and retry every hour as network improves
        (true, 3600) // clear_on_startup=true, retry_every=1 hour
    }

    fn classify_error(&self, error: &str) -> &'static str {
        let error_lower = error.to_lowercase();

        // I2P-specific permanent failures (dead)
        if error_lower.contains("404") ||
           error_lower.contains("not found") ||
           error_lower.contains("invalid destination") ||
           error_lower.contains("bad hostname") {
            return "dead";
        }

        // I2P-specific temporary failures (unreachable - P2P network issues)
        // Most I2P errors are network-related: insufficient peers, tunnels building, etc.
        if error_lower.contains("error sending request") ||
           error_lower.contains("timeout") ||
           error_lower.contains("connection") ||
           error_lower.contains("tunnel") ||
           error_lower.contains("peer") {
            return "unreachable"; // Retry later when network has more peers
        }

        // Default: unreachable (P2P network, assume temporary)
        "unreachable"
    }
}
