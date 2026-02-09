/// Cookie storage for reusing sessions across requests
/// Avoids repeated headless browser usage for sites with DDoS protection

use std::sync::Arc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCookies {
    pub domain: String,
    pub cookies: String,  // JSON-encoded cookie array
    pub user_agent: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct CookieStore {
    // domain -> cookies
    store: Arc<DashMap<String, StoredCookies>>,
}

impl CookieStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
        }
    }

    /// Store cookies for a domain
    pub fn set(&self, domain: String, cookies: String, user_agent: String) {
        let stored = StoredCookies {
            domain: domain.clone(),
            cookies,
            user_agent,
            created_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(24)),
        };

        self.store.insert(domain, stored);
    }

    /// Get cookies for a domain
    pub fn get(&self, domain: &str) -> Option<StoredCookies> {
        if let Some(entry) = self.store.get(domain) {
            let stored = entry.value().clone();

            // Check if expired
            if let Some(expires) = stored.expires_at {
                if expires < chrono::Utc::now() {
                    drop(entry);
                    self.store.remove(domain);
                    return None;
                }
            }

            return Some(stored);
        }

        None
    }

    /// Remove cookies for a domain (e.g., if they stopped working)
    pub fn remove(&self, domain: &str) {
        self.store.remove(domain);
    }

    /// Check if we have valid cookies for a domain
    pub fn has_cookies(&self, domain: &str) -> bool {
        self.get(domain).is_some()
    }
}

impl Default for CookieStore {
    fn default() -> Self {
        Self::new()
    }
}
