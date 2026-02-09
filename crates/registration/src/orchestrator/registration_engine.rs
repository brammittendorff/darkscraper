use anyhow::Result;
use tracing::{info, warn, error};
use url::Url;
use darkscraper_storage::Storage;

use crate::core::*;
use crate::adapters::GenericAdapter;
use crate::browser::HeadlessBrowser;
use crate::orchestrator::link_detector;
use crate::RegistrationConfig;

/// Main registration engine that orchestrates the entire process
pub struct RegistrationEngine {
    config: RegistrationConfig,
    adapter: Box<dyn SiteAdapter>,
    storage: Storage,
}

impl RegistrationEngine {
    /// Create a new registration engine with default generic adapter
    pub fn new(config: RegistrationConfig, storage: Storage) -> Self {
        Self {
            config,
            adapter: Box::new(GenericAdapter::new()),
            storage,
        }
    }

    fn detect_network(url: &str) -> String {
        if url.contains(".onion") {
            "tor".to_string()
        } else if url.contains(".i2p") {
            "i2p".to_string()
        } else if url.contains(".loki") {
            "lokinet".to_string()
        } else {
            "clearnet".to_string()
        }
    }

    /// Register an account on a site
    pub async fn register(
        &self,
        url: &str,
        data: RegistrationData,
        proxy_url: Option<&str>,
    ) -> Result<DetailedRegistrationResult> {
        let parsed_url = Url::parse(url)?;
        let mut context = RegistrationContext::new(parsed_url.clone(), data, 3);
        let storage = &self.storage;

        info!("Starting registration at {}", url);

        // Create browser instance
        context.transition(
            RegistrationState::NavigatingToPage { url: url.to_string() },
            Some("Creating browser instance".to_string()),
        );

        let browser = HeadlessBrowser::new(self.config.clone(), proxy_url)?;

        // Navigate to page
        context.transition(RegistrationState::LoadingPage, Some("Navigating to page".to_string()));

        let tab = match browser.navigate_and_wait(url).await {
            Ok(tab) => tab,
            Err(e) => {
                let error = crate::RegistrationError::BrowserError(e.to_string());
                context.transition(
                    RegistrationState::Failed(error.clone()),
                    Some(format!("Navigation failed: {}", e)),
                );
                return self.create_failed_result(context, error);
            }
        };

        // Get initial page HTML
        let html = browser.get_rendered_html(&tab)?;
        context.record_page_state(html.clone(), vec![]);

        // IMPORTANT: Many sites have login pages with "Register" links
        // Try to find and click register link first
        info!("Checking for register link before form detection");
        if self.try_click_register_link(&browser, &tab, &html).await? {
            info!("Clicked register link, waiting for page load");
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Get the new page HTML after clicking register link
            let new_html = browser.get_rendered_html(&tab)?;
            context.record_page_state(new_html.clone(), vec![]);

            // Update context URL if changed
            if let Ok(new_url_js) = tab.evaluate("window.location.href", false) {
                if let Some(new_url) = new_url_js.value.and_then(|v| v.as_str().map(String::from)) {
                    info!("Navigated to registration page: {}", new_url);
                    context.url = Url::parse(&new_url).unwrap_or(parsed_url.clone());
                }
            }
        }

        // Detect form
        context.transition(RegistrationState::DetectingForm, Some("Analyzing page for forms".to_string()));

        let form_info = match self.adapter.detect_form(&html, url).await {
            Ok(form) => form,
            Err(e) => {
                error!("Failed to detect form: {}", e);

                // Save failed attempt to database
                let error = crate::RegistrationError::FormNotFound;
                if let Err(db_err) = storage.record_registration_attempt(
                    url,
                    &parsed_url.host_str().unwrap_or(""),
                    &Self::detect_network(url),
                    Some(&context.data.username),
                    context.data.email.as_deref(),
                    false,  // captcha_required
                    false,  // captcha_solved
                    false,  // email_verification_required
                    false,  // email_verified
                    false,  // success
                    Some("Form not found on page"),
                    None,   // account_id
                ).await {
                    warn!("Failed to save registration attempt to DB: {}", db_err);
                }

                context.transition(
                    RegistrationState::Failed(error.clone()),
                    Some(format!("Form detection failed: {}", e)),
                );
                return self.create_failed_result(context, error);
            }
        };

        info!(
            "Detected {} form with {} fields",
            match form_info.form_type {
                FormType::Registration => "registration",
                FormType::Login => "login",
                FormType::Combined => "combined",
                FormType::Unknown => "unknown",
            },
            form_info.fields.len()
        );

        context.transition(
            RegistrationState::FormDetected(form_info.clone()),
            Some(format!("Found form with confidence {:.2}", form_info.confidence)),
        );

        // Fill form
        context.transition(
            RegistrationState::FillingFields { step: 1, total_steps: 1 },
            Some("Filling form fields".to_string()),
        );

        let field_mapping = self.adapter.fill_form(&form_info, &context.data).await?;

        for (selector, value) in &field_mapping {
            match browser.fill_field(&tab, selector, value) {
                Ok(_) => info!("Filled field: {}", selector),
                Err(e) => warn!("Failed to fill field {}: {}", selector, e),
            }
        }

        // Take screenshot before submit
        browser.screenshot(&tab, "/tmp/registration_before.png").ok();

        // Submit form
        context.transition(RegistrationState::Submitting, Some("Submitting form".to_string()));

        let submit_button = self.adapter.submit_form(&form_info).await?;

        match browser.click(&tab, &submit_button.selector) {
            Ok(_) => info!("Clicked submit button: {}", submit_button.selector),
            Err(e) => {
                error!("Failed to click submit button: {}", e);
                let error = crate::RegistrationError::FormNotFound;
                context.transition(
                    RegistrationState::Failed(error.clone()),
                    Some(format!("Submit failed: {}", e)),
                );
                return self.create_failed_result(context, error);
            }
        }

        // Wait for response
        context.transition(RegistrationState::AwaitingResponse, Some("Waiting for response".to_string()));

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Get final HTML and cookies
        let final_html = browser.get_rendered_html(&tab)?;
        let cookies_json = browser.get_cookies(&tab).ok();
        context.record_page_state(final_html.clone(), vec![]);

        // Take screenshot after submit
        browser.screenshot(&tab, "/tmp/registration_after.png").ok();

        // Detect result
        let before_state = context.previous_page_state().unwrap();
        let after_state = context.current_page_state().unwrap();

        let mut result = self.adapter.detect_result(before_state, after_state).await?;

        // Fill in missing account info
        if result.success {
            if let Some(ref mut account_info) = result.account_info {
                account_info.username = context.data.username.clone();
                account_info.email = context.data.email.clone();
            }
            result.session_cookies = cookies_json.clone();

            // Save successful registration to database
            if let Err(e) = storage.record_registration_attempt(
                url,
                parsed_url.host_str().unwrap_or(""),
                &Self::detect_network(url),
                Some(&context.data.username),
                context.data.email.as_deref(),
                false,  // TODO: Track actual CAPTCHA
                false,
                result.requires_email_verification,
                false,
                true,  // success
                None,
                None,
            ).await {
                warn!("Failed to save successful registration to DB: {}", e);
            }

            // Store account details
            if let Err(e) = storage.store_registered_account(
                url,
                parsed_url.host_str().unwrap_or(""),
                &Self::detect_network(url),
                &context.data.username,
                &context.data.password,
                context.data.email.as_deref(),
                Some("secmail.pro"),
                url,
                cookies_json.as_deref(),
                &self.config.user_agent,
                None,
            ).await {
                warn!("Failed to store account to DB: {}", e);
            }

            context.transition(
                RegistrationState::Success(result.clone()),
                Some("Registration successful".to_string()),
            );

            info!("✓ Registration successful for '{}'", context.data.username);
        } else {
            let error = crate::RegistrationError::RegistrationFailed(
                result.error.clone().unwrap_or_else(|| "Unknown error".to_string())
            );

            // Save failed registration to database
            if let Err(e) = storage.record_registration_attempt(
                url,
                parsed_url.host_str().unwrap_or(""),
                &Self::detect_network(url),
                Some(&context.data.username),
                context.data.email.as_deref(),
                false,  // TODO: Track actual CAPTCHA
                false,
                result.requires_email_verification,
                false,
                false,  // success = false
                result.error.as_deref(),
                None,
            ).await {
                warn!("Failed to save failed registration to DB: {}", e);
            }

            context.transition(
                RegistrationState::Failed(error.clone()),
                Some(format!("Registration failed: {:?}", result.error)),
            );

            warn!("✗ Registration failed: {:?}", result.error);
        }

        Ok(DetailedRegistrationResult::new(result, context))
    }

