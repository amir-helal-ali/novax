-- +migrate Up
-- Migration #005: UUID verification + SEO-friendly slugs for users
-- Ensures all IDs use UUID (already the case — this migration adds slugs and audit fields)

-- Add slug column for SEO-friendly user profile URLs (e.g. /u/alice)
ALTER TABLE users ADD COLUMN IF NOT EXISTS slug TEXT UNIQUE;

-- Auto-generate slug from name for existing users (lowercase, hyphenated)
UPDATE users SET slug = LOWER(REGEXP_REPLACE(name, '[^a-zA-Z0-9]+', '-', 'g'))
WHERE slug IS NULL AND name IS NOT NULL;

-- Remove leading/trailing hyphens
UPDATE users SET slug = BTRIM(slug, '-')
WHERE slug IS NOT NULL;

-- Ensure uniqueness for duplicate slugs (append short UUID suffix)
UPDATE users SET slug = slug || '-' || LEFT(id::text, 8)
WHERE slug IS NOT NULL AND id IN (
    SELECT id FROM (
        SELECT id, slug, ROW_NUMBER() OVER (PARTITION BY slug ORDER BY created_at) as rn
        FROM users WHERE slug IS NOT NULL
    ) dupes WHERE rn > 1
);

-- Add audit log table (all UUID-based)
CREATE TABLE IF NOT EXISTS audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID NULL,
    metadata JSONB NULL DEFAULT '{}',
    ip_address TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log (actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log (entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log (created_at DESC);

-- Add search index for users (used by admin search)
CREATE INDEX IF NOT EXISTS idx_users_search ON users USING gin (to_tsvector('english', email || ' ' || name || ' ' || COALESCE(slug, '')));

-- +migrate Down
DROP INDEX IF EXISTS idx_users_search;
DROP TABLE IF EXISTS audit_log;
ALTER TABLE users DROP COLUMN IF EXISTS slug;
