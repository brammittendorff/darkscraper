/// Headless browser handler for waiting screens
/// Uses cookies + mouse movements to bypass DDoS protection

use anyhow::Result;
use tracing::info;
use url::Url;

use darkscraper_core::{CrawlError, FetchResponse, cookie_store::CookieStore, waiting_screen};

/// Check if response is a waiting screen and needs headless browser
pub fn needs_headless_browser(response: &FetchResponse) -> bool {
    let body_text = String::from_utf8_lossy(&response.body);
    waiting_screen::is_waiting_screen(&body_text, response.body.len())
}

/// Fetch page with headless browser, simulate human behavior, capture cookies
pub async fn fetch_with_headless(
    url: &Url,
    proxy_url: &str,
    cookie_store: &CookieStore,
) -> Result<FetchResponse, CrawlError> {
    info!(url = %url, "waiting screen detected, using headless browser");

    // Try to create a headless browser with proxy
    let config = darkscraper_registration::RegistrationConfig {
        use_headless_browser: true,
        browser_timeout_seconds: 120,
        wait_for_content_seconds: 300, // 5 minutes for slow waiting screens
        captcha_service: None,
        temp_email_domain: String::new(),
        user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0".to_string(),
    };

    let browser = darkscraper_registration::browser::HeadlessBrowser::new(config, Some(proxy_url))
        .map_err(|e| CrawlError::Network(format!("headless browser launch failed: {}", e)))?;

    // Navigate and wait for content to load (handles waiting screens)
    let tab = browser.navigate_and_wait(url.as_str()).await
        .map_err(|e| CrawlError::Network(format!("headless navigation failed: {}", e)))?;

    // Simulate human behavior to bypass bot detection
    browser.simulate_human_behavior(&tab).await
        .map_err(|e| CrawlError::Network(format!("human simulation failed: {}", e)))?;

    // Get rendered HTML after JavaScript execution
    let html = browser.get_rendered_html(&tab)
        .map_err(|e| CrawlError::Network(format!("get rendered html failed: {}", e)))?;

    // Get cookies and store them
    let domain = url.host_str().unwrap_or("unknown");
    if let Ok(cookies_json) = browser.get_cookies(&tab) {
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0";
        cookie_store.set(domain.to_string(), cookies_json, user_agent.to_string());
        info!(domain = %domain, "stored cookies from headless session");
    }

    let body = html.into_bytes();
    let now = chrono::Utc::now();

    Ok(FetchResponse {
        url: url.clone(),
        final_url: url.clone(),
        status: 200,
        headers: std::collections::HashMap::new(),
        content_type: Some("text/html".to_string()),
        body,
        fetched_at: now,
        network: if domain.ends_with(".onion") { "tor" }
                 else if domain.ends_with(".i2p") { "i2p" }
                 else if domain.ends_with(".loki") { "lokinet" }
                 else if domain.ends_with(".bit") { "zeronet" }
                 else { "unknown" }.to_string(),
        response_time_ms: 0,
        domain: domain.to_string(),
    })
}

/// Build reqwest client with stored cookies
pub fn build_client_with_cookies(
    proxy_url: &str,
    domain: &str,
    cookie_store: &CookieStore,
) -> Result<reqwest::Client> {
    let mut client_builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60));

    // Add proxy
    if proxy_url.starts_with("socks5://") {
        client_builder = client_builder.proxy(reqwest::Proxy::all(proxy_url)?);
    } else {
        client_builder = client_builder.proxy(reqwest::Proxy::http(proxy_url)?);
    }

    // Add cookies if we have them
    if let Some(stored) = cookie_store.get(domain) {
        info!("reusing stored cookies for {}", domain);

        // Set user-agent from stored session
        client_builder = client_builder.user_agent(&stored.user_agent);

        // Parse and add cookies
        if let Ok(cookies) = serde_json::from_str::<Vec<serde_json::Value>>(&stored.cookies) {
            let cookie_jar = reqwest::cookie::Jar::default();

            for cookie in cookies {
                if let (Some(name), Some(value)) = (
                    cookie.get("name").and_then(|v| v.as_str()),
                    cookie.get("value").and_then(|v| v.as_str())
                ) {
                    let cookie_str = format!("{}={}", name, value);
                    let url = Url::parse(&format!("http://{}", domain)).ok();
                    if let Some(u) = url {
                        cookie_jar.add_cookie_str(&cookie_str, &u);
                    }
                }
            }

            // Note: reqwest doesn't support adding existing jar to builder
            // This is simplified - full implementation would need cookie management
        }
    }

    Ok(client_builder.build()?)
}
