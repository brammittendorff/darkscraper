use anyhow::Result;
use sqlx::PgPool;

/// Count existing accounts for a domain
pub async fn count_accounts_for_domain(pool: &PgPool, domain: &str) -> Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM registered_accounts WHERE site_domain = $1"
    )
    .bind(domain)
    .fetch_one(pool)
    .await?;

    Ok(count)
}

/// Count recent failed registration attempts (within last N seconds)
pub async fn count_recent_failed_attempts(pool: &PgPool, domain: &str, seconds: i64) -> Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM registration_attempts
        WHERE site_domain = $1
        AND success = false
        AND attempted_at > NOW() - INTERVAL '1 second' * $2
        "#
    )
    .bind(domain)
    .bind(seconds)
    .fetch_one(pool)
    .await?;

    Ok(count)
}
