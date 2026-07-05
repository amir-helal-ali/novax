-- +migrate Up
-- Migration #006: Novax Engine — projects + entities storage
-- يخزّن مشاريع Novax (إعدادات + كيانات) كـ JSON في PostgreSQL

CREATE TABLE IF NOT EXISTS novax_projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    description TEXT,
    config JSONB NOT NULL DEFAULT '{}',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_novax_projects_name ON novax_projects (name);
CREATE INDEX IF NOT EXISTS idx_novax_projects_enabled ON novax_projects (enabled) WHERE enabled = TRUE;

-- +migrate Down
DROP TABLE IF EXISTS novax_projects;
