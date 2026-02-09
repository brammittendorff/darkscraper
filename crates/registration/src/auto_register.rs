use anyhow::Result;
use darkscraper_storage::Storage;
use tokio::sync::Semaphore;
use tracing::{info, warn, error};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    RegistrationConfig,
    RegistrationData,
    orchestrator::RegistrationEngine,
};

/// Configuration for auto-registration
#[derive(Debug, Clone)]
pub struct AutoRegisterConfig {
    pub max_concurrent: usize,
    pub max_accounts_per_domain: usize,
    pub networks: Vec<String>,  // tor, i2p, lokinet, hyphanet
    pub skip_captcha_sites: bool,
    pub skip_email_verification_sites: bool,
    pub min_registration_interval_seconds: u64,
    pub registration_config: RegistrationConfig,
}

impl Default for AutoRegisterConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            max_accounts_per_domain: 3,
            networks: vec!["tor".to_string(), "i2p".to_string(), "lokinet".to_string()],
            skip_captcha_sites: true,  // Skip CAPTCHA sites by default (can enable later)
            skip_email_verification_sites: false,
            min_registration_interval_seconds: 60,
            registration_config: RegistrationConfig::default(),
        }
    }
}

/// Statistics for auto-registration session
#[derive(Debug, Clone)]
pub struct AutoRegisterStats {
    pub total_attempts: usize,
    pub successful: usize,
    pub failed: usize,
    pub captcha_blocked: usize,
    pub email_verification_required: usize,
    pub sites_processed: usize,
}

impl AutoRegisterStats {
    pub fn new() -> Self {
        Self {
            total_attempts: 0,
            successful: 0,
            failed: 0,
            captcha_blocked: 0,
            email_verification_required: 0,
            sites_processed: 0,
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            (self.successful as f64 / self.total_attempts as f64) * 100.0
        }
    }
}

/// Auto-registration worker
pub struct AutoRegister {
    config: AutoRegisterConfig,
    storage: Storage,
    stats: Arc<tokio::sync::RwLock<AutoRegisterStats>>,
}

impl AutoRegister {
    pub fn new(config: AutoRegisterConfig, storage: Storage) -> Self {
        Self {
            config,
            storage,
            stats: Arc::new(tokio::sync::RwLock::new(AutoRegisterStats::new())),
        }
    }

    /// Start auto-registration worker
    pub async fn run(&self) -> Result<AutoRegisterStats> {
        info!("🤖 Starting auto-registration worker");
        info!("Config: max_concurrent={}, networks={:?}",
            self.config.max_concurrent,
            self.config.networks
        );

        // Find registration opportunities
        let opportunities = self.find_opportunities().await?;

        info!("Found {} registration opportunities", opportunities.len());

        if opportunities.is_empty() {
            warn!("No registration opportunities found. Make sure crawler has discovered sites with registration forms.");
            return Ok(self.stats.read().await.clone());
        }

        // Process opportunities with concurrency control
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
        let mut handles = Vec::new();

        for opportunity in opportunities {
            let semaphore = semaphore.clone();
            let storage = self.storage.clone();
            let config = self.config.clone();
            let stats = self.stats.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                Self::process_opportunity(
                    opportunity,
                    storage,
                    config,
                    stats,
                ).await
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Task failed: {}", e);
            }
        }

        let final_stats = self.stats.read().await.clone();

        info!("✅ Auto-registration completed:");
        info!("  Total attempts: {}", final_stats.total_attempts);
        info!("  Successful: {} ({:.1}%)", final_stats.successful, final_stats.success_rate());
        info!("  Failed: {}", final_stats.failed);
        info!("  CAPTCHA blocked: {}", final_stats.captcha_blocked);
        info!("  Email verification required: {}", final_stats.email_verification_required);

