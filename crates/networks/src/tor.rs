use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rquest_util::Emulation;
use tracing::{debug, warn};
use url::Url;

use darkscraper_core::{CrawlError, FetchConfig, FetchResponse, NetworkDriver};

pub struct TorDriver {
    clients: Vec<rquest::Client>,
    counter: AtomicUsize,
    max_concurrency: usize,
    min_delay: Duration,
}

impl TorDriver {
    pub fn new(
        socks_addrs: &[String],
        max_concurrency: usize,
        min_delay_seconds: u64,
        connect_timeout_seconds: u64,
        request_timeout_seconds: u64,
    ) -> Result<Self, CrawlError> {
        if socks_addrs.is_empty() {
            return Err(CrawlError::Proxy("no tor socks proxies configured".into()));
        }

        let mut clients = Vec::new();
        let mut working_addrs = Vec::new();

        for addr in socks_addrs {
            // Try to create proxy - skip if it doesn't exist/resolve
            let proxy = match rquest::Proxy::all(format!("socks5h://{}", addr)) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("⚠️  Skipping tor proxy {}: {}", addr, e);
                    continue;
                }
            };

            match rquest::Client::builder()
                .emulation(Emulation::Firefox128)
                .proxy(proxy)
                .cookie_store(true)
                .connect_timeout(Duration::from_secs(connect_timeout_seconds))
                .timeout(Duration::from_secs(request_timeout_seconds))
                .cert_verification(false)
                .build()
            {
                Ok(client) => {
                    clients.push(client);
                    working_addrs.push(addr.clone());
                }
                Err(e) => {
                    eprintln!("⚠️  Skipping tor proxy {}: {}", addr, e);
                }
            }
        }

        if clients.is_empty() {
            return Err(CrawlError::Proxy("no working tor proxies found".into()));
        }

        eprintln!("✅ Tor: Using {} of {} configured proxies", clients.len(), socks_addrs.len());

        Ok(Self {
            clients,
            counter: AtomicUsize::new(rand::random::<usize>()),
            max_concurrency,
            min_delay: Duration::from_secs(min_delay_seconds),
        })
    }

    fn next_client(&self) -> &rquest::Client {
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % self.clients.len();
        &self.clients[idx]
    }
}

#[async_trait]
impl NetworkDriver for TorDriver {
    fn name(&self) -> &str {
        "tor"
    }

    fn can_handle(&self, url: &Url) -> bool {
        url.host_str()
            .map(|h| h.ends_with(".onion"))
            .unwrap_or(false)
    }

    async fn fetch(&self, url: &Url, config: &FetchConfig) -> Result<FetchResponse, CrawlError> {
        let start = Instant::now();
        let client = self.next_client();
        debug!(url = %url, "fetching via tor");

        let resp = client.get(url.as_str()).send().await.map_err(|e| {
            warn!(url = %url, error = %e, "tor fetch failed");
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
            network: "tor".to_string(),
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
        // Tor is stable and reliable, no need to retry dead URLs
        // Sites that are down are usually permanently down
        (false, 0) // no automatic retries
    }

    fn classify_error(&self, error: &str) -> &'static str {
        let error_lower = error.to_lowercase();

        // Tor-specific permanent failures (dead)
        if error_lower.contains("404") ||
           error_lower.contains("410") ||
           error_lower.contains("not found") ||
           error_lower.contains("client error (connect)") || // Onion service offline/gone
           error_lower.contains("invalid onion") {
            return "dead"; // Onion service doesn't exist or is permanently offline
        }

        // Tor-specific temporary failures (unreachable)
        if error_lower.contains("timeout") ||
           error_lower.contains("circuit") ||
           error_lower.contains("sendrequest") {
            return "unreachable"; // Tor circuit issues, retry possible
        }

        // Default: dead (Tor is stable, failures are usually permanent)
        "dead"
    }
}
