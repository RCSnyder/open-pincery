CREATE TABLE workspaces (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL,
    created_by      UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at     TIMESTAMPTZ,
    UNIQUE (organization_id, slug)
);
