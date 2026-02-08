use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub page_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub network: String,
    pub domain: String,
    pub snippet: Option<String>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct EntityResult {
    pub entity_id: i64,
    pub page_id: i64,
    pub entity_type: String,
    pub value: String,
    pub page_url: String,
    pub page_title: Option<String>,
}

pub struct SearchEngine {
    pool: PgPool,
}

impl SearchEngine {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Search pages by body text, raw HTML, and title (ILIKE with trigram index)
    pub async fn search_text(&self, query: &str, limit: i64) -> Result<Vec<SearchResult>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query_as::<
            _,
            (
                i64,
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            r#"SELECT id, url, title, network, domain, LEFT(body_text, 200), fetched_at
               FROM pages
               WHERE body_text ILIKE $1 OR title ILIKE $1 OR raw_html ILIKE $1
               ORDER BY fetched_at DESC
               LIMIT $2"#,
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SearchResult {
                page_id: r.0,
                url: r.1,
                title: r.2,
                network: r.3,
                domain: r.4,
                snippet: r.5,
                fetched_at: r.6,
            })
            .collect())
    }

    /// Search for entities by type and value
    pub async fn search_entity(
        &self,
        entity_type: Option<&str>,
        value: &str,
        limit: i64,
    ) -> Result<Vec<EntityResult>> {
        let pattern = format!("%{}%", value);

        let rows = if let Some(et) = entity_type {
            sqlx::query_as::<_, (i64, i64, String, String, String, Option<String>)>(
                r#"SELECT e.id, e.page_id, e.entity_type, e.value, p.url, p.title
                   FROM entities e
                   JOIN pages p ON p.id = e.page_id
                   WHERE e.entity_type = $1 AND e.value ILIKE $2
                   ORDER BY e.found_at DESC
                   LIMIT $3"#,
            )
            .bind(et)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, i64, String, String, String, Option<String>)>(
                r#"SELECT e.id, e.page_id, e.entity_type, e.value, p.url, p.title
                   FROM entities e
                   JOIN pages p ON p.id = e.page_id
                   WHERE e.value ILIKE $1
                   ORDER BY e.found_at DESC
                   LIMIT $2"#,
            )
            .bind(&pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| EntityResult {
                entity_id: r.0,
                page_id: r.1,
                entity_type: r.2,
                value: r.3,
                page_url: r.4,
                page_title: r.5,
            })
            .collect())
    }
}
