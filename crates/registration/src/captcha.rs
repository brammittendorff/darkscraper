use regex::Regex;
use tracing::info;

use crate::{CaptchaService, RegistrationError};

#[derive(Debug, Clone)]
pub struct CaptchaInfo {
    pub captcha_type: CaptchaType,
    pub site_key: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaptchaType {
    ReCaptchaV2,
    ReCaptchaV3,
    HCaptcha,
    TextCaptcha,
    ImageCaptcha,
    None,
}

pub struct CaptchaSolver {
    service: CaptchaService,
}

impl CaptchaSolver {
    pub fn new(service: CaptchaService) -> Self {
        Self { service }
    }

    /// Detect CAPTCHA on page and extract necessary information
    pub fn detect_captcha(html: &str, _url: &str) -> CaptchaInfo {
        let html_lower = html.to_lowercase();

        // Detect reCAPTCHA v2
        if html_lower.contains("g-recaptcha") || html_lower.contains("recaptcha") {
            if let Some(site_key) = extract_recaptcha_key(html) {
                info!("detected reCAPTCHA v2 with site key: {}", site_key);
                return CaptchaInfo {
                    captcha_type: CaptchaType::ReCaptchaV2,
                    site_key: Some(site_key),
                    image_url: None,
                };
            }
        }

        // Detect reCAPTCHA v3
        if html_lower.contains("grecaptcha.execute") {
            if let Some(site_key) = extract_recaptcha_key(html) {
                info!("detected reCAPTCHA v3 with site key: {}", site_key);
                return CaptchaInfo {
                    captcha_type: CaptchaType::ReCaptchaV3,
                    site_key: Some(site_key),
                    image_url: None,
                };
            }
        }

        // Detect hCaptcha
        if html_lower.contains("h-captcha") || html_lower.contains("hcaptcha") {
            if let Some(site_key) = extract_hcaptcha_key(html) {
                info!("detected hCaptcha with site key: {}", site_key);
                return CaptchaInfo {
                    captcha_type: CaptchaType::HCaptcha,
                    site_key: Some(site_key),
                    image_url: None,
                };
            }
        }

        // Detect image CAPTCHA (common in darknet sites)
        if let Some(captcha_img) = extract_captcha_image(html) {
            info!("detected image CAPTCHA: {}", captcha_img);
            return CaptchaInfo {
                captcha_type: CaptchaType::ImageCaptcha,
                site_key: None,
                image_url: Some(captcha_img),
            };
        }

        // Detect text CAPTCHA (simple math/text challenges)
        if html_lower.contains("captcha") && (html_lower.contains("what is") || html_lower.contains("solve")) {
            info!("detected text CAPTCHA");
            return CaptchaInfo {
                captcha_type: CaptchaType::TextCaptcha,
                site_key: None,
                image_url: None,
            };
        }

        CaptchaInfo {
            captcha_type: CaptchaType::None,
            site_key: None,
            image_url: None,
        }
    }

    /// Solve CAPTCHA using free methods only
    pub async fn solve_captcha(
        &self,
        captcha_info: &CaptchaInfo,
        page_url: &str,
    ) -> Result<String, RegistrationError> {
        match &self.service {
            CaptchaService::Free => {
                let free_solver = crate::captcha_free::FreeCaptchaSolver::new();
                free_solver.solve_captcha_free(captcha_info, "", page_url).await
            }
            CaptchaService::Disabled => {
                Err(RegistrationError::CaptchaRequired)
            }
        }
    }
}

/// Extract reCAPTCHA site key from HTML
fn extract_recaptcha_key(html: &str) -> Option<String> {
    let patterns = vec![
        r#"data-sitekey="([^"]+)""#,
        r#"sitekey:\s*["']([^"']+)["']"#,
        r#"grecaptcha\.render\([^,]+,\s*\{[^}]*sitekey:\s*["']([^"']+)["']"#,
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(html) {
                if let Some(key) = captures.get(1) {
                    return Some(key.as_str().to_string());
                }
            }
        }
    }

    None
}

/// Extract hCaptcha site key from HTML
fn extract_hcaptcha_key(html: &str) -> Option<String> {
    let patterns = vec![
        r#"data-sitekey="([^"]+)""#,
        r#"sitekey:\s*["']([^"']+)["']"#,
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(html) {
                if let Some(key) = captures.get(1) {
                    return Some(key.as_str().to_string());
                }
            }
        }
    }

    None
}

/// Extract CAPTCHA image URL from HTML
fn extract_captcha_image(html: &str) -> Option<String> {
    let patterns = vec![
        r#"<img[^>]+src="([^"]*captcha[^"]*)"#,
        r#"<img[^>]+src="([^"]*verification[^"]*)"#,
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(html) {
                if let Some(url) = captures.get(1) {
                    return Some(url.as_str().to_string());
                }
            }
        }
    }

    None
}
