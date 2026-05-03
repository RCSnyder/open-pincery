-- AC-34 (v6): Extend the agents.status CHECK constraint to include the two
-- reserved lifecycle values ('wake_acquiring', 'wake_ending') used by the
-- TLA+ specification in OpenPinceryAgent.tla. v6 runtime code never writes
-- these values; the constraint widen is a pure additive change so a future
-- CAS split (scope.md v10 Deferred) can roll out migrations-first.
--
-- No existing row is mutated — we only swap the CHECK constraint.

ALTER TABLE agents DROP CONSTRAINT IF EXISTS agents_status_check;

ALTER TABLE agents
    ADD CONSTRAINT agents_status_check
    CHECK (status IN ('asleep', 'wake_acquiring', 'awake', 'wake_ending', 'maintenance'));
