use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrawlError {
    #[error("network error: {0}")]
    Network(String),

    #[error("timeout after {0}s")]
    Timeout(u64),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("proxy error: {0}")]
    Proxy(String),

    #[error("body too large: {size} bytes (max {max})")]
    BodyTooLarge { size: usize, max: usize },

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
