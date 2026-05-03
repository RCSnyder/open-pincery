-- AC-80: Capability Nonce / Freshness (Phase G Slice G5)
--
-- Per-IssueToolCall freshness binding. On `AuthorizeExecution` the
-- runtime mints a 16-byte random nonce bound to {wake_id, tool_name,
-- capability_shape, expires_at} (see src/runtime/capability_nonce.rs);
-- on `IssueToolCall` an atomic single-statement UPDATE consumes the
-- nonce. Replays, cross-wake reuse, expired tokens, and shape
-- mismatches all reject with a `capability_nonce_rejected` event.
--
-- Workspace-scoped (AC-65) so a leaked nonce from workspace A cannot
-- be redeemed under workspace B even if the 16 random bytes were
-- somehow guessed.
--
-- Storage growth: lazy GC only in v9.0; rows expire by predicate
-- (`expires_at > now()`). A periodic background sweep is deferred to
-- v9.1 and noted in DELIVERY.md "Known Limitations".

CREATE TABLE capability_nonces (
    id                 uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    wake_id            uuid        NOT NULL,
    tool_name          text        NOT NULL,
    capability_shape   text        NOT NULL,
    nonce              bytea       NOT NULL,
    expires_at         timestamptz NOT NULL,
    consumed_at        timestamptz,
    workspace_id       uuid        NOT NULL,
    created_at         timestamptz NOT NULL DEFAULT now()
);

-- Hot path: consume scopes by (workspace_id, nonce). Unique because
-- the 16-byte random space is wide enough that a collision is treated
-- as a programming error, not a real-world event; this also blocks a
-- duplicate-mint regression from going unnoticed.
CREATE UNIQUE INDEX capability_nonces_lookup
    ON capability_nonces (workspace_id, nonce);

-- For future periodic sweep + ad-hoc operator queries.
CREATE INDEX capability_nonces_expiry
    ON capability_nonces (expires_at);
