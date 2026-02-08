use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

use crate::error::CrawlError;

/// Every overlay network implements this
#[async_trait]
pub trait NetworkDriver: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn can_handle(&self, url: &Url) -> bool;
    async fn fetch(&self, url: &Url, config: &FetchConfig) -> Result<FetchResponse, CrawlError>;
    fn max_concurrency(&self) -> usize;
    fn default_delay(&self) -> Duration;

    /// Network-specific retry configuration
    /// Returns (clear_dead_on_startup, periodic_retry_interval_secs)
    /// - clear_dead_on_startup: Whether to clear dead URLs from previous sessions
    /// - periodic_retry_interval_secs: How often to retry dead URLs (0 = never)
    fn retry_policy(&self) -> (bool, u64) {
        (false, 0) // Default: no automatic retries
    }

    /// Maximum retry attempts before marking URL as permanently dead
    /// Different networks have different reliability characteristics
    fn max_retries(&self) -> u32 {
        4 // Default: 4 retries for most networks
    }

    /// Maximum pages to crawl per domain before stopping
    /// Prevents single domains (blockchain explorers, infinite pagination) from monopolizing resources
    fn max_pages_per_domain(&self) -> usize {
        100 // Default: 100 pages per domain
    }

    /// Classify fetch error as either "dead" (permanent) or "unreachable" (temporary/network)
    /// This is network-specific because different networks have different failure modes
    /// - "dead": 404, 410, DNS failure, invalid address, content not found
    /// - "unreachable": timeout, connection refused, tunnel failure, insufficient peers
    fn classify_error(&self, error: &str) -> &'static str {
        let error_lower = error.to_lowercase();

        // Permanent failures (dead)
        if error_lower.contains("404") ||
           error_lower.contains("410") ||
           error_lower.contains("not found") ||
           error_lower.contains("does not exist") ||
           error_lower.contains("invalid") ||
           error_lower.contains("malformed") {
            return "dead";
        }

        // Temporary/network failures (unreachable)
        if error_lower.contains("timeout") ||
           error_lower.contains("connection refused") ||
           error_lower.contains("connection reset") ||
           error_lower.contains("tunnel") ||
           error_lower.contains("peer") ||
           error_lower.contains("network") {
            return "unreachable";
        }

        // Default: treat as unreachable (safer, allows retry)
        "unreachable"
    }
}

#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub timeout: Duration,
    pub max_body_size: usize,
    pub follow_redirects: bool,
    pub user_agent: String,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            max_body_size: 10 * 1024 * 1024,
            follow_redirects: true,
            user_agent: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub url: Url,
    pub final_url: Url,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub network: String,
    pub response_time_ms: u64,
    pub domain: String,  // Network-specific domain extraction (host for HTTP, site name for Hyphanet, etc.)
}

/// What we extract from every page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageData {
    pub url: String,
    pub final_url: String,
    pub network: String,
    pub title: Option<String>,
    pub h1: Vec<String>,
    pub h2: Vec<String>,
    pub h3: Vec<String>,
    pub body_text: String,
    pub raw_html: String,
    pub raw_html_hash: String,
    pub links: Vec<ExtractedLink>,
    pub entities: ExtractedEntities,
    pub metadata: PageMetadata,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub response_time_ms: u64,
    pub status_code: u16,
    pub domain: String,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedEntities {
    pub emails: Vec<String>,
    pub onion_addresses: Vec<String>,
    pub i2p_addresses: Vec<String>,
    pub bitcoin_addresses: Vec<String>,
    pub monero_addresses: Vec<String>,
    pub ethereum_addresses: Vec<String>,
    pub phone_numbers: Vec<String>,
    pub pgp_fingerprints: Vec<String>,
    pub usernames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedLink {
    pub url: String,
    pub anchor_text: Option<String>,
    pub is_onion: bool,
    pub is_i2p: bool,
    pub is_zeronet: bool,
    pub is_hyphanet: bool,
    pub is_lokinet: bool,
    pub is_external: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMetadata {
    pub server_header: Option<String>,
    pub powered_by: Option<String>,
    pub meta_description: Option<String>,
    pub meta_keywords: Vec<String>,
    pub language: Option<String>,
    pub has_login_form: bool,
    pub has_search_form: bool,
    pub open_graph: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct CrawlJob {
    pub url: Url,
    pub depth: u32,
    pub source_url: Option<String>,
    pub network: String,
    pub priority: f64,
    pub retry_count: u32,
}
