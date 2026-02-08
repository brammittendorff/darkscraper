pub mod entities;
pub mod html;

use darkscraper_core::{CrawlError, FetchResponse, PageData, PageMetadata};
use sha2::{Digest, Sha256};
use url::Url;

/// Max bytes to parse (5 MB). Larger pages are truncated before parsing.
const MAX_PARSE_SIZE: usize = 5 * 1024 * 1024;

/// Extract domain from URL, handling special schemes like Hyphanet
fn extract_domain(url: &Url) -> String {
    // For standard URLs, use host
    if let Some(host) = url.host_str() {
        return host.to_string();
    }

    // For Hyphanet URLs like hyphanet:USK@.../sitename/edition/
    // Extract the site name as the domain
    if url.scheme() == "hyphanet" || url.scheme() == "freenet" {
        let path = url.path();
        // Format: USK@key/sitename/edition/ or SSK@key/sitename/ or CHK@key/
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 2 {
            // parts[0] = "USK@key", parts[1] = "sitename"
            if let Some(sitename) = parts.get(1) {
                if !sitename.is_empty() {
                    return sitename.to_string();
                }
            }
        }
        // Fallback: use the entire key as domain
        if let Some(first_part) = parts.first() {
            return first_part.to_string();
        }
    }

    "unknown".to_string()
}

pub fn parse_response(resp: &FetchResponse) -> Result<PageData, CrawlError> {
    let body = if resp.body.len() > MAX_PARSE_SIZE {
        &resp.body[..MAX_PARSE_SIZE]
    } else {
        &resp.body
    };
    let body_str = String::from_utf8_lossy(body);

    let html_result = html::parse_html(&body_str, &resp.url);
    // Extract from both body_text AND raw HTML to catch JS-embedded data (ZeroNet SPAs)
    let mut entities = entities::extract_entities(&html_result.body_text);
    let raw_entities = entities::extract_entities(&body_str);
    // Merge entities from raw HTML
    entities.emails.extend(raw_entities.emails);
    entities
        .onion_addresses
        .extend(raw_entities.onion_addresses);
    entities.i2p_addresses.extend(raw_entities.i2p_addresses);
    entities
        .bitcoin_addresses
        .extend(raw_entities.bitcoin_addresses);
    entities
        .monero_addresses
        .extend(raw_entities.monero_addresses);
    entities
        .ethereum_addresses
        .extend(raw_entities.ethereum_addresses);
    entities.phone_numbers.extend(raw_entities.phone_numbers);
    entities
        .pgp_fingerprints
        .extend(raw_entities.pgp_fingerprints);
    entities.usernames.extend(raw_entities.usernames);
    // Dedup each type
    entities.emails.sort();
    entities.emails.dedup();
    entities.onion_addresses.sort();
    entities.onion_addresses.dedup();
    entities.i2p_addresses.sort();
    entities.i2p_addresses.dedup();
    entities.bitcoin_addresses.sort();
    entities.bitcoin_addresses.dedup();
    entities.monero_addresses.sort();
    entities.monero_addresses.dedup();
    entities.ethereum_addresses.sort();
    entities.ethereum_addresses.dedup();
    entities.phone_numbers.sort();
    entities.phone_numbers.dedup();
    entities.pgp_fingerprints.sort();
    entities.pgp_fingerprints.dedup();
    entities.usernames.sort();
    entities.usernames.dedup();

    let mut hasher = Sha256::new();
    hasher.update(&resp.body);
    let raw_html_hash = format!("{:x}", hasher.finalize());

    let domain = resp.domain.clone();

    // Extract metadata from response headers
    let metadata = PageMetadata {
        server_header: resp.headers.get("server").cloned(),
        powered_by: resp.headers.get("x-powered-by").cloned(),
        meta_description: html_result.meta_description,
        meta_keywords: html_result.meta_keywords,
        language: html_result.language,
        has_login_form: html_result.has_login_form,
        has_search_form: html_result.has_search_form,
        open_graph: html_result.open_graph,
    };

    Ok(PageData {
        url: resp.url.to_string(),
        final_url: resp.final_url.to_string(),
        network: resp.network.clone(),
        title: html_result.title,
        h1: html_result.h1,
        h2: html_result.h2,
        h3: html_result.h3,
        body_text: html_result.body_text,
        raw_html: body_str.to_string(),
        raw_html_hash,
        links: html_result.links,
        entities,
        metadata,
        fetched_at: resp.fetched_at,
        response_time_ms: resp.response_time_ms,
        status_code: resp.status,
        domain,
        content_type: resp.content_type.clone(),
    })
}
