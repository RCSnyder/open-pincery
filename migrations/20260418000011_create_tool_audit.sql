CREATE TABLE tool_audit (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        UUID NOT NULL REFERENCES agents(id),
    wake_id         UUID NOT NULL,
    llm_call_id     UUID REFERENCES llm_calls(id),
    tool_name       TEXT NOT NULL,
    tool_input      TEXT,
    tool_output     TEXT,
    category        TEXT NOT NULL DEFAULT 'execute',
    permission_mode TEXT NOT NULL DEFAULT 'yolo',
    approval_id     UUID,
    sandbox_profile TEXT,
    credentials_used TEXT[],
    exit_code       INT,
    duration_ms     INT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tool_audit_agent ON tool_audit(agent_id, created_at);
