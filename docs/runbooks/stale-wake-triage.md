# Stale Wake Triage

## Symptom

One or more agents appear "stuck" — they are in `awake` or `maintenance` status
but have stopped emitting `wake_end` events, the UI shows them as busy for an
unreasonable time, or scraping `/metrics` shows `open_pincery_wake_started_total`
growing without matching `open_pincery_wake_completed_total{reason="..."}` values.

The built-in stale recovery task (AC-8) normally catches this after
`STALE_WAKE_HOURS` (default 2 h), but during incidents you may need to confirm,
accelerate, or investigate the cause.

## Diagnostic Commands

```bash
# 1. List every agent currently not in 'asleep' with how long it has been awake.
psql "$DATABASE_URL" -c "
  SELECT id, status, wake_id, wake_started_at,
         now() - wake_started_at AS awake_for
  FROM agents
  WHERE status <> 'asleep'
  ORDER BY wake_started_at ASC NULLS LAST;
"

# 2. See the last 20 events for a specific agent (replace $AGENT).
psql "$DATABASE_URL" -c "
  SELECT created_at, event_type, source, tool_name, termination_reason
  FROM events
  WHERE agent_id = '$AGENT'
  ORDER BY created_at DESC
  LIMIT 20;
"

# 3. Check the most recent LLM call status for this agent.
psql "$DATABASE_URL" -c "
  SELECT created_at, model, status, duration_ms, error_message
  FROM llm_calls
  WHERE agent_id = '$AGENT'
  ORDER BY created_at DESC
  LIMIT 5;
"

# 4. If you have Prometheus, confirm the imbalance.
curl -s "$METRICS_ADDR/metrics" | grep -E 'open_pincery_wake_(started|completed)_total'
```

## Remediation

1. If the LLM call in step 3 is `error` and you see network or API problems,
   fix the upstream first. The stale recovery task will release the agent
   within `STALE_WAKE_HOURS`.
2. To force-release a single agent immediately:

   ```bash
   psql "$DATABASE_URL" -c "
     UPDATE agents
     SET status = 'asleep', wake_id = NULL, wake_started_at = NULL
     WHERE id = '$AGENT' AND status IN ('awake','maintenance');
   "
   psql "$DATABASE_URL" -c "
     INSERT INTO events (agent_id, event_type, source, termination_reason)
     VALUES ('$AGENT', 'stale_wake_recovery', 'operator', 'manual_release');
   "
   ```

3. To force-release all stale agents at once (bypass the 2 h window):

   ```bash
   psql "$DATABASE_URL" -c "
     UPDATE agents
     SET status = 'asleep', wake_id = NULL, wake_started_at = NULL
     WHERE status IN ('awake','maintenance')
       AND wake_started_at < now() - interval '30 minutes';
   "
   ```

## Escalation

If agents repeatedly go stale within a short window (> 3 per hour), treat this
as a real incident rather than noise:

- Capture `docker logs` / systemd journal output for the runtime.
- Snapshot the `events` and `llm_calls` tables for the affected agents.
- Open an incident ticket with those artefacts and the `/metrics` scrape.
- Disable webhook ingress (set `is_enabled=false` on the affected agents) to
  stop the bleeding while you investigate.
