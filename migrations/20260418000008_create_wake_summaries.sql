CREATE TABLE wake_summaries (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id   UUID NOT NULL REFERENCES agents(id),
    wake_id    UUID NOT NULL,
    summary    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_wake_summaries_agent ON wake_summaries(agent_id, created_at DESC);
