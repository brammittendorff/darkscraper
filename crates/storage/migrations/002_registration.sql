-- Add registration detection columns to pages table
ALTER TABLE pages ADD COLUMN IF NOT EXISTS has_register_form BOOLEAN DEFAULT FALSE;
ALTER TABLE pages ADD COLUMN IF NOT EXISTS has_captcha BOOLEAN DEFAULT FALSE;
ALTER TABLE pages ADD COLUMN IF NOT EXISTS requires_email BOOLEAN DEFAULT FALSE;
ALTER TABLE pages ADD COLUMN IF NOT EXISTS is_forum BOOLEAN DEFAULT FALSE;

-- Create index for finding registration opportunities
CREATE INDEX IF NOT EXISTS idx_pages_register_form ON pages(has_register_form) WHERE has_register_form = true;
CREATE INDEX IF NOT EXISTS idx_pages_captcha ON pages(has_captcha) WHERE has_captcha = true;

-- Table for storing registered accounts
CREATE TABLE IF NOT EXISTS registered_accounts (
    id BIGSERIAL PRIMARY KEY,
    site_url TEXT NOT NULL,
    site_domain TEXT NOT NULL,
    network VARCHAR(20) NOT NULL,
    username VARCHAR(255) NOT NULL,
    password TEXT NOT NULL,
    email VARCHAR(255),
    email_provider TEXT,
    email_verified BOOLEAN DEFAULT FALSE,
    registration_url TEXT NOT NULL,
    session_cookies TEXT,  -- JSON-encoded cookies
    user_agent TEXT,
    registration_notes TEXT,  -- JSON: captcha_type, form_fields, etc.
    status VARCHAR(20) DEFAULT 'active',  -- active, banned, inactive, verification_pending
    last_login_at TIMESTAMPTZ,
    registered_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(site_domain, username)
);

CREATE INDEX IF NOT EXISTS idx_accounts_domain ON registered_accounts(site_domain);
CREATE INDEX IF NOT EXISTS idx_accounts_network ON registered_accounts(network);
CREATE INDEX IF NOT EXISTS idx_accounts_status ON registered_accounts(status);
CREATE INDEX IF NOT EXISTS idx_accounts_email_verified ON registered_accounts(email_verified);

-- Table for temporary email addresses (self-hosted)
CREATE TABLE IF NOT EXISTS temp_emails (
    id BIGSERIAL PRIMARY KEY,
    email_address VARCHAR(255) NOT NULL UNIQUE,
    domain VARCHAR(255) NOT NULL,
    password VARCHAR(255),
    inbox_url TEXT,  -- URL to check inbox (if using external service)
    status VARCHAR(20) DEFAULT 'active',  -- active, expired, used
    account_id BIGINT REFERENCES registered_accounts(id),  -- Link to account using this email
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_temp_emails_status ON temp_emails(status);
CREATE INDEX IF NOT EXISTS idx_temp_emails_expires ON temp_emails(expires_at);

-- Table for received emails (for verification links)
CREATE TABLE IF NOT EXISTS received_emails (
    id BIGSERIAL PRIMARY KEY,
    temp_email_id BIGINT REFERENCES temp_emails(id),
    from_address TEXT,
    subject TEXT,
    body_text TEXT,
    body_html TEXT,
    verification_link TEXT,  -- Extracted verification URL
    received_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_received_emails_temp_email ON received_emails(temp_email_id);

-- Table for CAPTCHA solving attempts
CREATE TABLE IF NOT EXISTS captcha_solves (
    id BIGSERIAL PRIMARY KEY,
    site_url TEXT NOT NULL,
    captcha_type VARCHAR(50),  -- recaptcha_v2, recaptcha_v3, hcaptcha, text, image
    captcha_key TEXT,  -- Site key for reCAPTCHA/hCaptcha
    solution TEXT,
    solver_service VARCHAR(50),  -- 2captcha, anti-captcha, manual, etc.
    cost_usd FLOAT,
    solve_time_seconds INT,
    success BOOLEAN,
    error_message TEXT,
    solved_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_captcha_site ON captcha_solves(site_url);
CREATE INDEX IF NOT EXISTS idx_captcha_type ON captcha_solves(captcha_type);

-- Table for registration attempts (success & failures)
CREATE TABLE IF NOT EXISTS registration_attempts (
    id BIGSERIAL PRIMARY KEY,
    site_url TEXT NOT NULL,
    site_domain TEXT NOT NULL,
    network VARCHAR(20),
    username VARCHAR(255),
    email VARCHAR(255),
    captcha_required BOOLEAN DEFAULT FALSE,
    captcha_solved BOOLEAN DEFAULT FALSE,
    email_verification_required BOOLEAN DEFAULT FALSE,
    email_verified BOOLEAN DEFAULT FALSE,
    success BOOLEAN DEFAULT FALSE,
    error_message TEXT,
    account_id BIGINT REFERENCES registered_accounts(id),  -- Link to created account
    attempted_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_reg_attempts_domain ON registration_attempts(site_domain);
CREATE INDEX IF NOT EXISTS idx_reg_attempts_success ON registration_attempts(success);
CREATE INDEX IF NOT EXISTS idx_reg_attempts_time ON registration_attempts(attempted_at DESC);

-- View for registration opportunities
CREATE OR REPLACE VIEW registration_opportunities AS
SELECT
    p.id,
    p.url,
    p.domain,
    p.network,
    p.title,
    p.has_register_form,
    p.has_captcha,
    p.requires_email,
    p.has_login_form,
    COUNT(DISTINCT ra.id) as existing_accounts,
    MAX(ra.registered_at) as last_registration
FROM pages p
LEFT JOIN registered_accounts ra ON p.domain = ra.site_domain
WHERE p.has_register_form = true
GROUP BY p.id, p.url, p.domain, p.network, p.title, p.has_register_form, p.has_captcha, p.requires_email, p.has_login_form
ORDER BY existing_accounts ASC, p.fetched_at DESC;

-- View for account statistics
CREATE OR REPLACE VIEW account_statistics AS
SELECT
    network,
    COUNT(*) as total_accounts,
    COUNT(*) FILTER (WHERE status = 'active') as active_accounts,
    COUNT(*) FILTER (WHERE status = 'banned') as banned_accounts,
    COUNT(*) FILTER (WHERE email_verified = true) as verified_accounts,
    COUNT(*) FILTER (WHERE last_login_at > NOW() - INTERVAL '7 days') as recently_active
FROM registered_accounts
GROUP BY network;
