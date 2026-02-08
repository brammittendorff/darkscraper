use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

/// Extract URLs from JS string literals, HTML comments, data-* attributes,
/// inline event handlers, hidden/invisible DOM elements, and <noscript> tags.
pub struct SourceMiner;

// -- Regex patterns for source mining --

static HTML_COMMENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<!--([\s\S]*?)-->").unwrap());

static JS_URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"["'](https?://[^"'\s]{5,})["']"#).unwrap());

static JS_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"["'](/[a-zA-Z0-9_/\-\.]{2,})["']"#).unwrap());

static JS_ONION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"["']([a-z2-7]{56}\.onion[^"']*)["']"#).unwrap());

static JS_FETCH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"fetch\(\s*["']([^"']+)["']"#).unwrap());

static JS_LOCATION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:window\.)?location\s*=\s*["']([^"']+)["']"#).unwrap());

static JS_XHR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\.open\(\s*["'][A-Z]+["'],\s*["']([^"']+)["']"#).unwrap());

static DATA_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"data-(?:url|href|src|link|page|redirect|api|endpoint)\s*=\s*["']([^"']+)["']"#)
        .unwrap()
});

static EVENT_HANDLER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"on(?:click|load|mouseover|submit)\s*=\s*["'][^"']*(?:location|href|navigate|window\.open)\s*[=(]\s*['"]([^'"]+)['"]"#).unwrap()
});

static ONION_IN_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"https?://[a-z2-7]{56}\.onion[^\s"'<>]*"#).unwrap());

static I2P_IN_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"https?://[a-zA-Z0-9\-]+\.i2p[^\s"'<>]*"#).unwrap());

static ZERONET_IN_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"https?://[a-zA-Z0-9\-]+\.bit[^\s"'<>]*"#).unwrap());

static LOKI_IN_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"https?://[a-zA-Z0-9\-]+\.loki[^\s"'<>]*"#).unwrap());

static HYPHANET_IN_TEXT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:hyphanet|freenet):[A-Z]{2,3}@[^\s"'<>]+"#).unwrap());

impl SourceMiner {
    /// Mine the raw HTML source for URLs beyond standard <a href> links.
    /// Returns a list of discovered URL strings.
    pub fn mine(html: &str, base_url: &Url) -> Vec<String> {
        let mut urls = Vec::new();

        // 1. HTML comments
        for cap in HTML_COMMENT_RE.captures_iter(html) {
            let comment = &cap[1];
            urls.extend(extract_urls_from_text(comment, base_url));
        }

        // 2. JavaScript string literals
        for re in &[&*JS_URL_RE, &*JS_FETCH_RE, &*JS_LOCATION_RE, &*JS_XHR_RE] {
            for cap in re.captures_iter(html) {
                urls.push(cap[1].to_string());
            }
        }
        // JS relative paths
        for cap in JS_PATH_RE.captures_iter(html) {
            if let Ok(resolved) = base_url.join(&cap[1]) {
                urls.push(resolved.to_string());
            }
        }
        // JS onion addresses
        for cap in JS_ONION_RE.captures_iter(html) {
            let addr = &cap[1];
            if !addr.starts_with("http") {
                urls.push(format!("http://{}", addr));
            } else {
                urls.push(addr.to_string());
            }
        }

        // 3. Data attributes
        for cap in DATA_URL_RE.captures_iter(html) {
            let val = &cap[1];
            if val.starts_with("http") {
                urls.push(val.to_string());
            } else if let Ok(resolved) = base_url.join(val) {
                urls.push(resolved.to_string());
            }
        }

        // 4. Inline event handlers
        for cap in EVENT_HANDLER_RE.captures_iter(html) {
            let val = &cap[1];
            if val.starts_with("http") {
                urls.push(val.to_string());
            } else if let Ok(resolved) = base_url.join(val) {
                urls.push(resolved.to_string());
            }
        }

        // 5. Hidden/invisible elements
        urls.extend(extract_hidden_links(html, base_url));

        // 6. <noscript> tags
        urls.extend(extract_noscript_links(html, base_url));

        // 7. Bare .onion/.i2p/.bit URLs anywhere in source
        for m in ONION_IN_TEXT_RE.find_iter(html) {
            urls.push(m.as_str().to_string());
        }
        for m in I2P_IN_TEXT_RE.find_iter(html) {
            urls.push(m.as_str().to_string());
        }
        for m in ZERONET_IN_TEXT_RE.find_iter(html) {
            urls.push(m.as_str().to_string());
        }
        for m in LOKI_IN_TEXT_RE.find_iter(html) {
            urls.push(m.as_str().to_string());
        }
        for m in HYPHANET_IN_TEXT_RE.find_iter(html) {
            urls.push(m.as_str().to_string());
        }

        // Dedup
        urls.sort();
        urls.dedup();
        urls
    }
}

fn extract_urls_from_text(text: &str, base_url: &Url) -> Vec<String> {
    let mut urls = Vec::new();
    for m in ONION_IN_TEXT_RE.find_iter(text) {
        urls.push(m.as_str().to_string());
    }
    for m in I2P_IN_TEXT_RE.find_iter(text) {
        urls.push(m.as_str().to_string());
    }
    for m in ZERONET_IN_TEXT_RE.find_iter(text) {
        urls.push(m.as_str().to_string());
    }
    for m in LOKI_IN_TEXT_RE.find_iter(text) {
        urls.push(m.as_str().to_string());
    }
    for m in HYPHANET_IN_TEXT_RE.find_iter(text) {
        urls.push(m.as_str().to_string());
    }
    for cap in JS_URL_RE.captures_iter(text) {
        urls.push(cap[1].to_string());
    }
    for cap in JS_PATH_RE.captures_iter(text) {
        if let Ok(resolved) = base_url.join(&cap[1]) {
            urls.push(resolved.to_string());
        }
    }
    urls
}

fn extract_hidden_links(html: &str, base_url: &Url) -> Vec<String> {
    let document = Html::parse_document(html);
    let mut urls = Vec::new();

    let hidden_selectors = [
        "[style*='display:none'] a[href]",
        "[style*='display: none'] a[href]",
        "[style*='visibility:hidden'] a[href]",
        "[style*='visibility: hidden'] a[href]",
        ".hidden a[href]",
        ".d-none a[href]",
    ];

    for sel_str in &hidden_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in document.select(&sel) {
                if let Some(href) = el.value().attr("href") {
                    if let Ok(resolved) = base_url.join(href) {
                        urls.push(resolved.to_string());
                    }
                }
            }
        }
    }

    urls
}

fn extract_noscript_links(html: &str, base_url: &Url) -> Vec<String> {
    let document = Html::parse_document(html);
    let mut urls = Vec::new();

    if let Ok(sel) = Selector::parse("noscript a[href]") {
        for el in document.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                if let Ok(resolved) = base_url.join(href) {
                    urls.push(resolved.to_string());
                }
            }
        }
    }

    urls
}
