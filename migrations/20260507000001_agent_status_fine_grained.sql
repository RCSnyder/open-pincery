-- AC-82 (v9 Phase G Slice G7a): Widen the agents.status CHECK constraint
-- to include the five fine-grained TLA+-named variants the runtime is
-- about to start writing:
--
--   prompt_assembling, tool_dispatching, tool_executing,
--   tool_result_processing, mid_wake_event_polling
--
-- Per scaffolding/readiness.md T-AC82-1, this is a forward-only widen:
-- no row is mutated. The constraint must accept the five new strings
-- before any code starts emitting them, otherwise the first CAS UPDATE
-- writing `prompt_assembling` would fail the CHECK.
--
-- The wake_acquiring and wake_ending values were already admitted by
-- migration 20260420000001_agent_status_states.sql (AC-34 reserved
-- values); AC-82 begins actually writing them.

ALTER TABLE agents DROP CONSTRAINT IF EXISTS agents_status_check;

ALTER TABLE agents
    ADD CONSTRAINT agents_status_check
    CHECK (status IN (
        'asleep',
        'wake_acquiring',
        'prompt_assembling',
        'awake',
        'tool_dispatching',
        'tool_executing',
        'tool_result_processing',
        'mid_wake_event_polling',
        'wake_ending',
        'maintenance'
    ));
