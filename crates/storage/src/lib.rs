use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

use darkscraper_core::PageData;

pub struct Storage {
    pool: PgPool,
}

impl Storage {
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::with_pool_size(database_url, 10).await
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
        info!("migrations complete");
        Ok(())
    }

    pub async fn store_page(&self, page: &PageData) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"INSERT INTO pages (url, final_url, network, domain, title, body_text, raw_html, raw_html_hash, status_code, content_type, server_header, language, has_login_form, response_time_ms, fetched_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
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
            let mut all_freenet: Vec<bool> = Vec::new();
            let mut all_lokinet: Vec<bool> = Vec::new();
            for link in &page.links {
                all_targets.push(link.url.clone());
                all_anchors.push(link.anchor_text.clone().unwrap_or_default());
                all_onion.push(link.is_onion);
                all_i2p.push(link.is_i2p);
                all_zeronet.push(link.is_zeronet);
                all_freenet.push(link.is_freenet);
                all_lokinet.push(link.is_lokinet);
            }
            sqlx::query(
                "INSERT INTO links (source_page_id, target_url, anchor_text, is_onion, is_i2p, is_zeronet, is_freenet, is_lokinet)
                 SELECT $1, * FROM UNNEST($2::text[], $3::text[], $4::bool[], $5::bool[], $6::bool[], $7::bool[], $8::bool[])"
            )
            .bind(page_id)
            .bind(&all_targets)
            .bind(&all_anchors)
            .bind(&all_onion)
            .bind(&all_i2p)
            .bind(&all_zeronet)
            .bind(&all_freenet)
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
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO dead_urls (url, network, domain, retry_count, last_error) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (url) DO NOTHING",
        )
        .bind(url)
        .bind(network)
        .bind(domain)
        .bind(retry_count as i32)
        .bind(last_error)
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

    /// Clear dead URLs for a specific network (e.g. after fixing a broken proxy).
    pub async fn clear_dead_urls_for_network(&self, network: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM dead_urls WHERE network = $1")
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

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
