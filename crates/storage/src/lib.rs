use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

use darkscraper_core::PageData;

mod registration_queries;

#[derive(Clone)]
pub struct Storage {
    pool: PgPool,
}

impl Storage {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Pool size: 200 to handle high worker counts (SCALE_LEVEL=5 = 186 workers + storage task)
        // Each worker needs connection for writes, plus overhead
        Self::with_pool_size(database_url, 200).await
    }

    pub async fn with_pool_size(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        info!(max_connections, "connected to postgres");
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        // Run embedded migrations
        sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
            .execute(&self.pool)
            .await?;
        sqlx::raw_sql(include_str!("../migrations/002_registration.sql"))
            .execute(&self.pool)
            .await?;
        info!("migrations complete");
        Ok(())
    }

    pub async fn store_page(&self, page: &PageData) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"INSERT INTO pages (url, final_url, network, domain, title, body_text, raw_html, raw_html_hash, status_code, content_type, server_header, language, has_login_form, has_register_form, has_captcha, requires_email, is_forum, response_time_ms, fetched_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
               ON CONFLICT (url, fetched_at) DO UPDATE SET body_text = EXCLUDED.body_text, raw_html = EXCLUDED.raw_html
               RETURNING id"#,
        )
        .bind(&page.url)
        .bind(&page.final_url)
        .bind(&page.network)
        .bind(&page.domain)
        .bind(&page.title)
        .bind(&page.body_text)
        .bind(&page.raw_html)
        .bind(&page.raw_html_hash)
        .bind(page.status_code as i32)
        .bind(&page.content_type)
        .bind(&page.metadata.server_header)
        .bind(&page.metadata.language)
        .bind(page.metadata.has_login_form)
        .bind(page.metadata.has_register_form)
        .bind(page.metadata.has_captcha)
        .bind(page.metadata.requires_email)
        .bind(page.metadata.is_forum)
        .bind(page.response_time_ms as i32)
        .bind(page.fetched_at)
        .fetch_one(&self.pool)
        .await?;

        let page_id = row.0;

        // Store headings (batch UNNEST insert)
        let mut all_levels: Vec<i32> = Vec::new();
        let mut all_texts: Vec<String> = Vec::new();
        for h in &page.h1 {
            all_levels.push(1);
            all_texts.push(h.clone());
        }
        for h in &page.h2 {
            all_levels.push(2);
            all_texts.push(h.clone());
        }
        for h in &page.h3 {
            all_levels.push(3);
            all_texts.push(h.clone());
        }
        if !all_levels.is_empty() {
            sqlx::query(
                "INSERT INTO headings (page_id, level, text)
                 SELECT $1, * FROM UNNEST($2::int[], $3::text[])",
            )
            .bind(page_id)
            .bind(&all_levels)
            .bind(&all_texts)
            .execute(&self.pool)
            .await?;
        }

        // Store entities (batch UNNEST insert)
        {
            let now = chrono::Utc::now();
            let entity_groups: Vec<(&str, &[String])> = vec![
                ("email", &page.entities.emails),
                ("onion_address", &page.entities.onion_addresses),
                ("i2p_address", &page.entities.i2p_addresses),
                ("bitcoin", &page.entities.bitcoin_addresses),
                ("monero", &page.entities.monero_addresses),
                ("ethereum", &page.entities.ethereum_addresses),
                ("phone", &page.entities.phone_numbers),
                ("pgp_fingerprint", &page.entities.pgp_fingerprints),
                ("username", &page.entities.usernames),
            ];

            let mut all_types: Vec<String> = Vec::new();
            let mut all_values: Vec<String> = Vec::new();
            let mut all_times: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
            for (entity_type, values) in entity_groups {
                for value in values.iter() {
                    all_types.push(entity_type.to_string());
                    all_values.push(value.clone());
                    all_times.push(now);
                }
            }
            if !all_types.is_empty() {
                sqlx::query(
                    "INSERT INTO entities (page_id, entity_type, value, found_at)
                     SELECT $1, * FROM UNNEST($2::varchar[], $3::text[], $4::timestamptz[])",
                )
                .bind(page_id)
                .bind(&all_types)
                .bind(&all_values)
                .bind(&all_times)
                .execute(&self.pool)
                .await?;
            }
        }

        // Store links (batch UNNEST insert)
        if !page.links.is_empty() {
            let mut all_targets: Vec<String> = Vec::new();
            let mut all_anchors: Vec<String> = Vec::new();
            let mut all_onion: Vec<bool> = Vec::new();
            let mut all_i2p: Vec<bool> = Vec::new();
            let mut all_zeronet: Vec<bool> = Vec::new();
            let mut all_hyphanet: Vec<bool> = Vec::new();
            let mut all_lokinet: Vec<bool> = Vec::new();
            for link in &page.links {
                all_targets.push(link.url.clone());
                all_anchors.push(link.anchor_text.clone().unwrap_or_default());
                all_onion.push(link.is_onion);
                all_i2p.push(link.is_i2p);
                all_zeronet.push(link.is_zeronet);
                all_hyphanet.push(link.is_hyphanet);
                all_lokinet.push(link.is_lokinet);
            }
            sqlx::query(
                "INSERT INTO links (source_page_id, target_url, anchor_text, is_onion, is_i2p, is_zeronet, is_hyphanet, is_lokinet)
                 SELECT $1, * FROM UNNEST($2::text[], $3::text[], $4::bool[], $5::bool[], $6::bool[], $7::bool[], $8::bool[])"
            )
            .bind(page_id)
            .bind(&all_targets)
            .bind(&all_anchors)
            .bind(&all_onion)
            .bind(&all_i2p)
            .bind(&all_zeronet)
            .bind(&all_hyphanet)
            .bind(&all_lokinet)
            .execute(&self.pool)
            .await?;
        }

        Ok(page_id)
    }

    pub async fn get_page_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pages")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn get_entity_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn check_connectivity(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn store_correlation(
        &self,
        domain: &str,
        correlation_type: &str,
        value: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO correlations (domain, correlation_type, value) VALUES ($1, $2, $3) ON CONFLICT (domain, correlation_type, value) DO NOTHING",
        )
        .bind(domain)
        .bind(correlation_type)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_correlation_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM correlations")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn get_link_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM links")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Mark a URL as permanently dead (unreachable after max retries).
    pub async fn mark_dead(
        &self,
        url: &str,
        network: &str,
        domain: &str,
        retry_count: u32,
        last_error: &str,
        failure_type: &str, // "dead" or "unreachable"
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO dead_urls (url, network, domain, retry_count, last_error, failure_type, last_attempt_at)
             VALUES ($1, $2, $3, $4, $5, $6, NOW())
             ON CONFLICT (url) DO UPDATE SET
                retry_count = $4,
                last_error = $5,
                failure_type = $6,
                last_attempt_at = NOW()",
        )
        .bind(url)
        .bind(network)
        .bind(domain)
        .bind(retry_count as i32)
        .bind(last_error)
        .bind(failure_type)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Check if a URL is dead.
    pub async fn is_dead(&self, url: &str) -> Result<bool> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM dead_urls WHERE url = $1")
            .bind(url)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 > 0)
    }

    /// Load all dead URLs into a set (for startup).
    pub async fn load_dead_urls(&self) -> Result<std::collections::HashSet<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT url FROM dead_urls")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(url,)| url).collect())
    }

    /// Clear unreachable URLs for a specific network (for retry as network improves).
    /// Only clears "unreachable" failures, not truly "dead" URLs.
    pub async fn clear_dead_urls_for_network(&self, network: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM dead_urls WHERE network = $1 AND failure_type = 'unreachable'"
        )
            .bind(network)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Count dead URLs.
    pub async fn get_dead_url_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM dead_urls")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Load all previously crawled URLs from pages + dead_urls tables.
    /// Used at startup to prime the bloom filter so we don't re-crawl.
    pub async fn load_all_known_urls(&self) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT url FROM pages UNION SELECT url FROM dead_urls")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(url,)| url).collect())
    }

    /// Get network health stats (dead vs unreachable counts per network)
    pub async fn get_network_health_stats(&self) -> Result<Vec<(String, i64, i64, i64)>> {
        let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT
                network,
                COUNT(*) FILTER (WHERE failure_type = 'dead') as dead_count,
                COUNT(*) FILTER (WHERE failure_type = 'unreachable') as unreachable_count,
                COUNT(*) as total_failed
             FROM dead_urls
             GROUP BY network
             ORDER BY network"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Count existing accounts for a domain
    pub async fn count_accounts_for_domain(&self, domain: &str) -> Result<i64> {
        registration_queries::count_accounts_for_domain(&self.pool, domain).await
    }

    /// Count recent failed registration attempts
    pub async fn count_recent_failed_attempts(&self, domain: &str, seconds: i64) -> Result<i64> {
        registration_queries::count_recent_failed_attempts(&self.pool, domain, seconds).await
    }

    // --- Registration Account Management ---

    /// Store a newly registered account
    pub async fn store_registered_account(
        &self,
        site_url: &str,
        site_domain: &str,
        network: &str,
        username: &str,
        password: &str,
        email: Option<&str>,
        email_provider: Option<&str>,
        registration_url: &str,
        session_cookies: Option<&str>,
        user_agent: &str,
        registration_notes: Option<&str>,
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO registered_accounts (site_url, site_domain, network, username, password, email, email_provider, registration_url, session_cookies, user_agent, registration_notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (site_domain, username) DO UPDATE SET
                password = EXCLUDED.password,
                session_cookies = EXCLUDED.session_cookies,
                last_login_at = NOW()
             RETURNING id"
        )
        .bind(site_url)
        .bind(site_domain)
        .bind(network)
        .bind(username)
        .bind(password)
        .bind(email)
        .bind(email_provider)
        .bind(registration_url)
        .bind(session_cookies)
        .bind(user_agent)
        .bind(registration_notes)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Get registration opportunities (sites with register forms, no existing accounts)
    pub async fn get_registration_opportunities(&self, limit: i64) -> Result<Vec<(String, String, String, bool, bool)>> {
        let rows: Vec<(String, String, String, bool, bool)> = sqlx::query_as(
            "SELECT url, domain, network, has_captcha, requires_email
             FROM pages
             WHERE has_register_form = true
               AND domain NOT IN (SELECT DISTINCT site_domain FROM registered_accounts)
             ORDER BY fetched_at DESC
             LIMIT $1"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Record a registration attempt
    pub async fn record_registration_attempt(
        &self,
        site_url: &str,
        site_domain: &str,
        network: &str,
        username: Option<&str>,
        email: Option<&str>,
        captcha_required: bool,
        captcha_solved: bool,
        email_verification_required: bool,
        email_verified: bool,
        success: bool,
        error_message: Option<&str>,
        account_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO registration_attempts (site_url, site_domain, network, username, email, captcha_required, captcha_solved, email_verification_required, email_verified, success, error_message, account_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
        )
        .bind(site_url)
        .bind(site_domain)
        .bind(network)
        .bind(username)
        .bind(email)
        .bind(captcha_required)
        .bind(captcha_solved)
        .bind(email_verification_required)
        .bind(email_verified)
        .bind(success)
        .bind(error_message)
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Create a temporary email address
    pub async fn create_temp_email(
        &self,
        email_address: &str,
        domain: &str,
        password: Option<&str>,
        inbox_url: Option<&str>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO temp_emails (email_address, domain, password, inbox_url, expires_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (email_address) DO UPDATE SET status = 'active'
             RETURNING id"
        )
        .bind(email_address)
        .bind(domain)
        .bind(password)
        .bind(inbox_url)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Get account count by network
    pub async fn get_account_count_by_network(&self) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT network, COUNT(*) FROM registered_accounts WHERE status = 'active' GROUP BY network"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
