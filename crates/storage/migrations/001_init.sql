CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE IF NOT EXISTS pages (
    id BIGSERIAL PRIMARY KEY,
    url TEXT NOT NULL,
    final_url TEXT NOT NULL,
    network VARCHAR(20) NOT NULL,
    domain TEXT NOT NULL,
    title TEXT,
    body_text TEXT,
    raw_html TEXT,
    raw_html_hash VARCHAR(64) NOT NULL,
    status_code INT,
    content_type TEXT,
    server_header TEXT,
    language VARCHAR(10),
    has_login_form BOOLEAN DEFAULT FALSE,
    adverse_media_score FLOAT DEFAULT 0.0,
    adverse_categories TEXT[],
    response_time_ms INT,
    fetched_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(url, fetched_at)
);

CREATE INDEX IF NOT EXISTS idx_pages_domain ON pages(domain);
CREATE INDEX IF NOT EXISTS idx_pages_network ON pages(network);
CREATE INDEX IF NOT EXISTS idx_pages_adverse ON pages(adverse_media_score DESC);

-- Full-text search index on body_text and raw_html
CREATE INDEX IF NOT EXISTS idx_pages_body_trgm ON pages USING gin(body_text gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_pages_title_trgm ON pages USING gin(title gin_trgm_ops);

CREATE TABLE IF NOT EXISTS headings (
    id BIGSERIAL PRIMARY KEY,
    page_id BIGINT REFERENCES pages(id),
    level INT NOT NULL,
    text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS entities (
    id BIGSERIAL PRIMARY KEY,
    page_id BIGINT REFERENCES pages(id),
    entity_type VARCHAR(30) NOT NULL,
    value TEXT NOT NULL,
    context TEXT,
    found_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entities_type_value ON entities(entity_type, value);
CREATE INDEX IF NOT EXISTS idx_entities_value_trgm ON entities USING gin(value gin_trgm_ops);

CREATE TABLE IF NOT EXISTS links (
    id BIGSERIAL PRIMARY KEY,
    source_page_id BIGINT REFERENCES pages(id),
    target_url TEXT NOT NULL,
    anchor_text TEXT,
    is_onion BOOLEAN,
    is_i2p BOOLEAN,
    is_zeronet BOOLEAN,
    is_freenet BOOLEAN,
    is_lokinet BOOLEAN
);

-- Add columns for existing databases that don't have them yet
ALTER TABLE links ADD COLUMN IF NOT EXISTS is_freenet BOOLEAN;
ALTER TABLE links ADD COLUMN IF NOT EXISTS is_lokinet BOOLEAN;

CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_url);

CREATE TABLE IF NOT EXISTS correlations (
    id BIGSERIAL PRIMARY KEY,
    domain TEXT NOT NULL,
    correlation_type VARCHAR(50) NOT NULL,
    value TEXT NOT NULL,
    found_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_corr_unique ON correlations(domain, correlation_type, value);
CREATE INDEX IF NOT EXISTS idx_corr_type_value ON correlations(correlation_type, value);
CREATE INDEX IF NOT EXISTS idx_corr_domain ON correlations(domain);

-- Add columns if missing (for existing databases)
DO $$ BEGIN
    ALTER TABLE links ADD COLUMN is_zeronet BOOLEAN;
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE pages ADD COLUMN raw_html TEXT;
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS dead_urls (
    id BIGSERIAL PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    network VARCHAR(20),
    domain TEXT,
    reason TEXT,
    retry_count INT DEFAULT 0,
    last_error TEXT,
    died_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_dead_urls_url ON dead_urls(url);
CREATE INDEX IF NOT EXISTS idx_dead_urls_domain ON dead_urls(domain);

CREATE TABLE IF NOT EXISTS crawl_queue (
    id BIGSERIAL PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    network VARCHAR(20) NOT NULL,
    depth INT DEFAULT 0,
    priority FLOAT DEFAULT 0.5,
    status VARCHAR(20) DEFAULT 'pending',
    retry_count INT DEFAULT 0,
    source_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
