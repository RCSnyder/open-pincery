CREATE TABLE auth_audit (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID REFERENCES users(id),
    auth_provider TEXT NOT NULL,
    event_type    TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ip_address    INET,
    user_agent    TEXT
);

CREATE INDEX idx_auth_audit_user ON auth_audit(user_id, created_at);
