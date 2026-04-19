CREATE TABLE organization_memberships (
    organization_id UUID NOT NULL REFERENCES organizations(id),
    user_id         UUID NOT NULL REFERENCES users(id),
    role            TEXT NOT NULL DEFAULT 'org_owner',
    status          TEXT NOT NULL DEFAULT 'active',
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    invited_by      UUID REFERENCES users(id),
    PRIMARY KEY (organization_id, user_id)
);

CREATE TABLE workspace_memberships (
    workspace_id UUID NOT NULL REFERENCES workspaces(id),
    user_id      UUID NOT NULL REFERENCES users(id),
    role         TEXT NOT NULL DEFAULT 'workspace_owner',
    status       TEXT NOT NULL DEFAULT 'active',
    joined_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    invited_by   UUID REFERENCES users(id),
    PRIMARY KEY (workspace_id, user_id)
);
