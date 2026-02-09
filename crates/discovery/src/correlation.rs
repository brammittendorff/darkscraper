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

        // === HIGH-VALUE CORRELATIONS ===

        // 1. CSS/JS Library Versions
        Self::extract_library_versions(domain, html, &mut correlations);

        // 2. CMS Fingerprints
        Self::extract_cms_fingerprints(domain, html, headers, &mut correlations);

        // 3. TLS Certificate Info (from headers if available)
        Self::extract_tls_info(domain, headers, &mut correlations);

        // 4. Cookie Name Patterns
        Self::extract_cookie_patterns(domain, headers, &mut correlations);

        // 5. Meta Generator Tags
        Self::extract_meta_generator(domain, html, &mut correlations);

        correlations
    }

    /// Extract CSS/JS library versions for infrastructure correlation
    fn extract_library_versions(domain: &str, html: &str, correlations: &mut Vec<Correlation>) {
        // jQuery versions
        if let Some(version) = Regex::new(r"jquery[/-]v?(\d+\.\d+\.?\d*)").ok()
            .and_then(|re| re.captures(html))
            .and_then(|cap| cap.get(1)) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "library_jquery".to_string(),
                value: version.as_str().to_string(),
            });
        }

        // Bootstrap versions
        if let Some(version) = Regex::new(r"bootstrap[/-]v?(\d+\.\d+\.?\d*)").ok()
            .and_then(|re| re.captures(html))
            .and_then(|cap| cap.get(1)) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "library_bootstrap".to_string(),
                value: version.as_str().to_string(),
            });
        }

        // React versions
        if let Some(version) = Regex::new(r"react[/-]v?(\d+\.\d+\.?\d*)").ok()
            .and_then(|re| re.captures(html))
            .and_then(|cap| cap.get(1)) {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "library_react".to_string(),
                value: version.as_str().to_string(),
            });
        }

        // Angular versions
        if html.contains("ng-version") {
            if let Some(version) = Regex::new(r#"ng-version="([^"]+)""#).ok()
                .and_then(|re| re.captures(html))
                .and_then(|cap| cap.get(1)) {
                correlations.push(Correlation {
                    domain: domain.to_string(),
                    correlation_type: "library_angular".to_string(),
                    value: version.as_str().to_string(),
                });
            }
        }
    }

    /// Extract CMS fingerprints
    fn extract_cms_fingerprints(domain: &str, html: &str, headers: &std::collections::HashMap<String, String>, correlations: &mut Vec<Correlation>) {
        // WordPress detection
        if html.contains("wp-content") || html.contains("wordpress") {
            // Extract WordPress version
            if let Some(version) = Regex::new(r#"content="WordPress (\d+\.\d+\.?\d*)""#).ok()
                .and_then(|re| re.captures(html))
                .and_then(|cap| cap.get(1)) {
                correlations.push(Correlation {
                    domain: domain.to_string(),
                    correlation_type: "cms_wordpress".to_string(),
                    value: version.as_str().to_string(),
                });
            } else {
                correlations.push(Correlation {
                    domain: domain.to_string(),
                    correlation_type: "cms_wordpress".to_string(),
                    value: "detected".to_string(),
                });
            }
        }

        // Drupal detection
        if html.contains("Drupal") || headers.get("x-drupal-cache").is_some() {
            if let Some(version) = Regex::new(r"Drupal (\d+\.?\d*)").ok()
                .and_then(|re| re.captures(html))
                .and_then(|cap| cap.get(1)) {
                correlations.push(Correlation {
                    domain: domain.to_string(),
                    correlation_type: "cms_drupal".to_string(),
                    value: version.as_str().to_string(),
                });
            } else {
                correlations.push(Correlation {
                    domain: domain.to_string(),
                    correlation_type: "cms_drupal".to_string(),
                    value: "detected".to_string(),
                });
            }
        }

        // Joomla detection
        if html.contains("/components/com_") || html.contains("Joomla") {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "cms_joomla".to_string(),
                value: "detected".to_string(),
            });
        }
    }

    /// Extract TLS certificate info
    fn extract_tls_info(domain: &str, headers: &std::collections::HashMap<String, String>, correlations: &mut Vec<Correlation>) {
        // Check for alt-svc header (indicates HTTP/3, QUIC support)
        if let Some(alt_svc) = headers.get("alt-svc") {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "tls_alt_svc".to_string(),
                value: alt_svc.clone(),
            });
        }

        // Strict-Transport-Security indicates HTTPS support
        if let Some(hsts) = headers.get("strict-transport-security") {
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "tls_hsts".to_string(),
                value: hsts.clone(),
            });
        }
    }

    /// Extract cookie name patterns
    fn extract_cookie_patterns(domain: &str, headers: &std::collections::HashMap<String, String>, correlations: &mut Vec<Correlation>) {
        if let Some(cookie) = headers.get("set-cookie") {
            // Extract cookie names (before =)
            for cookie_str in cookie.split(';') {
                if let Some(name) = cookie_str.trim().split('=').next() {
                    if !name.is_empty() && name.len() < 50 {
                        correlations.push(Correlation {
                            domain: domain.to_string(),
                            correlation_type: "cookie_name".to_string(),
                            value: name.to_string(),
                        });
                    }
                }
            }

            // Framework-specific cookies
            let framework_cookies = [
                ("PHPSESSID", "php_session"),
                ("laravel_session", "laravel"),
                ("connect.sid", "express_nodejs"),
                ("rack.session", "ruby_rack"),
                ("ASP.NET_SessionId", "aspnet"),
                ("JSESSIONID", "java_servlet"),
            ];

            for (cookie_name, framework) in &framework_cookies {
                if cookie.contains(cookie_name) {
                    correlations.push(Correlation {
                        domain: domain.to_string(),
                        correlation_type: "framework_cookie".to_string(),
                        value: framework.to_string(),
                    });
                }
            }
        }
    }

    /// Extract meta generator tags (CMS/framework identification)
    fn extract_meta_generator(domain: &str, html: &str, correlations: &mut Vec<Correlation>) {
        if let Some(generator) = Regex::new(r#"<meta[^>]+name=["']generator["'][^>]+content=["']([^"']+)["']"#).ok()
            .and_then(|re| re.captures(html))
            .and_then(|cap| cap.get(1)) {
            let gen_value = generator.as_str().to_string();
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "meta_generator".to_string(),
                value: gen_value,
            });
        }

        // Reverse order (content first, name second)
        if let Some(generator) = Regex::new(r#"<meta[^>]+content=["']([^"']+)["'][^>]+name=["']generator["']"#).ok()
            .and_then(|re| re.captures(html))
            .and_then(|cap| cap.get(1)) {
            let gen_value = generator.as_str().to_string();
            correlations.push(Correlation {
                domain: domain.to_string(),
                correlation_type: "meta_generator".to_string(),
                value: gen_value,
            });
        }
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
