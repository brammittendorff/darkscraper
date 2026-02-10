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
    verification_helpers,
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

    // PRIORITY: Ensure we have at least 2 email provider accounts before registering elsewhere
    let is_email_provider = crate::email_provider_priority::is_email_provider(&page.domain);
    let email_account_count = storage.count_dnmx_accounts().await.unwrap_or(0);

    // Skip non-email sites until we have 2 email accounts
    if !is_email_provider && email_account_count < 2 {
        info!("Skipping {} - need 2 email accounts first (have {})", page.domain, email_account_count);
        return Ok(false);
    }

    // For email providers, create up to 5 accounts
    if is_email_provider {
        info!("🎯 Auto-registering on EMAIL PROVIDER {} ({}) - HIGH PRIORITY", page.domain, page.network);
        info!("Current email accounts: {} (will create up to 5)", email_account_count);
    }

    // Check if we already have accounts on this domain (respect max_per_domain)
    let max_accounts = if is_email_provider { 5 } else { 3 };  // More email accounts
    let existing_accounts = storage.count_accounts_for_domain(&page.domain).await?;
    if existing_accounts >= max_accounts {
        info!("Domain {} already has {} accounts (max {}), skipping", page.domain, existing_accounts, max_accounts);
        return Ok(false);
    }

    // Check if we already tried and failed recently
    if let Ok(recent_failures) = storage.count_recent_failed_attempts(&page.domain, 3600).await {
        if recent_failures >= 3 {
            warn!("Domain {} has {} recent failures, skipping", page.domain, recent_failures);
            return Ok(false);
        }
    }

    if !is_email_provider {
        info!("🎯 Auto-registering on {} ({})", page.domain, page.network);
    }

    // Generate registration data
    let mut data = generate_registration_data(page, is_email_provider);

    // If site needs email and we have email provider accounts, use them
    if page.metadata.requires_email && !is_email_provider {
        if let Ok(provider_emails) = crate::email_provider_priority::get_available_dnmx_emails(storage).await {
            if !provider_emails.is_empty() {
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                let selected_email = provider_emails.choose(&mut rng).unwrap();
                info!("Using email provider account: {}", selected_email);
                data.email = Some(selected_email.clone());
            }
        }
    }

    // Create registration engine with longer timeouts for dark web
    let config = RegistrationConfig {
        use_headless_browser: true,
        browser_timeout_seconds: 300,  // 5 minutes total timeout
        wait_for_content_seconds: 120,  // 2 minutes to wait for content (dark web is slow!)
        captcha_service: Some(crate::CaptchaService::Free),  // Enable CAPTCHA solver
        temp_email_domain: "secmail.pro".to_string(),
        user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0".to_string(),
    };

    let engine = RegistrationEngine::new(config.clone(), storage.clone());

    // Attempt registration
    match engine.register(&page.url, data.clone(), proxy_url).await {
        Ok(result) => {
            if result.is_success() {
                info!("✓ Auto-registration successful on {}", page.domain);

                // COMPLETE VERIFICATION FLOW
                let mut account_verified = false;

                // DIFFERENT flow for email providers vs regular sites
                if is_email_provider {
                    // EMAIL PROVIDERS (DNMX, cock.li): Success message confirms account creation
                    // No need to login - the success message is proof enough!
                    info!("✅ EMAIL ACCOUNT CREATED: {}@dnmx.cc or {}@cock.li", data.username, data.username);
                    account_verified = true;

                    // Mark as verified since success message appeared
                    storage.update_account_status(&page.domain, &data.username, "verified").await.ok();

                } else if result.result.requires_email_verification {
                    info!("📧 Email verification required, checking inbox...");

                    if let Some(ref email) = data.email {
                        let email_verifier = crate::email_verifier::EmailVerifier::new("secmail.pro".to_string());

                        // Check inbox and get verification link
                        match email_verifier.verify_email(email, &page.domain).await {
                            Ok(email_result) => {
                                if email_result.verified {
                                    if let Some(ref verify_link) = email_result.verification_link {
                                        info!("✓ Found verification link: {}", verify_link);

                                        // Click verification link
                                        match verification_helpers::click_verification_link(verify_link, proxy_url, &config).await {
                                            Ok(true) => {
                                                info!("✓ Email verified successfully");

                                                // Update database
                                                if let Err(e) = storage.update_email_verified(&page.domain, &data.username).await {
                                                    warn!("Failed to update email verified status: {}", e);
                                                }
                                            }
                                            Ok(false) => {
                                                warn!("✗ Failed to click verification link");
                                            }
                                            Err(e) => {
                                                warn!("✗ Error clicking verification link: {}", e);
                                            }
                                        }
                                    }
                                } else {
                                    warn!("✗ No verification email received: {:?}", email_result.error);
                                }
                            }
                            Err(e) => {
                                warn!("Email verification error: {}", e);
                            }
                        }
                    }
                }

                // Step 2: Verify account works by logging in
                info!("🔐 Verifying account by attempting login...");

                // Wait 30 seconds for account activation (DNMX and similar sites need this)
                info!("Waiting 30 seconds for account activation...");
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

                let verifier = crate::login_verifier::LoginVerifier::new(storage.clone(), config.clone());

                match verifier.attempt_login(&page.url, &data.username, &data.password, proxy_url).await {
                    Ok((login_success, logged_url, _cookies)) => {
                        if login_success {
                            info!("✓ Login verification successful for {} @ {}", data.username, page.domain);
                            account_verified = true;

                            // Update last_login_at in database
                            if let Err(e) = storage.update_last_login(&page.domain, &data.username).await {
                                warn!("Failed to update last_login_at: {}", e);
                            }
                        } else {
                            warn!("✗ Login verification failed for {} @ {} - account may not be real", data.username, page.domain);

                            // Mark account as invalid
                            if let Err(e) = storage.update_account_status(&page.domain, &data.username, "invalid").await {
                                warn!("Failed to update account status: {}", e);
                            }
                        }

                        if let Some(url) = logged_url {
                            info!("Logged in URL: {}", url);
                        }
                    }
                    Err(e) => {
                        warn!("Login verification error: {}", e);
                    }
                }

                if account_verified {
                    info!("✅ FULL VERIFICATION COMPLETE: {} @ {} is a REAL working account!", data.username, page.domain);
                } else {
                    warn!("⚠️  Account created but not verified: {} @ {}", data.username, page.domain);
                }

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
fn generate_registration_data(page: &PageData, is_email_provider: bool) -> RegistrationData {
    // Generate realistic username with English words
    let username = crate::email::generate_random_username();

    // Generate strong password
    let password = generate_strong_password();

    // Generate email
    // IMPORTANT: Email providers (DNMX, cock.li) ALWAYS need email to register
    // Use secmail.pro for bootstrapping email providers
    let email = if is_email_provider {
        // Email providers always need email (use secmail.pro)
        Some(format!("{}@secmail.pro", username.to_lowercase()))
    } else if page.metadata.requires_email {
        // Other sites: will be replaced with DNMX/cock.li emails later
        Some(format!("{}@secmail.pro", username.to_lowercase()))
    } else {
        // Sites that don't need email
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
