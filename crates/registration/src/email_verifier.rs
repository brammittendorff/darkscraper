/// Email verification - checks inbox and clicks verification links
use anyhow::Result;
use tracing::{info, warn};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct EmailVerificationResult {
    pub verified: bool,
    pub verification_link: Option<String>,
    pub error: Option<String>,
}

pub struct EmailVerifier {
    email_domain: String,
}

impl EmailVerifier {
    pub fn new(email_domain: String) -> Self {
        Self { email_domain }
    }

    /// Check inbox and verify email (polls for up to 5 minutes)
    pub async fn verify_email(
        &self,
        email_address: &str,
        expected_from_domain: &str,
    ) -> Result<EmailVerificationResult> {
        info!("📧 Checking inbox for verification email: {}", email_address);

        // Extract username from email
        let username = email_address.split('@').next().unwrap_or("");

        // Poll inbox for up to 5 minutes
        let max_attempts = 30; // 30 attempts * 10 seconds = 5 minutes
        let mut attempt = 0;

        while attempt < max_attempts {
            attempt += 1;

            info!("Checking inbox (attempt {}/{})", attempt, max_attempts);

            // Check inbox using secmail.pro API
            match self.check_secmail_inbox(username).await {
                Ok(emails) => {
                    info!("Found {} emails in inbox", emails.len());

                    // Look for verification email
                    for email in emails {
                        info!("Email from: {} - Subject: {}", email.from, email.subject);

                        // Check if this is from the expected domain
                        if email.from.contains(expected_from_domain) ||
                           email.subject.to_lowercase().contains("verif") ||
                           email.subject.to_lowercase().contains("confirm") ||
                           email.subject.to_lowercase().contains("activ") {

                            // Extract verification link
                            if let Some(link) = Self::extract_verification_link(&email.body_text, expected_from_domain) {
                                info!("✓ Found verification link: {}", link);

                                return Ok(EmailVerificationResult {
                                    verified: true,
                                    verification_link: Some(link),
                                    error: None,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Error checking inbox: {}", e);
                }
            }

            // Wait before next check
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }

        warn!("✗ No verification email received after {} attempts", max_attempts);

        Ok(EmailVerificationResult {
            verified: false,
            verification_link: None,
            error: Some("No verification email received".to_string()),
        })
    }

    /// Check secmail.pro inbox via API
    async fn check_secmail_inbox(&self, username: &str) -> Result<Vec<EmailMessage>> {
        // secmail.pro API endpoint
        let api_url = format!("https://www.1secmail.com/api/v1/?action=getMessages&login={}&domain=secmail.pro", username);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client.get(&api_url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to check inbox: HTTP {}", response.status());
        }

        let emails: Vec<SecmailMessage> = response.json().await?;

        // Fetch full email bodies
        let mut full_emails = Vec::new();

        for msg in emails {
            if let Ok(full_msg) = self.fetch_email_body(username, msg.id).await {
                full_emails.push(EmailMessage {
                    from: full_msg.from,
                    subject: full_msg.subject,
                    body_text: full_msg.body.unwrap_or_default(),
                    body_html: full_msg.htmlBody,
                });
            }
        }

        Ok(full_emails)
    }

    /// Fetch full email body
    async fn fetch_email_body(&self, username: &str, email_id: i64) -> Result<SecmailFullMessage> {
        let api_url = format!(
            "https://www.1secmail.com/api/v1/?action=readMessage&login={}&domain=secmail.pro&id={}",
            username, email_id
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client.get(&api_url).send().await?;
        let msg: SecmailFullMessage = response.json().await?;

        Ok(msg)
    }

    /// Extract verification link from email body
    fn extract_verification_link(email_body: &str, domain: &str) -> Option<String> {
        use regex::Regex;

        // Common verification link patterns
        let patterns = vec![
            // Direct links
            format!(r"https?://{}/[^\s<>]*(?:verify|confirm|activate)[^\s<>]*", regex::escape(domain)),
            format!(r"https?://[^\s<>]*{}[^\s<>]*(?:verify|confirm|activate)[^\s<>]*", regex::escape(domain)),
            // With tokens
            r"https?://[^\s<>]*[?&](?:token|code|key)=[a-zA-Z0-9_-]+[^\s<>]*".to_string(),
            // With hash
            r"https?://[^\s<>]*#[a-zA-Z0-9_-]+".to_string(),
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(&pattern) {
                if let Some(captures) = re.captures(email_body) {
                    if let Some(link) = captures.get(0) {
                        let link_str = link.as_str().to_string();
                        // Clean up common trailing characters
                        let cleaned = link_str.trim_end_matches(&[')', ']', '.', ',', ';'][..]);
                        return Some(cleaned.to_string());
                    }
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub from: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SecmailMessage {
    id: i64,
    from: String,
    subject: String,
    date: String,
}

#[derive(Debug, Deserialize)]
struct SecmailFullMessage {
    from: String,
    subject: String,
    date: String,
    body: Option<String>,
    htmlBody: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_verification_link() {
        let body = "Please verify your email by clicking: http://example.onion/verify?token=abc123";
        let link = EmailVerifier::extract_verification_link(body, "example.onion");
        assert!(link.is_some());
        assert!(link.unwrap().contains("verify"));
    }
}
