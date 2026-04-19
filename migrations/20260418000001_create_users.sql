CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    auth_provider TEXT NOT NULL,
    auth_subject  TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE(auth_provider, auth_subject)
);
