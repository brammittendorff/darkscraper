use anyhow::Result;
use tracing::{info, warn, error};
use url::Url;

use darkscraper_storage::Storage;

use crate::{
    browser::HeadlessBrowser,
    captcha::{CaptchaSolver, CaptchaType},
    email::TempEmailService,
    form_filler::FormAnalyzer,
    RegistrationAttempt, RegistrationConfig, RegistrationError,
};

pub struct Registrar {
    config: RegistrationConfig,
    storage: Storage,
    email_service: TempEmailService,
    captcha_solver: Option<CaptchaSolver>,
}

impl Registrar {
    pub fn new(config: RegistrationConfig, storage: Storage) -> Self {
        let email_service = TempEmailService::new(config.temp_email_domain.clone());

        let captcha_solver = if let Some(ref service) = config.captcha_service {
            Some(CaptchaSolver::new(service.clone()))
        } else {
            None
        };

        Self {
            config,
            storage,
            email_service,
            captcha_solver,
        }
    }

    /// Attempt to register an account on a given site
    pub async fn register(
        &self,
        url: &str,
        proxy_url: Option<&str>,
    ) -> Result<RegistrationAttempt, RegistrationError> {
        info!("attempting registration at {}", url);

        let parsed_url = Url::parse(url)
            .map_err(|e| RegistrationError::RegistrationFailed(e.to_string()))?;

        let domain = parsed_url.host_str().unwrap_or("").to_string();
        let network = Self::detect_network(&domain);

        // Generate credentials
        let username = crate::email::generate_random_username();
        let password = crate::email::generate_random_password();
        let _email: Option<String> = None;

        let mut attempt = RegistrationAttempt {
            url: url.to_string(),
            domain: domain.clone(),
            network: network.clone(),
            username: username.clone(),
            password: password.clone(),
            email: None,
            success: false,
            error: None,
            captcha_encountered: false,
            captcha_solved: false,
            email_verification_required: false,
            session_cookies: None,
        };

        // Use headless browser for JavaScript-rendered content
        if self.config.use_headless_browser {
            match self.register_with_browser(url, proxy_url, &username, &password).await {
                Ok(result) => {
                    attempt = result;
                }
                Err(e) => {
                    error!("registration failed: {}", e);
                    attempt.error = Some(e.to_string());
                }
            }
        } else {
            attempt.error = Some("Non-browser registration not implemented".to_string());
        }

        // Store attempt in database
        self.storage
            .record_registration_attempt(
                &attempt.url,
                &attempt.domain,
                &attempt.network,
                Some(&attempt.username),
                attempt.email.as_deref(),
                attempt.captcha_encountered,
                attempt.captcha_solved,
                attempt.email_verification_required,
                false, // email_verified (TODO: implement verification)
                attempt.success,
                attempt.error.as_deref(),
                None, // account_id (will be set if successful)
            )
            .await
            .ok();

        // If successful, store account
        if attempt.success {
            match self
                .storage
                .store_registered_account(
                    &attempt.url,
                    &attempt.domain,
                    &attempt.network,
                    &attempt.username,
                    &attempt.password,
                    attempt.email.as_deref(),
                    None, // email_provider
                    &attempt.url,
                    attempt.session_cookies.as_deref(),
                    &self.config.user_agent,
                    None, // notes
                )
                .await
            {
                Ok(account_id) => {
                    info!("account created successfully: id={}", account_id);
                }
                Err(e) => {
                    warn!("failed to store account: {}", e);
                }
            }
        }

        Ok(attempt)
    }