        Ok(final_stats)
    }

    /// Find registration opportunities from database
    async fn find_opportunities(&self) -> Result<Vec<RegistrationOpportunity>> {
        let mut opportunities = Vec::new();

        // Query registration_opportunities view
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<bool>, Option<bool>, Option<i64>, Option<chrono::DateTime<chrono::Utc>>)>(
            r#"
            SELECT
                url,
                domain,
                network,
                title,
                has_captcha,
                requires_email,
                existing_accounts,
                last_registration
            FROM registration_opportunities
            WHERE network = ANY($1)
            AND existing_accounts < $2
            ORDER BY existing_accounts ASC, last_registration DESC NULLS FIRST
            LIMIT 100
            "#,
        )
        .bind(&self.config.networks)
        .bind(self.config.max_accounts_per_domain as i64)
        .fetch_all(self.storage.pool())
        .await?;

        for row in rows {
            let (url, domain, network, title, has_captcha, requires_email, existing_accounts, _last_registration) = row;

            // Skip if CAPTCHA and configured to skip
            if self.config.skip_captcha_sites && has_captcha.unwrap_or(false) {
                continue;
            }

            // Skip if email verification required and configured to skip
            if self.config.skip_email_verification_sites && requires_email.unwrap_or(false) {
                continue;
            }

            opportunities.push(RegistrationOpportunity {
                url,
                domain,
                network,
                title,
                has_captcha: has_captcha.unwrap_or(false),
                requires_email: requires_email.unwrap_or(false),
                existing_accounts: existing_accounts.unwrap_or(0) as usize,
            });
        }

        Ok(opportunities)
    }

    /// Process a single registration opportunity
    async fn process_opportunity(
        opportunity: RegistrationOpportunity,
        storage: Storage,
        config: AutoRegisterConfig,
        stats: Arc<tokio::sync::RwLock<AutoRegisterStats>>,
    ) -> Result<()> {
        info!("📝 Attempting registration at: {} ({})", opportunity.domain, opportunity.network);

        // Generate registration data
        let data = Self::generate_registration_data(&opportunity);

        // Get proxy URL for this network
        let proxy_url = Self::get_proxy_url(&opportunity.network);

        // Create registration engine
        let engine = RegistrationEngine::new(config.registration_config.clone(), storage.clone());

        // Update stats - attempt started
        {
            let mut stats = stats.write().await;
            stats.total_attempts += 1;
            stats.sites_processed += 1;
        }

        // Attempt registration
        match engine.register(&opportunity.url, data, proxy_url.as_deref()).await {
            Ok(result) => {
                if result.is_success() {
                    info!("✓ Successfully registered at {}", opportunity.domain);

                    let mut stats = stats.write().await;
                    stats.successful += 1;

                    if result.result.requires_email_verification {
                        stats.email_verification_required += 1;
                    }
                } else {
                    warn!("✗ Registration failed at {}: {:?}", opportunity.domain, result.result.error);

                    let mut stats = stats.write().await;
                    stats.failed += 1;

                    // Check if it was a CAPTCHA block
                    if let Some(ref error) = result.result.error {
                        if error.contains("CAPTCHA") || error.contains("captcha") {
                            stats.captcha_blocked += 1;
                        }
                    }
                }
            }
            Err(e) => {
                error!("✗ Registration error at {}: {}", opportunity.domain, e);
                let mut stats = stats.write().await;
                stats.failed += 1;
            }
        }

        // Wait before next attempt to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_secs(
            config.min_registration_interval_seconds
        )).await;

        Ok(())
    }

    /// Generate random registration data
    fn generate_registration_data(opportunity: &RegistrationOpportunity) -> RegistrationData {
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
        let password = Self::generate_strong_password();

        // Generate email if required
        let email = if opportunity.requires_email {
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

    /// Generate a strong random password
    fn generate_strong_password() -> String {
        use rand::Rng;

        let mut rng = rand::thread_rng();

        let lowercase = "abcdefghijklmnopqrstuvwxyz";
        let uppercase = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let numbers = "0123456789";
        let special = "!@#$%^&*";

        let mut password = String::new();

        // Ensure at least one of each type
        password.push(lowercase.chars().nth(rng.gen_range(0..lowercase.len())).unwrap());
        password.push(uppercase.chars().nth(rng.gen_range(0..uppercase.len())).unwrap());
        password.push(numbers.chars().nth(rng.gen_range(0..numbers.len())).unwrap());
        password.push(special.chars().nth(rng.gen_range(0..special.len())).unwrap());

        // Fill rest with random characters
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

    /// Get proxy URL for network
    fn get_proxy_url(network: &str) -> Option<String> {
        match network {
            "tor" => Some("socks5://127.0.0.1:9050".to_string()),
            "i2p" => Some("http://127.0.0.1:4444".to_string()),
            "lokinet" => Some("socks5://127.0.0.1:1080".to_string()),
            "hyphanet" => Some("http://127.0.0.1:8888".to_string()),
            _ => None,
        }
    }

    /// Get current statistics
    pub async fn stats(&self) -> AutoRegisterStats {
        self.stats.read().await.clone()
    }
}

#[derive(Debug, Clone)]
pub struct RegistrationOpportunity {
    pub url: String,
    pub domain: String,
    pub network: String,
    pub title: Option<String>,
    pub has_captcha: bool,
    pub requires_email: bool,
    pub existing_accounts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_strong_password() {
        let password = AutoRegister::generate_strong_password();

        assert!(password.len() >= 16);
        assert!(password.chars().any(|c| c.is_lowercase()));
        assert!(password.chars().any(|c| c.is_uppercase()));
        assert!(password.chars().any(|c| c.is_numeric()));
        assert!(password.chars().any(|c| "!@#$%^&*".contains(c)));
    }

    #[test]
    fn test_get_proxy_url() {
        assert_eq!(
            AutoRegister::get_proxy_url("tor"),
            Some("socks5://127.0.0.1:9050".to_string())
        );
        assert_eq!(
            AutoRegister::get_proxy_url("i2p"),
            Some("http://127.0.0.1:4444".to_string())
        );
        assert_eq!(AutoRegister::get_proxy_url("clearnet"), None);
    }
}
