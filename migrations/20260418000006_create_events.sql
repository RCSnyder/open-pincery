CREATE TABLE events (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id           UUID NOT NULL REFERENCES agents(id),
    event_type         TEXT NOT NULL,
    source             TEXT,
    wake_id            UUID,
    tool_name          TEXT,
    tool_input         TEXT,
    tool_output        TEXT,
    content            TEXT,
    termination_reason TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_events_agent_created ON events(agent_id, created_at);
CREATE INDEX idx_events_agent_type ON events(agent_id, event_type);
CREATE INDEX idx_events_wake ON events(wake_id);
