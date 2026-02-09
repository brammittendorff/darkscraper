use anyhow::Result;
use darkscraper_registration::{AutoRegister, AutoRegisterConfig, RegistrationConfig};
use darkscraper_storage::Storage;
use tracing::info;

/// Auto-register command - automatically register accounts on discovered sites
pub async fn run(
    storage: Storage,
    max_concurrent: Option<usize>,
    max_per_domain: Option<usize>,
    networks: Option<Vec<String>>,
    enable_captcha: bool,
    skip_email_verification: bool,
) -> Result<()> {
    info!("Starting auto-registration system");

    // Build configuration
    let mut config = AutoRegisterConfig::default();

    if let Some(concurrent) = max_concurrent {
        config.max_concurrent = concurrent;
    }

    if let Some(per_domain) = max_per_domain {
        config.max_accounts_per_domain = per_domain;
    }

    if let Some(nets) = networks {
        config.networks = nets;
    }

    config.skip_captcha_sites = !enable_captcha;
    config.skip_email_verification_sites = skip_email_verification;

    // Configure registration engine
    config.registration_config = RegistrationConfig {
        use_headless_browser: true,
        browser_timeout_seconds: 120,
        wait_for_content_seconds: 30,
        captcha_service: if enable_captcha {
            Some(darkscraper_registration::CaptchaService::Free)
        } else {
            None
        },
        temp_email_domain: "secmail.pro".to_string(),
        user_agent: "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0".to_string(),
    };

    // Create and run auto-register worker
    let auto_register = AutoRegister::new(config, storage);
    let stats = auto_register.run().await?;

    // Print summary
    println!("\n╔══════════════════════════════════════════════╗");
    println!("║      Auto-Registration Summary               ║");
    println!("╠══════════════════════════════════════════════╣");
    println!("║ Total Attempts:       {:>20}    ║", stats.total_attempts);
    println!("║ Successful:           {:>20}    ║", stats.successful);
    println!("║ Failed:               {:>20}    ║", stats.failed);
    println!("║ CAPTCHA Blocked:      {:>20}    ║", stats.captcha_blocked);
    println!("║ Email Verification:   {:>20}    ║", stats.email_verification_required);
    println!("║ Sites Processed:      {:>20}    ║", stats.sites_processed);
    println!("║ Success Rate:         {:>19.1}%   ║", stats.success_rate());
    println!("╚══════════════════════════════════════════════╝\n");

    Ok(())
}
