/// Helper functions for verification flow
use anyhow::Result;
use tracing::info;
use crate::browser::HeadlessBrowser;
use crate::RegistrationConfig;

/// Click a verification link and wait for confirmation
pub async fn click_verification_link(
    verification_link: &str,
    proxy_url: Option<&str>,
    config: &RegistrationConfig,
) -> Result<bool> {
    info!("Clicking verification link: {}", verification_link);

    // Create browser
    let browser = HeadlessBrowser::new(config.clone(), proxy_url)?;

    // Navigate to verification link
    let tab = browser.navigate_and_wait(verification_link).await?;

    // Get final HTML
    let html = browser.get_rendered_html(&tab)?;
    let html_lower = html.to_lowercase();

    // Check for success indicators
    let success = html_lower.contains("verified") ||
                 html_lower.contains("confirmed") ||
                 html_lower.contains("activated") ||
                 html_lower.contains("success") ||
                 html_lower.contains("thank you");

    // Take screenshot
    browser.screenshot(&tab, "/tmp/email_verification.png").ok();

    if success {
        info!("✓ Email verification confirmed");
        Ok(true)
    } else {
        info!("⚠️  Verification link clicked but success unclear");
        Ok(false)
    }
}