    /// Create a failed result
    fn create_failed_result(
        &self,
        context: RegistrationContext,
        error: crate::RegistrationError,
    ) -> Result<DetailedRegistrationResult> {
        let result = RegistrationResult {
            success: false,
            account_info: None,
            error: Some(error.to_string()),
            requires_email_verification: false,
            session_cookies: None,
            evidence: vec![],
        };

        Ok(DetailedRegistrationResult::new(result, context))
    }

    /// Try to find and click a register link (for login pages with register links)
    async fn try_click_register_link(
        &self,
        browser: &HeadlessBrowser,
        tab: &std::sync::Arc<headless_chrome::Tab>,
        html: &str,
    ) -> Result<bool> {
        // Check if this looks like a login page
        if !link_detector::is_login_page(html) {
            info!("Page doesn't look like a login page, skipping link click");
            return Ok(false);
        }

        info!("Page looks like login page with register link, attempting to click");

        // Try multiple selectors for register links
        let selectors = link_detector::get_register_link_selectors();

        // Convert selectors to string slices
        let selector_refs: Vec<&str> = selectors.iter().map(|s| *s).collect();

        if browser.try_click(tab, &selector_refs)? {
            info!("✓ Successfully clicked register link");
            return Ok(true);
        }

        info!("No register link found");
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_network() {
        assert_eq!(RegistrationEngine::detect_network("http://example.onion"), "tor");
        assert_eq!(RegistrationEngine::detect_network("http://example.i2p"), "i2p");
        assert_eq!(RegistrationEngine::detect_network("http://example.loki"), "lokinet");
    }
}
