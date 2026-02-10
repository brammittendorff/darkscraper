use anyhow::Result;
use headless_chrome::{Browser, Tab};
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use crate::{RegistrationConfig, RegistrationError};

pub struct HeadlessBrowser {
    browser: Browser,
    config: RegistrationConfig,
}

impl HeadlessBrowser {
    /// Create a new headless browser instance with proxy support
    pub fn new(config: RegistrationConfig, proxy_url: Option<&str>) -> Result<Self> {
        let mut extra_args: Vec<OsString> = Vec::new();

        // Required for running in Docker containers
        extra_args.push(OsString::from("--no-sandbox"));
        extra_args.push(OsString::from("--disable-dev-shm-usage"));
        extra_args.push(OsString::from("--disable-gpu"));

        // Configure proxy if provided (for Tor/I2P/etc)
        if let Some(proxy) = proxy_url {
            extra_args.push(OsString::from(format!("--proxy-server={}", proxy)));
        }

        let mut builder = headless_chrome::LaunchOptionsBuilder::default();
        builder
            .headless(true)
            .window_size(Some((1920, 1080)))
            .args(extra_args.iter().map(|a| a.as_ref()).collect());

        // Use CHROME_PATH env var if set (for Docker/custom installs)
        if let Ok(chrome_path) = std::env::var("CHROME_PATH") {
            builder.path(Some(std::path::PathBuf::from(chrome_path)));
        }

        let launch_options = builder.build()?;

        let browser = Browser::new(launch_options)?;

        Ok(Self { browser, config })
    }

