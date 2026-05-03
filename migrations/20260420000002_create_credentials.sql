-- AC-38 / AC-39 (v7): workspace-scoped encrypted credential vault.
--
-- Ciphertext and nonce are stored as BYTEA. The AES-256-GCM tag is
-- appended to ciphertext by the `aes-gcm` crate, so the minimum useful
-- ciphertext length is 16 bytes (tag only, zero-byte plaintext is
-- disallowed upstream at the API layer via a 1..=8192 length check).
--
-- The one-active-per-name invariant is enforced by a unique partial index
-- on the not-yet-revoked rows. Revoking a credential (setting revoked_at)
-- releases the name so a new credential can be stored under it later.

CREATE TABLE credentials (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  UUID NOT NULL REFERENCES workspaces(id),
    name          TEXT NOT NULL,
    ciphertext    BYTEA NOT NULL,
    nonce         BYTEA NOT NULL,
    created_by    UUID NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at    TIMESTAMPTZ,
    CHECK (length(nonce) = 12),
    CHECK (length(ciphertext) >= 16),
    CHECK (name ~ '^[a-z0-9_]{1,64}$')
);

CREATE UNIQUE INDEX credentials_one_active_per_name
    ON credentials (workspace_id, name)
    WHERE revoked_at IS NULL;

CREATE INDEX credentials_workspace_idx ON credentials (workspace_id);

-- AC-39: extend auth_audit for workspace-level audit events
-- (credential_added / credential_revoked / credential_forbidden).
-- The existing user_agent text column cannot carry structured
-- {workspace_id, name, actor} metadata cleanly; a JSONB column is the
-- minimal extension. Existing rows get NULL which is schema-compatible.
ALTER TABLE auth_audit
    ADD COLUMN IF NOT EXISTS workspace_id UUID REFERENCES workspaces(id),
    ADD COLUMN IF NOT EXISTS details JSONB;

CREATE INDEX IF NOT EXISTS idx_auth_audit_workspace
    ON auth_audit (workspace_id, created_at)
    WHERE workspace_id IS NOT NULL;
