CREATE TABLE webhook_dedup (
    idempotency_key TEXT NOT NULL,
    agent_id UUID NOT NULL REFERENCES agents(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (idempotency_key, agent_id)
);

CREATE INDEX idx_webhook_dedup_agent ON webhook_dedup (agent_id);
