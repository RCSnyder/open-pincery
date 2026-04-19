CREATE TABLE user_sessions (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id            UUID NOT NULL REFERENCES users(id),
    session_token_hash TEXT NOT NULL UNIQUE,
    auth_provider      TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at         TIMESTAMPTZ NOT NULL,
    ip_address         INET,
    user_agent         TEXT,
    revoked_at         TIMESTAMPTZ
);

CREATE INDEX idx_sessions_user ON user_sessions(user_id);
