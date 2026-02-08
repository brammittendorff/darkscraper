use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

/// Cross-site correlation engine.
/// Extracts fingerprints that can link different sites to the same operator.
pub struct CorrelationEngine;

#[derive(Debug, Clone)]
pub struct Correlation {
    pub domain: String,
    pub correlation_type: String,
    pub value: String,
}

// Analytics / tracker ID patterns
static GA_UA_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"UA-\d{4,10}-\d{1,4}").unwrap());

static GA_G_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"G-[A-Z0-9]{10,}").unwrap());

static GTM_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"GTM-[A-Z0-9]+").unwrap());

static FB_PIXEL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"fbq\(\s*'init'\s*,\s*'(\d{15,})'").unwrap());

static PGP_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"-----BEGIN PGP PUBLIC KEY BLOCK-----[\s\S]+?-----END PGP PUBLIC KEY BLOCK-----")
        .unwrap()
});

impl CorrelationEngine {
    /// Extract all correlatable fingerprints from a page's HTML and headers.
    pub fn extract(
        domain: &str,
        html: &str,
        headers: &std::collections::HashMap<String, String>,
    ) -> Vec<Correlation> {
        let mut correlations = Vec::new();

        // Analytics IDs
        for m in GA_UA_RE.find_iter(html) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "google_analytics_ua".to_string(),
                value: m.as_str().to_string(),
            });
        }
        for m in GA_G_RE.find_iter(html) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "google_analytics_g".to_string(),
                value: m.as_str().to_string(),
            });
        }
        for m in GTM_RE.find_iter(html) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "google_tag_manager".to_string(),
                value: m.as_str().to_string(),
            });
        }
        for cap in FB_PIXEL_RE.captures_iter(html) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "facebook_pixel".to_string(),
                value: cap[1].to_string(),
            });
        }

        // PGP key blocks
        for m in PGP_BLOCK_RE.find_iter(html) {
            let mut hasher = Sha256::new();
            hasher.update(m.as_str().as_bytes());
            let hash = format!("{:x}", hasher.finalize());
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "pgp_key_hash".to_string(),
                value: hash,
            });
        }

        // ETag header
        if let Some(etag) = headers.get("etag") {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "etag".to_string(),
                value: etag.clone(),
            });
        }

        // Server header
        if let Some(server) = headers.get("server") {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "server_header".to_string(),
                value: server.clone(),
            });
        }

        // X-Powered-By
        if let Some(powered) = headers.get("x-powered-by") {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "powered_by".to_string(),
                value: powered.clone(),
            });
        }

        // HTTP header ordering fingerprint
        let mut header_names: Vec<&String> = headers.keys().collect();
        header_names.sort(); // deterministic for storage, but we also store raw order
        let header_order: Vec<&str> = headers.keys().map(|k| k.as_str()).collect();
        let order_fingerprint = format!("{:?}", header_order);
        let mut hasher = Sha256::new();
        hasher.update(order_fingerprint.as_bytes());
        correlations.push(Correlation {
            domain: domain.to_string(),
            correlation_type: "header_order_hash".to_string(),
            value: format!("{:x}", hasher.finalize()),
        });

        correlations
    }

    /// Hash favicon bytes for cross-site correlation.
    pub fn hash_favicon(domain: &str, favicon_bytes: &[u8]) -> Correlation {
        let mut hasher = Sha256::new();
        hasher.update(favicon_bytes);
        Correlation {
            domain: domain.to_string(),
            correlation_type: "favicon_hash".to_string(),
            value: format!("{:x}", hasher.finalize()),
        }
    }

    /// Hash CSS/JS file content for shared-infrastructure detection.
    pub fn hash_static_asset(domain: &str, asset_url: &str, content: &[u8]) -> Correlation {
        let mut hasher = Sha256::new();
        hasher.update(content);
        Correlation {
            domain: domain.to_string(),
            correlation_type: format!("asset_hash:{}", asset_url),
            value: format!("{:x}", hasher.finalize()),
        }
    }

    /// Hash a 404 error page for fingerprinting.
    pub fn hash_error_page(domain: &str, error_body: &[u8]) -> Correlation {
        let mut hasher = Sha256::new();
        hasher.update(error_body);
        Correlation {
            domain: domain.to_string(),
            correlation_type: "error_page_hash".to_string(),
            value: format!("{:x}", hasher.finalize()),
        }
    }
}
