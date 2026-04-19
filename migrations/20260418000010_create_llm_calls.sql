CREATE TABLE llm_calls (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id          UUID NOT NULL REFERENCES agents(id),
    wake_id           UUID NOT NULL,
    call_type         TEXT NOT NULL,
    model             TEXT NOT NULL,
    prompt_hash       TEXT NOT NULL,
    prompt_template   TEXT,
    prompt_tokens     INT,
    completion_tokens INT,
    total_tokens      INT,
    cost_usd          NUMERIC(10, 6),
    latency_ms        INT,
    response_hash     TEXT NOT NULL,
    finish_reason     TEXT,
    temperature       FLOAT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_llm_calls_agent ON llm_calls(agent_id, created_at);

CREATE TABLE llm_call_prompts (
    llm_call_id   UUID PRIMARY KEY REFERENCES llm_calls(id),
    system_prompt TEXT NOT NULL,
    messages_json JSONB NOT NULL,
    tools_json    JSONB,
    response_text TEXT NOT NULL
);
