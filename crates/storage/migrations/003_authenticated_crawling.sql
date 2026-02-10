-- Add table for authenticated URLs to crawl
CREATE TABLE IF NOT EXISTS authenticated_urls (
    id BIGSERIAL PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    domain TEXT NOT NULL,
    network VARCHAR(20) NOT NULL,
    session_cookies TEXT,
    account_id BIGINT REFERENCES registered_accounts(id),
    priority INT DEFAULT 100,
    crawl_status VARCHAR(20) DEFAULT 'pending',
    last_checked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auth_urls_domain ON authenticated_urls(domain);
CREATE INDEX IF NOT EXISTS idx_auth_urls_status ON authenticated_urls(crawl_status);

-- Add content analysis columns
ALTER TABLE registered_accounts ADD COLUMN IF NOT EXISTS urls_before_registration INT DEFAULT 0;
ALTER TABLE registered_accounts ADD COLUMN IF NOT EXISTS urls_after_registration INT DEFAULT 0;
ALTER TABLE registered_accounts ADD COLUMN IF NOT EXISTS content_unlocked_percent FLOAT DEFAULT 0;
ALTER TABLE registered_accounts ADD COLUMN IF NOT EXISTS authenticated_urls_discovered INT DEFAULT 0;
ALTER TABLE registered_accounts ADD COLUMN IF NOT EXISTS last_crawled_at TIMESTAMPTZ;