    /// Navigate to URL and wait for JavaScript to render content
    /// This handles "please wait" screens and dynamic loading
    pub async fn navigate_and_wait(&self, url: &str) -> Result<Arc<Tab>, RegistrationError> {
        let tab = self.browser.new_tab()
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        info!("navigating to {}", url);

        // Navigate to URL
        tab.navigate_to(url)
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        // Wait for initial page load
        tab.wait_until_navigated()
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        info!("page loaded, waiting for dynamic content");

        // Wait for dynamic content to load (handles waiting screens)
        // Strategy: Wait for specific indicators that content is ready
        let wait_start = std::time::Instant::now();
        let max_wait = Duration::from_secs(self.config.wait_for_content_seconds);

        loop {
            if wait_start.elapsed() > max_wait {
                warn!("timeout waiting for content to load");
                return Err(RegistrationError::Timeout);
            }

            // Check if content is loaded by looking for:
            // 1. No loading spinners
            // 2. Registration form is visible
            // 3. Body has substantial content

            let html = tab.get_content()
                .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

            let html_lower = html.to_lowercase();

            // Check for common loading indicators
            let is_loading = html_lower.contains("loading") ||
                            html_lower.contains("please wait") ||
                            html_lower.contains("checking your browser") ||
                            html_lower.contains("ddos protection") ||
                            html_lower.contains("cloudflare");

            // Check if we have substantial content
            let has_content = html.len() > 5000 ||
                            html_lower.contains("<form") ||
                            html_lower.contains("register") ||
                            html_lower.contains("sign up");

            if !is_loading && has_content {
                info!("content loaded successfully");
                break;
            }

            // Wait a bit before checking again
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Additional wait to ensure all JavaScript has executed
        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(tab)
    }

    /// Get the final rendered HTML after all JavaScript execution
    pub fn get_rendered_html(&self, tab: &Tab) -> Result<String, RegistrationError> {
        tab.get_content()
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))
    }

    /// Fill a form field by name or ID
    pub fn fill_field(&self, tab: &Tab, selector: &str, value: &str) -> Result<(), RegistrationError> {
        // Find element and fill it
        tab.evaluate(
            &format!(
                r#"
                const elem = document.querySelector('{}');
                if (elem) {{
                    elem.value = '{}';
                    elem.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    elem.dispatchEvent(new Event('change', {{ bubbles: true }}));
                }} else {{
                    throw new Error('Element not found: {}');
                }}
                "#,
                selector, value, selector
            ),
            false,
        )
        .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        Ok(())
    }

    /// Click a button or link
    pub fn click(&self, tab: &Tab, selector: &str) -> Result<(), RegistrationError> {
        tab.evaluate(
            &format!(
                r#"
                const elem = document.querySelector('{}');
                if (elem) {{
                    elem.click();
                }} else {{
                    throw new Error('Element not found: {}');
                }}
                "#,
                selector, selector
            ),
            false,
        )
        .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        Ok(())
    }

    /// Try to click an element using multiple selector strategies
    pub fn try_click(&self, tab: &Tab, selectors: &[&str]) -> Result<bool, RegistrationError> {
        for selector in selectors {
            if self.click(tab, selector).is_ok() {
                info!("Successfully clicked: {}", selector);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check if an element exists on the page
    pub fn element_exists(&self, tab: &Tab, selector: &str) -> bool {
        tab.evaluate(
            &format!(
                r#"document.querySelector('{}') !== null"#,
                selector
            ),
            false,
        )
        .map(|result| {
            result.value
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }

    /// Get cookies from the browser session
    pub fn get_cookies(&self, tab: &Tab) -> Result<String, RegistrationError> {
        let cookies = tab.get_cookies()
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        let cookies_json = serde_json::to_string(&cookies)
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        Ok(cookies_json)
    }

    /// Simulate human-like mouse movements to bypass bot detection
    pub async fn simulate_human_behavior(&self, tab: &Tab) -> Result<(), RegistrationError> {
        info!("simulating human behavior (mouse movements, scrolling)");

        // Move mouse around randomly
        for _ in 0..5 {
            let x = rand::random::<f64>() * 1000.0;
            let y = rand::random::<f64>() * 800.0;

            tab.evaluate(
                &format!(
                    r#"
                    const event = new MouseEvent('mousemove', {{
                        clientX: {},
                        clientY: {},
                        bubbles: true
                    }});
                    document.dispatchEvent(event);
                    "#,
                    x, y
                ),
                false,
            )
            .ok();

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }

        // Scroll down slowly
        for i in 0..3 {
            let scroll_y = (i + 1) * 300;
            tab.evaluate(
                &format!("window.scrollTo({{ top: {}, behavior: 'smooth' }});", scroll_y),
                false,
            )
            .ok();

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Random pause (humans don't act instantly)
        let pause_ms = 1000 + (rand::random::<u64>() % 2000);
        tokio::time::sleep(tokio::time::Duration::from_millis(pause_ms)).await;

        Ok(())
    }

    /// Take a screenshot for debugging
    pub fn screenshot(&self, tab: &Tab, path: &str) -> Result<(), RegistrationError> {
        let screenshot_data = tab.capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        )
        .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        std::fs::write(path, screenshot_data)
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        info!("screenshot saved to {}", path);
        Ok(())
    }

    /// Capture CAPTCHA image from browser (avoids proxy download issues)
    pub fn capture_captcha_image(&self, tab: &Tab, selector: &str) -> Result<Vec<u8>, RegistrationError> {
        // Try to find and screenshot the CAPTCHA image element
        let result = tab.evaluate(
            &format!(
                r#"
                const img = document.querySelector('{}');
                if (img) {{
                    const rect = img.getBoundingClientRect();
                    JSON.stringify({{
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height
                    }});
                }} else {{
                    null;
                }}
                "#,
                selector
            ),
            false,
        )
        .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        // For now, just take full screenshot
        // TODO: Crop to element bounds
        let screenshot_data = tab.capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        )
        .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        Ok(screenshot_data)
    }
}

/// Detect if a page has a waiting screen
pub fn has_waiting_screen(html: &str) -> bool {
    let html_lower = html.to_lowercase();

    html_lower.contains("loading") ||
    html_lower.contains("please wait") ||
    html_lower.contains("checking your browser") ||
    html_lower.contains("ddos protection") ||
    html_lower.contains("cloudflare") ||
    html_lower.contains("just a moment") ||
    html_lower.contains("verifying you are human")
}