    /// Register using headless browser (handles JS, waiting screens, etc.)
    async fn register_with_browser(
        &self,
        url: &str,
        proxy_url: Option<&str>,
        username: &str,
        password: &str,
    ) -> Result<RegistrationAttempt, RegistrationError> {
        // Create browser instance
        let browser = HeadlessBrowser::new(self.config.clone(), proxy_url)
            .map_err(|e| RegistrationError::BrowserError(e.to_string()))?;

        // Navigate and wait for content to load (handles waiting screens)
        let tab = browser.navigate_and_wait(url).await?;

        // Get rendered HTML after JavaScript execution
        let html = browser.get_rendered_html(&tab)?;

        // Detect CAPTCHA
        let captcha_info = if let Some(ref _solver) = self.captcha_solver {
            crate::captcha::CaptchaSolver::detect_captcha(&html, url)
        } else {
            crate::captcha::CaptchaInfo {
                captcha_type: CaptchaType::None,
                site_key: None,
                image_url: None,
            }
        };

        let mut captcha_solution = None;
        let captcha_encountered = captcha_info.captcha_type != CaptchaType::None;

        // Solve CAPTCHA if present
        if captcha_encountered {
            if let Some(ref solver) = self.captcha_solver {
                info!("CAPTCHA detected, attempting to solve");
                match solver.solve_captcha(&captcha_info, url).await {
                    Ok(solution) => {
                        info!("CAPTCHA solved successfully");
                        captcha_solution = Some(solution);
                    }
                    Err(e) => {
                        warn!("CAPTCHA solve failed: {}", e);
                        return Err(e);
                    }
                }
            } else {
                warn!("CAPTCHA required but no solver configured");
                return Err(RegistrationError::CaptchaRequired);
            }
        }

        // Analyze form
        let form = FormAnalyzer::find_registration_form(&html)?;

        // Generate email if required
        let email = if form.has_email {
            Some(self.email_service.create_email())
        } else {
            None
        };

        // Create field mapping
        let field_mapping = FormAnalyzer::create_field_mapping(&form, username, password, email.as_deref());

        // Fill form fields
        for (selector, value) in &field_mapping {
            browser.fill_field(&tab, selector, value)?;
        }

        // Fill CAPTCHA response if solved
        if let Some(ref solution) = captcha_solution {
            // Try common CAPTCHA response field names
            let captcha_fields = vec![
                "g-recaptcha-response",
                "h-captcha-response",
                "captcha",
                "captcha_response",
            ];

            for field in captcha_fields {
                if html.contains(field) {
                    browser.fill_field(&tab, &format!("textarea[name='{}']", field), &solution).ok();
                    break;
                }
            }
        }

        // Take screenshot before submission (for debugging)
        browser.screenshot(&tab, "/tmp/registration_before.png").ok();

        // Find and click submit button
        let submit_selectors = vec![
            "button[type='submit']",
            "input[type='submit']",
            "button:contains('Register')",
            "button:contains('Sign Up')",
            "button:contains('Create Account')",
        ];

        let mut submitted = false;
        for selector in submit_selectors {
            if browser.click(&tab, selector).is_ok() {
                info!("clicked submit button: {}", selector);
                submitted = true;
                break;
            }
        }

        if !submitted {
            return Err(RegistrationError::FormNotFound);
        }

        // Wait for submission to complete
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        // Take screenshot after submission
        browser.screenshot(&tab, "/tmp/registration_after.png").ok();

        // Get final HTML to check for success/error messages
        let final_html = browser.get_rendered_html(&tab)?;
        let final_html_lower = final_html.to_lowercase();

        // Check for success indicators
        let success = final_html_lower.contains("success") ||
                     final_html_lower.contains("welcome") ||
                     final_html_lower.contains("registered") ||
                     final_html_lower.contains("verify your email") ||
                     !final_html_lower.contains("error");

        // Get session cookies
        let cookies = browser.get_cookies(&tab).ok();

        // Check if email verification is required
        let email_verification_required = final_html_lower.contains("verify") &&
                                         final_html_lower.contains("email");

        Ok(RegistrationAttempt {
            url: url.to_string(),
            domain: Url::parse(url).unwrap().host_str().unwrap_or("").to_string(),
            network: Self::detect_network(&url),
            username: username.to_string(),
            password: password.to_string(),
            email,
            success,
            error: if success { None } else { Some("Unknown error".to_string()) },
            captcha_encountered,
            captcha_solved: captcha_solution.is_some(),
            email_verification_required,
            session_cookies: cookies,
        })
    }

    fn detect_network(url: &str) -> String {
        if url.contains(".onion") {
            "tor".to_string()
        } else if url.contains(".i2p") {
            "i2p".to_string()
        } else if url.contains(".loki") {
            "lokinet".to_string()
        } else if url.contains(".bit") {
            "zeronet".to_string()
        } else if url.contains("hyphanet:") || url.contains("freenet:") {
            "hyphanet".to_string()
        } else {
            "clearnet".to_string()
        }
    }
}
