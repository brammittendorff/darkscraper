/// Login verification - tests if registered accounts actually work
use anyhow::Result;
use darkscraper_storage::Storage;
use tracing::{info, warn};
use std::sync::Arc;

use crate::browser::HeadlessBrowser;
use crate::RegistrationConfig;

#[derive(Debug, Clone)]
pub struct LoginVerificationResult {
    pub account_id: i64,
    pub username: String,
    pub domain: String,
    pub login_successful: bool,
    pub error: Option<String>,
    pub logged_in_url: Option<String>,
    pub new_cookies: Option<String>,
}

pub struct LoginVerifier {
    storage: Storage,
    config: RegistrationConfig,
}

impl LoginVerifier {
    pub fn new(storage: Storage, config: RegistrationConfig) -> Self {
        Self { storage, config }
    }

    /// Verify all registered accounts by attempting login
    pub async fn verify_all_accounts(&self, network: Option<&str>) -> Result<Vec<LoginVerificationResult>> {
        info!("🔐 Starting login verification for all registered accounts");

        // Get all accounts to verify
        let accounts = self.get_accounts_to_verify(network).await?;

        info!("Found {} accounts to verify", accounts.len());

        let mut results = Vec::new();

        for account in accounts {
            let result = self.verify_account(&account).await;
            results.push(result);

            // Small delay between attempts
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        // Update database with verification results
        for result in &results {
            if let Err(e) = self.update_account_status(result).await {
                warn!("Failed to update account status: {}", e);
            }
        }

        Ok(results)
    }

    /// Verify a single account by attempting login
    async fn verify_account(&self, account: &AccountToVerify) -> LoginVerificationResult {
        info!("🔐 Verifying account: {} @ {}", account.username, account.domain);

        let proxy_url = Self::get_proxy_for_network(&account.network);

        match self.attempt_login(
            &account.site_url,
            &account.username,
            &account.password,
            proxy_url.as_deref(),
        ).await {
            Ok((success, logged_in_url, cookies)) => {
                if success {
                    info!("✓ Login successful: {} @ {}", account.username, account.domain);
                } else {
                    warn!("✗ Login failed: {} @ {}", account.username, account.domain);
                }

                LoginVerificationResult {
                    account_id: account.id,
                    username: account.username.clone(),
                    domain: account.domain.clone(),
                    login_successful: success,
                    error: if success { None } else { Some("Login failed".to_string()) },
                    logged_in_url,
                    new_cookies: cookies,
                }
            }
            Err(e) => {
                warn!("✗ Login error for {} @ {}: {}", account.username, account.domain, e);

                LoginVerificationResult {
                    account_id: account.id,
                    username: account.username.clone(),
                    domain: account.domain.clone(),
                    login_successful: false,
                    error: Some(e.to_string()),
                    logged_in_url: None,
                    new_cookies: None,
                }
            }
        }
    }

    /// Attempt to log in to a site
    pub async fn attempt_login(
        &self,
        site_url: &str,
        username: &str,
        password: &str,
        proxy_url: Option<&str>,
    ) -> Result<(bool, Option<String>, Option<String>)> {
        info!("Attempting login at {}", site_url);

        // Create browser
        let browser = HeadlessBrowser::new(self.config.clone(), proxy_url)?;

        // For DNMX and similar, login is on the same page
        // Navigate to site
        let tab = browser.navigate_and_wait(site_url).await?;

        // Get HTML
        let html = browser.get_rendered_html(&tab)?;

        // Find login form
        let login_form = Self::find_login_form(&html)?;

        info!("Found login form with {} fields", login_form.username_field.is_some() as usize + login_form.password_field.is_some() as usize);

        // Detect and solve CAPTCHA if present
        let captcha_info = crate::captcha::CaptchaSolver::detect_captcha(&html, site_url);
        if captcha_info.captcha_type != crate::captcha::CaptchaType::None {
            info!("🤖 Login CAPTCHA detected: {:?}", captcha_info.captcha_type);

            // For image CAPTCHAs, capture from browser instead of downloading
            if captcha_info.captcha_type == crate::captcha::CaptchaType::ImageCaptcha {
                info!("Using browser screenshot method for CAPTCHA");

                // Capture CAPTCHA image from browser
                if let Ok(image_data) = browser.capture_captcha_image(&tab, "img[src*='captcha']") {
                    // Save to temp file
                    let temp_path = "/tmp/login_captcha.png";
                    if std::fs::write(temp_path, &image_data).is_ok() {
                        // Run OCR
                        if let Ok(output) = std::process::Command::new("tesseract")
                            .arg(temp_path)
                            .arg("stdout")
                            .arg("--psm").arg("7")
                            .arg("-c").arg("tessedit_char_whitelist=0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
                            .output() {

                            let captcha_text = String::from_utf8_lossy(&output.stdout)
                                .trim()
                                .to_string();

                            if !captcha_text.is_empty() {
                                info!("✓ Login CAPTCHA solved via browser screenshot: {}", captcha_text);

                                // Fill CAPTCHA solution
                                for field_name in &["captcha", "captcha_response", "verification", "code"] {
                                    let selector = format!("input[name='{}']", field_name);
                                    if browser.fill_field(&tab, &selector, &captcha_text).is_ok() {
                                        info!("Filled CAPTCHA field: {}", field_name);
                                        break;
                                    }
                                }
                            } else {
                                warn!("OCR returned empty result");
                            }
                        }
                    }
                }
            }
        }

        // Fill username
        if let Some(ref username_selector) = login_form.username_field {
            browser.fill_field(&tab, username_selector, username)?;
            info!("Filled username field");
        }

        // Fill password
        if let Some(ref password_selector) = login_form.password_field {
            browser.fill_field(&tab, password_selector, password)?;
            info!("Filled password field");
        }

        // Take screenshot before login
        browser.screenshot(&tab, "/tmp/login_before.png").ok();

        // Click login button
        if let Some(ref submit_selector) = login_form.submit_button {
            browser.click(&tab, submit_selector)?;
            info!("Clicked login button");
        } else {
            anyhow::bail!("No login button found");
        }

        // Wait for response
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Take screenshot after login
        browser.screenshot(&tab, "/tmp/login_after.png").ok();

        // Get final URL and HTML
        let final_url = if let Ok(url_js) = tab.evaluate("window.location.href", false) {
            url_js.value.and_then(|v| v.as_str().map(String::from))
        } else {
            None
        };

        let final_html = browser.get_rendered_html(&tab)?;
        let cookies = browser.get_cookies(&tab).ok();

        // Detect if login succeeded
        let success = Self::detect_login_success(&final_html, final_url.as_deref());

        Ok((success, final_url, cookies))
    }

    /// Find login form on page
    fn find_login_form(html: &str) -> Result<LoginForm> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);

        // Find form with login indicators
        let form_selector = Selector::parse("form").unwrap();

        for form in document.select(&form_selector) {
            let form_html = form.html().to_lowercase();

            // Check if this is a login form (more flexible matching)
            let is_login = form_html.contains("login") ||
                          form_html.contains("sign in") ||
                          form_html.contains("log in") ||
                          form_html.contains("signin") ||
                          form_html.contains("username") ||  // Has username field
                          form_html.contains("user");        // Or user field

            if is_login {
                // Find username/email field (including DNMX's login_username and secretkey)
                let username_field = Self::find_field(&form, &["user", "login", "email", "name", "login_username"]);

                // Find password field (including DNMX's secretkey)
                let password_field = Self::find_field(&form, &["password", "pass", "pwd", "secretkey", "secret"]);

                // Find submit button
                let submit_button = Self::find_submit(&form);

                if username_field.is_some() && password_field.is_some() {
                    return Ok(LoginForm {
                        username_field,
                        password_field,
                        submit_button,
                    });
                }
            }
        }

        anyhow::bail!("No login form found");
    }

