/// Account status update methods
use anyhow::Result;
use sqlx::PgPool;

/// Mark email as verified
pub async fn update_email_verified(pool: &PgPool, domain: &str, username: &str) -> Result<()> {
    sqlx::query(
        "UPDATE registered_accounts SET email_verified = true WHERE site_domain = $1 AND username = $2"
    )
    .bind(domain)
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update last login timestamp
pub async fn update_last_login(pool: &PgPool, domain: &str, username: &str) -> Result<()> {
    sqlx::query(
        "UPDATE registered_accounts SET last_login_at = NOW() WHERE site_domain = $1 AND username = $2"
    )
    .bind(domain)
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update account status
pub async fn update_account_status(pool: &PgPool, domain: &str, username: &str, status: &str) -> Result<()> {
    sqlx::query(
        "UPDATE registered_accounts SET status = $3 WHERE site_domain = $1 AND username = $2"
    )
    .bind(domain)
    .bind(username)
    .bind(status)
    .execute(pool)
    .await?;

    Ok(())
}
