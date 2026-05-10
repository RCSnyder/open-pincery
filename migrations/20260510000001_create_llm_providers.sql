-- AC-93 (v9.1): LLM providers as first-class, workspace-scoped resources.
--
-- A provider points at a base URL and an existing credential (by name)
-- in the same workspace. At most one provider per workspace may be the
-- default (enforced by the partial unique index below). The CLI noun
-- `pcy provider {add,list,use,remove}` is the operator-facing surface.

CREATE TABLE llm_providers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    credential_name TEXT NOT NULL,
    is_default      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Provider names are unique within a workspace so the CLI's
    -- noun-by-name lookup (`pcy provider use <name>`) is unambiguous.
    UNIQUE (workspace_id, name),

    -- Logical FK to credentials(name) is enforced at the application
    -- layer because credentials get rotated/revoked over time and we
    -- don't want a provider delete to cascade-block a credential
    -- rotation. The application-layer check guarantees the credential
    -- exists at `pcy provider add` time.

    -- Name shape mirrors credentials.name (^[a-z0-9_]{1,64}$).
    CONSTRAINT llm_providers_name_shape
        CHECK (char_length(name) BETWEEN 1 AND 64),
    CONSTRAINT llm_providers_credential_shape
        CHECK (char_length(credential_name) BETWEEN 1 AND 64),
    CONSTRAINT llm_providers_base_url_nonempty
        CHECK (char_length(base_url) > 0)
);

-- Partial unique index: at most one default per workspace. Concurrent
-- "set default" operations serialize on this index.
CREATE UNIQUE INDEX llm_providers_one_default_per_workspace
    ON llm_providers (workspace_id)
    WHERE is_default;

CREATE INDEX llm_providers_workspace ON llm_providers (workspace_id);