    /// Find field by name patterns
    fn find_field(form: &scraper::ElementRef, patterns: &[&str]) -> Option<String> {
        use scraper::Selector;

        let input_selector = Selector::parse("input").unwrap();

        for input in form.select(&input_selector) {
            // Check name attribute
            if let Some(name) = input.value().attr("name") {
                let name_lower = name.to_lowercase();
                if patterns.iter().any(|p| name_lower.contains(p)) {
                    // Return selector
                    if let Some(id) = input.value().attr("id") {
                        return Some(format!("#{}", id));
                    } else {
                        return Some(format!("input[name='{}']", name));
                    }
                }
            }

            // Also check id attribute
            if let Some(id) = input.value().attr("id") {
                let id_lower = id.to_lowercase();
                if patterns.iter().any(|p| id_lower.contains(p)) {
                    return Some(format!("#{}", id));
                }
            }

            // Check placeholder
            if let Some(placeholder) = input.value().attr("placeholder") {
                let placeholder_lower = placeholder.to_lowercase();
                if patterns.iter().any(|p| placeholder_lower.contains(p)) {
                    if let Some(name) = input.value().attr("name") {
                        return Some(format!("input[name='{}']", name));
                    } else if let Some(id) = input.value().attr("id") {
                        return Some(format!("#{}", id));
                    }
                }
            }
        }

        None
    }

