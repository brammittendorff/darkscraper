use anyhow::Result;
use rand::Rng;
use tracing::info;

/// Generate a random temporary email address
pub fn generate_temp_email(domain: &str) -> String {
    let username = generate_random_username();
    format!("{}@{}", username, domain)
}

/// Generate a random username from English words (like happy_cloud, blue_tiger)
pub fn generate_random_username() -> String {
    use rand::seq::SliceRandom;

    let mut rng = rand::thread_rng();

    // Adjectives
    let adjectives = [
        "happy", "blue", "fast", "bright", "cool", "dark", "brave", "calm", "wise", "bold",
        "quick", "silent", "strong", "gentle", "wild", "free", "clever", "friendly", "honest",
        "kind", "lucky", "mighty", "noble", "proud", "royal", "safe", "swift", "true", "warm",
        "active", "ancient", "cosmic", "digital", "electric", "golden", "hidden", "instant",
        "magic", "mystic", "neon", "omega", "phantom", "quantum", "rapid", "secret", "silver",
        "sonic", "stellar", "super", "turbo", "ultra", "virtual", "zero",
    ];

    // Nouns
    let nouns = [
        "cloud", "tiger", "river", "mountain", "ocean", "forest", "desert", "island", "valley",
        "dragon", "eagle", "wolf", "bear", "lion", "hawk", "fox", "deer", "whale", "dolphin",
        "knight", "warrior", "wizard", "hunter", "guardian", "shadow", "phantom", "spirit",
        "storm", "thunder", "lightning", "fire", "wind", "earth", "star", "moon", "sun",
        "crystal", "diamond", "emerald", "ruby", "sapphire", "jade", "pearl", "amber",
        "code", "byte", "pixel", "cyber", "matrix", "nexus", "vortex", "zenith", "apex",
        "arrow", "blade", "crown", "dream", "flame", "ghost", "heart", "key", "light",
    ];

    let adj = adjectives.choose(&mut rng).unwrap();
    let noun = nouns.choose(&mut rng).unwrap();

    // Random separator and optional number
    let separator = if rng.gen_bool(0.7) { "_" } else { "" };
    let add_number = rng.gen_bool(0.3);

    if add_number {
        let num = rng.gen_range(1..9999);
        format!("{}{}{}{}", adj, separator, noun, num)
    } else {
        format!("{}{}{}", adj, separator, noun)
    }
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
