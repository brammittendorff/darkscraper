use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub tor: TorConfig,
    pub i2p: I2pConfig,
    pub zeronet: ZeronetConfig,
    pub freenet: FreenetConfig,
    pub lokinet: LokinetConfig,
    pub database: DatabaseConfig,
    pub extraction: ExtractionConfig,
    pub frontier: FrontierConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    pub data_dir: String,
    pub max_depth: u32,
    pub max_pages_per_domain: u64,
    pub max_body_size_mb: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TorConfig {
    pub enabled: bool,
    pub socks_proxies: Vec<String>,
    pub max_concurrency: usize,
    pub min_delay_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct I2pConfig {
    pub enabled: bool,
    pub http_proxies: Vec<String>,
    pub max_concurrency: usize,
    pub min_delay_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ZeronetConfig {
    pub enabled: bool,
    pub http_proxies: Vec<String>,
    pub max_concurrency: usize,
    pub min_delay_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FreenetConfig {
    pub enabled: bool,
    pub http_proxies: Vec<String>,
    pub max_concurrency: usize,
    pub min_delay_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LokinetConfig {
    pub enabled: bool,
    pub socks_proxies: Vec<String>,
    pub max_concurrency: usize,
    pub min_delay_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub postgres_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExtractionConfig {
    pub extract_emails: bool,
    pub extract_crypto: bool,
    pub extract_phones: bool,
    pub extract_pgp: bool,
    pub extract_usernames: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FrontierConfig {
    /// Deprecated: bloom auto-grows now. Kept for config compat.
    #[serde(default = "default_bloom_size")]
    pub bloom_filter_size: usize,
    #[serde(default = "default_bloom_fp")]
    pub bloom_filter_fp_rate: f64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_bloom_size() -> usize { 10_000_000 }
fn default_bloom_fp() -> f64 { 0.001 }
fn default_max_retries() -> u32 { 3 }
