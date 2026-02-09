/// Inline auto-registration - attempts registration immediately when form is detected
use anyhow::Result;
use darkscraper_storage::Storage;
use darkscraper_core::PageData;
use tracing::{info, warn};
use std::collections::HashMap;

use crate::{
    RegistrationEngine,
    RegistrationConfig,
    RegistrationData,
};

/// Try to auto-register on a page if it has a registration form
pub async fn try_auto_register(
    page: &PageData,
    storage: &Storage,
    proxy_url: Option<&str>,
) -> Result<bool> {
    // Check if page has registration form
    if !page.metadata.has_register_form {
        return Ok(false);
    }

    // Check if we already have accounts on this domain (respect max_per_domain)
    let existing_accounts = storage.count_accounts_for_domain(&page.domain).await?;
    if existing_accounts >= 3 {
        info!("Domain {} already has {} accounts, skipping", page.domain, existing_accounts);
        return Ok(false);
    }

    // Check if we already tried and failed recently
    if let Ok(recent_failures) = storage.count_recent_failed_attempts(&page.domain, 3600).await {
        if recent_failures >= 3 {
            warn!("Domain {} has {} recent failures, skipping", page.domain, recent_failures);
            return Ok(false);
        }
    }

    info!("🎯 Auto-registering on {} ({})", page.domain, page.network);

    // Generate registration data
    let data = generate_registration_data(page);

    // Create registration engine with longer timeouts for dark web
    let config = RegistrationConfig {
        use_headless_browser: true,
        browser_timeout_seconds: 300,  // 5 minutes total timeout
        wait_for_content_seconds: 120,  // 2 minutes to wait for content (dark web is slow!)
        captcha_service: if page.metadata.has_captcha {
            None  // Skip CAPTCHA sites for now
        } else {
            Some(crate::CaptchaService::Free)
        },
        temp_email_domain: "secmail.pro".to_string(),
        user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0".to_string(),
    };

    let engine = RegistrationEngine::new(config, storage.clone());

    // Attempt registration
    match engine.register(&page.url, data, proxy_url).await {
        Ok(result) => {
            if result.is_success() {
                info!("✓ Auto-registration successful on {}", page.domain);
                Ok(true)
            } else {
                warn!("✗ Auto-registration failed on {}: {:?}", page.domain, result.result.error);
                Ok(false)
            }
        }
        Err(e) => {
            warn!("✗ Auto-registration error on {}: {}", page.domain, e);
            Ok(false)
        }
    }
}

/// Generate random registration data
fn generate_registration_data(page: &PageData) -> RegistrationData {
    use rand::Rng;
    use rand::distributions::Alphanumeric;

    let mut rng = rand::thread_rng();

    // Generate random username (8-12 chars)
    let username_len = rng.gen_range(8..13);
    let username: String = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(username_len)
        .collect();

    // Generate strong password
    let password = generate_strong_password();

    // Generate email if required
    let email = if page.metadata.requires_email {
        Some(format!("{}@secmail.pro", username.to_lowercase()))
    } else {
        None
    };

    RegistrationData {
        username,
        email,
        password: password.clone(),
        password_confirm: Some(password),
        first_name: None,
        last_name: None,
        date_of_birth: None,
        country: None,
        accept_terms: true,
        subscribe_newsletter: false,
        captcha_solution: None,
        custom_fields: HashMap::new(),
    }
}

/// Generate a strong random password that meets most site requirements
fn generate_strong_password() -> String {
    use rand::Rng;

    let mut rng = rand::thread_rng();

    let lowercase = "abcdefghijklmnopqrstuvwxyz";
    let uppercase = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let numbers = "0123456789";
    let special = "!@#$%^&*";

    let mut password = String::new();

    // Ensure at least 2 of each type for stricter requirements
    for _ in 0..2 {
        password.push(lowercase.chars().nth(rng.gen_range(0..lowercase.len())).unwrap());
        password.push(uppercase.chars().nth(rng.gen_range(0..uppercase.len())).unwrap());
        password.push(numbers.chars().nth(rng.gen_range(0..numbers.len())).unwrap());
        password.push(special.chars().nth(rng.gen_range(0..special.len())).unwrap());
    }

    // Fill to 20 characters total (very strong)
    let all_chars = format!("{}{}{}{}", lowercase, uppercase, numbers, special);
    for _ in 0..12 {
        password.push(all_chars.chars().nth(rng.gen_range(0..all_chars.len())).unwrap());
    }

    // Shuffle
    let mut chars: Vec<char> = password.chars().collect();
    for i in 0..chars.len() {
        let j = rng.gen_range(0..chars.len());
        chars.swap(i, j);
    }

    chars.iter().collect()
}
