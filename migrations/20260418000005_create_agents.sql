CREATE TABLE agents (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                 TEXT NOT NULL,
    workspace_id         UUID NOT NULL REFERENCES workspaces(id),
    owner_id             UUID NOT NULL REFERENCES users(id),
    status               TEXT NOT NULL DEFAULT 'asleep'
                         CHECK (status IN ('asleep', 'awake', 'maintenance')),
    wake_id              UUID,
    wake_started_at      TIMESTAMPTZ,
    wake_iteration_count INT NOT NULL DEFAULT 0,
    permission_mode      TEXT NOT NULL DEFAULT 'yolo'
                         CHECK (permission_mode IN ('yolo', 'supervised', 'locked')),
    is_enabled           BOOLEAN NOT NULL DEFAULT TRUE,
    disabled_reason      TEXT,
    disabled_at          TIMESTAMPTZ,
    budget_limit_usd     NUMERIC(12, 6) NOT NULL DEFAULT 10.000000,
    budget_used_usd      NUMERIC(12, 6) NOT NULL DEFAULT 0.000000
                         CHECK (budget_used_usd >= 0),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agents_workspace ON agents(workspace_id);
CREATE INDEX idx_agents_owner ON agents(owner_id);
CREATE INDEX idx_agents_status ON agents(status);
