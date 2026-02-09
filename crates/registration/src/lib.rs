// Core modules
pub mod core;
pub mod detection;
pub mod adapters;
pub mod orchestrator;
pub mod auto_register;
pub mod auto_register_inline;

// Legacy modules (will be phased out or refactored)
pub mod browser;
pub mod captcha;
pub mod captcha_free;
pub mod email;
pub mod form_filler;
pub mod registrar;
pub mod multilingual;
pub mod temp_email_providers;

// Re-exports for convenience
pub use core::types::*;
pub use core::context::*;
pub use core::result::*;
pub use detection::*;
pub use adapters::*;
pub use orchestrator::*;
pub use auto_register::*;
pub use auto_register_inline::*;


#[derive(Debug, Clone)]
pub struct RegistrationConfig {
    pub use_headless_browser: bool,
    pub browser_timeout_seconds: u64,
    pub wait_for_content_seconds: u64,
    pub captcha_service: Option<CaptchaService>,
    pub temp_email_domain: String,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub enum CaptchaService {
    Free,
    Disabled,
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            use_headless_browser: true,
            browser_timeout_seconds: 120,
            wait_for_content_seconds: 30,
            captcha_service: Some(CaptchaService::Free),  // 🆓 FREE by default!
            temp_email_domain: "secmail.pro".to_string(),  // Free Tor email
            user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegistrationAttempt {
    pub url: String,
    pub domain: String,
    pub network: String,
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub success: bool,
    pub error: Option<String>,
    pub captcha_encountered: bool,
    pub captcha_solved: bool,
    pub email_verification_required: bool,
    pub session_cookies: Option<String>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RegistrationError {
    #[error("Form not found on page")]
    FormNotFound,

    #[error("CAPTCHA required but no solver configured")]
    CaptchaRequired,

    #[error("Email verification required")]
    EmailVerificationRequired,

    #[error("Username already taken")]
    UsernameExists,

    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    #[error("Browser error: {0}")]
    BrowserError(String),

    #[error("Timeout waiting for content")]
    Timeout,

    #[error("Other error: {0}")]
    Other(String),
}
