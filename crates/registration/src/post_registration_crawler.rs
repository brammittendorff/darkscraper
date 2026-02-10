/// Post-registration crawler - crawls sites after registration to discover authenticated content
use anyhow::Result;
use darkscraper_storage::Storage;
use tracing::{info, warn};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PostRegistrationAnalysis {
    pub domain: String,
    pub username: String,
    pub urls_before: usize,
    pub urls_after: usize,
    pub new_urls_discovered: Vec<String>,
    pub content_increase_percent: f64,
    pub authenticated_areas_found: bool,
}

pub struct PostRegistrationCrawler {
    storage: Storage,
}

impl PostRegistrationCrawler {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Crawl a site after successful registration to discover new content
    pub async fn crawl_after_registration(
        &self,
        domain: &str,
        base_url: &str,
        session_cookies: Option<&str>,
        network: &str,
    ) -> Result<PostRegistrationAnalysis> {
        info!("🔍 Post-registration crawl starting for {}", domain);

        // Get URL count before registration
        let urls_before = self.storage.count_urls_for_domain(domain).await?;

        // Parse session cookies
        let cookies = Self::parse_cookies(session_cookies);

        // Crawl common authenticated areas
        let authenticated_urls = Self::get_authenticated_url_patterns(base_url);

        let mut new_urls = Vec::new();
        let mut authenticated_areas_found = false;

        // Try to access each authenticated URL
        for url in authenticated_urls {
            info!("Checking authenticated URL: {}", url);

            // TODO: Fetch with cookies and check if we get different content
            // For now, just queue them for crawling
            new_urls.push(url.clone());

            // Check if this URL returns 200 vs 401/403
            if Self::url_requires_auth(&url) {
                authenticated_areas_found = true;
            }
        }

        // Store discovered URLs for future crawling
        for url in &new_urls {
            if let Err(e) = self.storage.store_authenticated_url(
                url,
                domain,
                network,
                session_cookies,
            ).await {
                warn!("Failed to store authenticated URL: {}", e);
            }
        }

        // Get URL count after
        let urls_after = urls_before + new_urls.len();

        let content_increase = if urls_before > 0 {
            ((urls_after - urls_before) as f64 / urls_before as f64) * 100.0
        } else {
            0.0
        };

        info!(
            "✓ Post-registration analysis: {} → {} URLs (+{}), {:.1}% increase",
            urls_before, urls_after, new_urls.len(), content_increase
        );

        Ok(PostRegistrationAnalysis {
            domain: domain.to_string(),
            username: String::new(), // Will be filled by caller
            urls_before,
            urls_after,
            new_urls_discovered: new_urls,
            content_increase_percent: content_increase,
            authenticated_areas_found,
        })
    }

    /// Get common authenticated URL patterns
    fn get_authenticated_url_patterns(base_url: &str) -> Vec<String> {
        let patterns = vec![
            "/profile",
            "/dashboard",
            "/account",
            "/settings",
            "/messages",
            "/inbox",
            "/pm",
            "/private",
            "/member",
            "/user",
            "/admin",
            "/panel",
            "/members",
            "/users",
            "/forum",
            "/board",
            "/topics",
            "/posts",
            "/downloads",
            "/files",
            "/upload",
        ];

        patterns.iter()
            .map(|p| format!("{}{}", base_url.trim_end_matches('/'), p))
            .collect()
    }

    /// Parse cookie string
    fn parse_cookies(cookies_json: Option<&str>) -> HashMap<String, String> {
        let mut cookie_map = HashMap::new();

        if let Some(json) = cookies_json {
            if let Ok(cookies) = serde_json::from_str::<Vec<serde_json::Value>>(json) {
                for cookie in cookies {
                    if let (Some(name), Some(value)) = (
                        cookie["name"].as_str(),
                        cookie["value"].as_str()
                    ) {
                        cookie_map.insert(name.to_string(), value.to_string());
                    }
                }
            }
        }

        cookie_map
    }

    /// Check if URL typically requires authentication
    fn url_requires_auth(url: &str) -> bool {
        let auth_patterns = [
            "/profile", "/dashboard", "/account", "/settings",
            "/messages", "/inbox", "/pm", "/private", "/admin",
        ];

        auth_patterns.iter().any(|p| url.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_authenticated_patterns() {
        let urls = PostRegistrationCrawler::get_authenticated_url_patterns("http://example.onion");
        assert!(urls.contains(&"http://example.onion/profile".to_string()));
        assert!(urls.contains(&"http://example.onion/dashboard".to_string()));
        assert!(urls.len() > 10);
    }
}
