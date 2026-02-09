use anyhow::Result;
use rand::Rng;
use tracing::info;

/// Generate a random temporary email address
pub fn generate_temp_email(domain: &str) -> String {
    let username = generate_random_username();
    format!("{}@{}", username, domain)
}

/// Generate a random username (8-12 chars, alphanumeric)
pub fn generate_random_username() -> String {
    let mut rng = rand::thread_rng();
    let length = rng.gen_range(8..=12);

    let charset: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx]
        })
        .collect()
}

/// Generate a strong random password
pub fn generate_random_password() -> String {
    use sha2::{Digest, Sha256};

    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();

    let mut hasher = Sha256::new();
    hasher.update(&random_bytes);
    let hash = hasher.finalize();

    // Take first 16 chars of hex + add special chars for complexity
    format!("{}!@#", hex::encode(&hash[..8]))
}

/// Temporary email service client (can be extended to use external services)
pub struct TempEmailService {
    domain: String,
}

impl TempEmailService {
    pub fn new(domain: String) -> Self {
        Self { domain }
    }

    /// Create a new temporary email
    pub fn create_email(&self) -> String {
        generate_temp_email(&self.domain)
    }

    /// Check inbox for verification emails (mock implementation)
    /// In practice, this would connect to an SMTP server or API
    pub async fn check_inbox(&self, email: &str) -> Result<Vec<ReceivedEmail>> {
        info!("checking inbox for {}", email);
        // TODO: Implement actual email checking via IMAP or custom service
        Ok(vec![])
    }

    /// Extract verification link from email body
    pub fn extract_verification_link(email_body: &str, domain: &str) -> Option<String> {
        use regex::Regex;

        // Common verification link patterns
        let patterns = vec![
            format!(r"https?://{}/verify[^\s<>]*", regex::escape(domain)),
            format!(r"https?://{}/activate[^\s<>]*", regex::escape(domain)),
            format!(r"https?://{}/confirm[^\s<>]*", regex::escape(domain)),
            r"https?://[^\s<>]*[?&]token=[a-zA-Z0-9]+".to_string(),
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(&pattern) {
                if let Some(captures) = re.captures(email_body) {
                    if let Some(link) = captures.get(0) {
                        return Some(link.as_str().to_string());
                    }
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct ReceivedEmail {
    pub from: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub received_at: chrono::DateTime<chrono::Utc>,
}
