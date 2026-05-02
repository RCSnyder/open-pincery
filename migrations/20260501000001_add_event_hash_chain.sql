-- AC-78: Event-Log Hash Chain
--
-- Per-agent SHA-256 hash chain. A BEFORE INSERT trigger computes
-- entry_hash = sha256(prev_hash || canonical_payload(NEW)) and stores
-- both columns, holding a row lock on the immediately preceding event
-- for the same agent so concurrent inserts cannot fork the chain.
--
-- The canonical payload is length-prefixed (u32 big-endian length
-- followed by UTF-8 bytes for each field, in fixed order). This avoids
-- delimiter ambiguity and is trivially reproducible in Rust by the
-- verifier in src/background/audit_chain.rs.
--
-- Backfill walks every agent's events in (created_at, id) order and
-- writes both columns; then both are tightened to NOT NULL. The whole
-- migration runs in a single transaction (sqlx default).

CREATE EXTENSION IF NOT EXISTS pgcrypto;

ALTER TABLE events
    ADD COLUMN prev_hash  TEXT,
    ADD COLUMN entry_hash TEXT;

-- ---------------------------------------------------------------------
-- Canonical pre-image helper. Length-prefixed so any byte sequence in
-- text fields (incl. NUL, '|', JSON-like content) is unambiguous.
-- ---------------------------------------------------------------------
CREATE OR REPLACE FUNCTION events_chain_field(v text) RETURNS bytea AS $$
DECLARE
    b bytea;
BEGIN
    IF v IS NULL THEN
        b := convert_to('', 'UTF8');
    ELSE
        b := convert_to(v, 'UTF8');
    END IF;
    RETURN int4send(length(b)) || b;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION events_chain_canonical_payload(
    p_event_type        text,
    p_agent_id          uuid,
    p_source            text,
    p_wake_id           uuid,
    p_tool_name         text,
    p_tool_input        text,
    p_tool_output       text,
    p_content           text,
    p_termination_reason text,
    p_created_at        timestamptz
) RETURNS bytea AS $$
DECLARE
    micros bigint;
BEGIN
    -- Microseconds since the UNIX epoch, big-endian i64. Postgres
    -- timestamptz precision is microseconds, so this is lossless.
    micros := (extract(epoch from p_created_at) * 1000000)::bigint;
    RETURN events_chain_field(p_event_type)
        || events_chain_field(coalesce(p_agent_id::text, ''))
        || events_chain_field(p_source)
        || events_chain_field(coalesce(p_wake_id::text, ''))
        || events_chain_field(p_tool_name)
        || events_chain_field(p_tool_input)
        || events_chain_field(p_tool_output)
        || events_chain_field(p_content)
        || events_chain_field(p_termination_reason)
        || int4send(8) || int8send(micros);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- ---------------------------------------------------------------------
-- BEFORE INSERT trigger: lock the prior event for this agent, compute
-- prev_hash and entry_hash.
-- ---------------------------------------------------------------------
CREATE OR REPLACE FUNCTION events_chain_compute_hash() RETURNS TRIGGER AS $$
DECLARE
    prior_hash text;
    payload    bytea;
BEGIN
    -- Default created_at if not set (matches column default semantics).
    IF NEW.created_at IS NULL THEN
        NEW.created_at := NOW();
    END IF;

    -- Per-agent advisory lock serializes chain inserts so concurrent
    -- transactions cannot both compute against the same prev (which
    -- would happen at genesis or whenever MVCC hides an in-flight
    -- INSERT from a peer). The lock is released at transaction end.
    -- Class 44224 (0xACC0) namespaces this to the events chain;
    -- hashtext maps the agent UUID into int4.
    PERFORM pg_advisory_xact_lock(44224, hashtext(NEW.agent_id::text));

    -- Lock the most recent event for this agent so two concurrent
    -- inserts cannot both compute against the same prev. Returns NULL
    -- (genesis) if no prior event exists for this agent.
    SELECT entry_hash INTO prior_hash
    FROM events
    WHERE agent_id = NEW.agent_id
    ORDER BY created_at DESC, id DESC
    LIMIT 1
    FOR UPDATE;

    NEW.prev_hash := coalesce(prior_hash, '');

    payload := events_chain_canonical_payload(
        NEW.event_type,
        NEW.agent_id,
        NEW.source,
        NEW.wake_id,
        NEW.tool_name,
        NEW.tool_input,
        NEW.tool_output,
        NEW.content,
        NEW.termination_reason,
        NEW.created_at
    );

    NEW.entry_hash := encode(
        digest(decode(NEW.prev_hash, 'hex') || payload, 'sha256'),
        'hex'
    );
    -- Note: the genesis case (prev_hash = '') decodes to empty bytea,
    -- so the hash input is just the canonical payload — well-defined.

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ---------------------------------------------------------------------
-- Backfill existing rows. Walks each agent's events in stable order
-- and writes both columns. Empty databases are no-ops.
-- ---------------------------------------------------------------------
DO $$
DECLARE
    r            record;
    last_hash    text;
    cur_agent    uuid := NULL;
    payload      bytea;
BEGIN
    FOR r IN
        SELECT id, agent_id, event_type, source, wake_id, tool_name,
               tool_input, tool_output, content, termination_reason,
               created_at
        FROM events
        ORDER BY agent_id, created_at, id
    LOOP
        IF cur_agent IS DISTINCT FROM r.agent_id THEN
            last_hash := '';
            cur_agent := r.agent_id;
        END IF;

        payload := events_chain_canonical_payload(
            r.event_type,
            r.agent_id,
            r.source,
            r.wake_id,
            r.tool_name,
            r.tool_input,
            r.tool_output,
            r.content,
            r.termination_reason,
            r.created_at
        );

        UPDATE events
        SET prev_hash = last_hash,
            entry_hash = encode(
                digest(decode(last_hash, 'hex') || payload, 'sha256'),
                'hex'
            )
        WHERE id = r.id
        RETURNING entry_hash INTO last_hash;
    END LOOP;
END;
$$;

ALTER TABLE events
    ALTER COLUMN prev_hash  SET NOT NULL,
    ALTER COLUMN entry_hash SET NOT NULL;

CREATE TRIGGER events_chain_compute_hash_trigger
BEFORE INSERT ON events
FOR EACH ROW
EXECUTE FUNCTION events_chain_compute_hash();
