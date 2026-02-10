/// Email provider prioritization - register on email services first
use std::collections::HashMap;

/// Email service domains that don't require email to register
pub fn get_email_provider_domains() -> Vec<&'static str> {
    vec![
        // DNMX - Dark Net Mail Exchange
        "dnmxjaitaiafwmss2lx7tbs5bv66l7vjdmb5mtb3yqpxqhk3it5zivad.onion", // DNMX
        "hxuzjtocnzvv5g2rtg2bhwkcbupmk7rclb6lly3fo4tvqkk5oyrv3nid.onion", // DNMX mirror

        // cock.li - Privacy email service
        "rurcblzhmdk22kttfkel2zduhyu3r6to7knyc7wiorzrx5gw4c3lftad.onion", // cock.li

        // Other email services
        "ed4ivea276i66ne6ca67h22ozp6yjsg553wpeozh34b5qqvnjh7yuiyd.onion", // MailBuddy
        "mailcityi5k7ccf2cepjnqrroxtjrx2veaeiafda5dclognvutz22xid.onion", // MailCity
    ]
}

/// Check if domain is an email provider
pub fn is_email_provider(domain: &str) -> bool {
    get_email_provider_domains().contains(&domain)
}

/// Get priority score - higher = should register first
pub fn get_registration_priority(domain: &str, requires_email: bool) -> i32 {
    // Priority levels:
    // 1000+ = Email providers (register FIRST to get emails)
    // 500-999 = Sites without email requirement
    // 1-499 = Sites requiring email (register AFTER we have emails)

    if is_email_provider(domain) {
        1000  // Highest priority - register on email services first
    } else if !requires_email {
        700  // Medium-high - sites that don't need email
    } else {
        300  // Lower - sites needing email (wait until we have working emails)
    }
}

/// Get available email accounts from our registered email providers
pub async fn get_available_dnmx_emails(
    storage: &darkscraper_storage::Storage
) -> anyhow::Result<Vec<String>> {
    let accounts = sqlx::query_as::<_, (String, String,)>(
        r#"
        SELECT username, site_domain
        FROM registered_accounts
        WHERE (site_domain LIKE '%dnmx%' OR site_domain LIKE '%cock.li%')
          AND status IN ('active', 'verified')
          AND last_login_at IS NOT NULL
        ORDER BY registered_at DESC
        LIMIT 10
        "#
    )
    .fetch_all(storage.pool())
    .await?;

    // Convert to email addresses
    let emails: Vec<String> = accounts.into_iter().map(|(username, domain)| {
        if domain.contains("dnmx") {
            format!("{}@dnmx.cc", username)
        } else if domain.contains("cock.li") {
            format!("{}@cock.li", username)
        } else {
            format!("{}@secmail.pro", username)  // Fallback
        }
    }).collect();

    Ok(emails)
}