    /// Find submit button
    fn find_submit(form: &scraper::ElementRef) -> Option<String> {
        use scraper::Selector;

        // Try input[type=submit]
        if let Ok(selector) = Selector::parse("input[type='submit']") {
            if let Some(elem) = form.select(&selector).next() {
                if let Some(id) = elem.value().attr("id") {
                    return Some(format!("#{}", id));
                } else if let Some(name) = elem.value().attr("name") {
                    return Some(format!("input[name='{}']", name));
                } else {
                    return Some("input[type='submit']".to_string());
                }
            }
        }

        // Try button[type=submit]
        if let Ok(selector) = Selector::parse("button[type='submit']") {
            if let Some(elem) = form.select(&selector).next() {
                if let Some(id) = elem.value().attr("id") {
                    return Some(format!("#{}", id));
                } else if let Some(name) = elem.value().attr("name") {
                    return Some(format!("button[name='{}']", name));
                } else {
                    return Some("button[type='submit']".to_string());
                }
            }
        }

        // Try any button in form
        if let Ok(selector) = Selector::parse("button") {
            if let Some(elem) = form.select(&selector).next() {
                if let Some(name) = elem.value().attr("name") {
                    return Some(format!("button[name='{}']", name));
                }
                return Some("button".to_string());
            }
        }

        // Try input with name=submit, action, login, etc.
        for name in &["submit", "action", "login", "signin", "send"] {
            if let Ok(selector) = Selector::parse(&format!("input[name='{}']", name)) {
                if form.select(&selector).next().is_some() {
                    return Some(format!("input[name='{}']", name));
                }
            }
            if let Ok(selector) = Selector::parse(&format!("button[name='{}']", name)) {
                if form.select(&selector).next().is_some() {
                    return Some(format!("button[name='{}']", name));
                }
            }
        }

        None
    }

    /// Detect if login was successful
    fn detect_login_success(html: &str, final_url: Option<&str>) -> bool {
        let html_lower = html.to_lowercase();

        // Positive signals for email providers (DNMX shows SquirrelMail)
        let email_success = html_lower.contains("squirrelmail") ||
                           html_lower.contains("inbox") ||
                           html_lower.contains("compose") ||
                           html_lower.contains("folders") ||
                           html_lower.contains("your email:");

        // General success signals
        let general_success = html_lower.contains("dashboard") ||
                             html_lower.contains("welcome") ||
                             html_lower.contains("logout") ||
                             html_lower.contains("sign out") ||
                             html_lower.contains("my account") ||
                             html_lower.contains("profile");

        // Check URL changed
        let url_changed = if let Some(url) = final_url {
            url.contains("/inbox") || url.contains("/dashboard") || url.contains("/home") || url.contains("/profile")
        } else {
            false
        };

        // Negative signals
        let failure_signals = (html_lower.contains("invalid") && html_lower.contains("login")) ||
                             (html_lower.contains("incorrect") && html_lower.contains("password")) ||
                             html_lower.contains("login failed");

        (email_success || general_success || url_changed) && !failure_signals
    }

    /// Get accounts that need verification
    async fn get_accounts_to_verify(&self, network: Option<&str>) -> Result<Vec<AccountToVerify>> {
        let query = if let Some(net) = network {
            format!(
                "SELECT id, site_url, site_domain, network, username, password
                 FROM registered_accounts
                 WHERE network = '{}' AND status = 'active' AND last_login_at IS NULL
                 ORDER BY registered_at DESC
                 LIMIT 20",
                net
            )
        } else {
            "SELECT id, site_url, site_domain, network, username, password
             FROM registered_accounts
             WHERE status = 'active' AND last_login_at IS NULL
             ORDER BY registered_at DESC
             LIMIT 20".to_string()
        };

        let rows = sqlx::query_as::<_, (i64, String, String, String, String, String)>(&query)
            .fetch_all(self.storage.pool())
            .await?;

        Ok(rows.iter().map(|(id, site_url, domain, network, username, password)| {
            AccountToVerify {
                id: *id,
                site_url: site_url.clone(),
                domain: domain.clone(),
                network: network.clone(),
                username: username.clone(),
                password: password.clone(),
            }
        }).collect())
    }

    /// Update account status after verification
    async fn update_account_status(&self, result: &LoginVerificationResult) -> Result<()> {
        if result.login_successful {
            sqlx::query(
                "UPDATE registered_accounts SET status = 'verified', last_login_at = NOW() WHERE id = $1"
            )
            .bind(result.account_id)
            .execute(self.storage.pool())
            .await?;

            info!("✓ Account verified: {}", result.username);
        } else {
            sqlx::query(
                "UPDATE registered_accounts SET status = 'invalid' WHERE id = $1"
            )
            .bind(result.account_id)
            .execute(self.storage.pool())
            .await?;

            warn!("✗ Account invalid: {}", result.username);
        }

        Ok(())
    }

    fn get_proxy_for_network(network: &str) -> Option<String> {
        match network {
            "tor" => Some("socks5://tor1:9050".to_string()),
            "i2p" => Some("http://i2p1:4444".to_string()),
            "lokinet" => Some("socks5://lokinet1:1080".to_string()),
            "hyphanet" => Some("http://hyphanet1:8888".to_string()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct AccountToVerify {
    id: i64,
    site_url: String,
    domain: String,
    network: String,
    username: String,
    password: String,
}

#[derive(Debug)]
struct LoginForm {
    username_field: Option<String>,
    password_field: Option<String>,
    submit_button: Option<String>,
}
