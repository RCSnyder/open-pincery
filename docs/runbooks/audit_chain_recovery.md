# Audit-Chain Recovery

## Symptom

Open Pincery refuses to boot and exits with code **5**. Logs include a
structured error event:

```
event=audit_chain_broken
broken_agent_count=<n>
"Audit chain broken on at least one agent; refusing to boot"
```

This means the per-agent SHA-256 event hash chain (AC-78) detected a
mismatch between a stored `entry_hash` and the hash recomputed from the
canonical pre-image during the startup integrity walk.

The mismatch is **always significant**. A clean Open Pincery deployment
cannot produce a broken chain through normal operation — the BEFORE INSERT
trigger advances the chain atomically inside a row-locking transaction.
A broken chain means one of:

1. A direct `UPDATE`/`DELETE` against `events` (e.g. ad-hoc psql, ORM
   misuse, restore from a partial dump, replication divergence).
2. A storage corruption event (disk, filesystem, replica drift).
3. A malicious tamper attempt against the audit log.

The startup gate refuses to proceed because every later event would
otherwise inherit a hash chain that lies about its history.

## Diagnostic Commands

```bash
# 1. Identify which agents have broken chains. Run from any host that
#    can reach the database.
pcy audit verify

# 2. Drill into a specific agent. Returns first_divergent_event_id and
#    the expected vs actual hashes at the point the chain breaks.
pcy audit verify --agent <agent-uuid>

# 3. Look at the divergent event row directly.
psql "$DATABASE_URL" -c "
  SELECT id, agent_id, created_at, event_type, source, tool_name,
         length(content) AS content_len,
         prev_hash, entry_hash
  FROM events
  WHERE id = '<first_divergent_event_id>';
"

# 4. Compare the chain in primary vs replica (if you suspect drift).
psql "$REPLICA_URL" -c "
  SELECT id, prev_hash, entry_hash
  FROM events
  WHERE id = '<first_divergent_event_id>';
"
```

## Recovery Decision

Pick exactly one path below. Document which one in your incident record.

### Path A — Restore from backup (preferred)

Use this when the breakage is recent and a clean point-in-time backup
exists. This is the only path that fully restores integrity.

1. Take Open Pincery down (it is already refusing to boot).
2. Identify the last good backup whose timestamp predates
   `first_divergent_event_id.created_at`.
3. Follow `docs/runbooks/db-restore.md` to restore.
4. Re-run `pcy audit verify` against the restored database. It must
   return `all_verified: true`. If it does not, restore an older backup.
5. Restart Open Pincery. The startup gate will pass and boot will
   complete normally.
6. File an incident report. Include: who/what reached the database,
   `first_divergent_event_id`, both hash values, the backup snapshot
   used, and the gap of dropped events between the backup and the
   pre-tamper state.

### Path B — Forensic preservation, then quarantine restart (incident response)

Use this when you must keep the broken database for forensic analysis
(e.g. you suspect an active intruder) but you also need the system back
up.

1. Take a full dump of the current `events` table to immutable storage:

   ```bash
   pg_dump --table=events "$DATABASE_URL" \
     | gzip > "events-broken-$(date -u +%Y%m%dT%H%M%SZ).sql.gz"
   ```

2. Provision a fresh Open Pincery instance against a new database.
   Do **not** copy the broken `events` rows.
3. Coordinate with security. Treat the broken chain as evidence.
4. Once forensics complete, choose Path A on the original database or
   decommission it.

### Path C — Override and proceed (last resort, time-boxed)

Use this **only** when:

- You have already taken Path A or Path B for the data,
- You need the running process up to ship something else, and
- A human operator has signed off on the override.

This path knowingly continues operation against a database whose audit
log is no longer cryptographically intact. Every later event will be
cryptographically chained to the broken state — the chain becomes
"clean from this point forward" but the historical break remains
permanently visible.

To engage the override:

```bash
export OPEN_PINCERY_AUDIT_CHAIN_FLOOR=relaxed
export OPEN_PINCERY_ALLOW_UNSAFE=true
# restart Open Pincery
```

Both variables are required. Either one alone is refused.

When the override is armed, the startup gate:

- Logs a `audit_chain_floor_relaxed` warning at boot.
- Appends one `audit_chain_floor_relaxed` event per broken agent to
  the events table so the override is itself part of the audit trail.
- Returns success and lets boot proceed.

After the override has served its purpose:

1. Unset both environment variables.
2. Run `pcy audit verify` again to confirm whether the chain healed
   (it will not, unless you also took Path A).
3. File or update the incident report with the duration of the
   override window and the operator who authorized it.

## Verification

Regardless of path:

```bash
# Confirm the gate now passes cleanly without overrides set.
unset OPEN_PINCERY_AUDIT_CHAIN_FLOOR OPEN_PINCERY_ALLOW_UNSAFE
pcy audit verify
# expected: all_verified: true, exit code 0
```

If `pcy audit verify` exits non-zero (`2`) or the startup gate continues
to refuse boot (exit `5`), the chain is still broken and you have not
finished recovery.

## Related

- AC-78 — Event-log hash chain (scope.md)
- `docs/runbooks/db-restore.md` — point-in-time restore procedure
- `src/background/audit_chain.rs` — verifier + startup gate implementation
- `migrations/20260501000001_add_event_hash_chain.sql` — chain trigger
