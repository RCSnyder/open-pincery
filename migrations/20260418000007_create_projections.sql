CREATE TABLE agent_projections (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id   UUID NOT NULL REFERENCES agents(id),
    identity   TEXT NOT NULL DEFAULT '',
    work_list  TEXT NOT NULL DEFAULT '',
    version    INT NOT NULL DEFAULT 1,
    wake_id    UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_projections_agent_version ON agent_projections(agent_id, version DESC);
