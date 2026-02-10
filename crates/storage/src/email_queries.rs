/// Email-related queries
use anyhow::Result;
use sqlx::PgPool;

/// Count email provider accounts (DNMX, cock.li, etc.)
pub async fn count_dnmx_accounts(pool: &PgPool) -> Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM registered_accounts
        WHERE (site_domain LIKE '%dnmx%' OR site_domain LIKE '%cock.li%')
          AND status IN ('active', 'verified')
        "#
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}
