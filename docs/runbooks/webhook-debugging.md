# Webhook Debugging

## Symptom

An external integration is sending webhooks to
`POST /api/agents/:id/webhooks`, but one of the following is true:

- The caller sees `401 Unauthorized` — signature rejected.
- The caller sees `404 Not Found` — wrong agent id.
- The caller sees `403 Forbidden` — the agent is disabled.
- The caller gets `202 Accepted` but the agent never wakes.
- The caller gets `200 {"status":"duplicate"}` on every request — idempotency
  key collision.

## Diagnostic Commands

```bash
# 1. Confirm the agent exists and is enabled.
psql "$DATABASE_URL" -c "
  SELECT id, is_enabled, disabled_reason, substring(webhook_secret, 1, 8) AS webhook_secret_head
  FROM agents
  WHERE id = '$AGENT';
"

# 2. Look at the last 20 webhook-related events for this agent.
psql "$DATABASE_URL" -c "
  SELECT created_at, event_type, source, content
  FROM events
  WHERE agent_id = '$AGENT' AND event_type IN ('webhook_received', 'wake_start')
  ORDER BY created_at DESC
  LIMIT 20;
"

# 3. Count recent webhook receipts via Prometheus.
curl -s "$METRICS_ADDR/metrics" | grep open_pincery_webhook_received_total

# 4. Inspect idempotency dedupe entries for this agent in the last hour.
psql "$DATABASE_URL" -c "
  SELECT idempotency_key, first_seen_at
  FROM webhook_dedup
  WHERE agent_id = '$AGENT'
    AND first_seen_at > now() - interval '1 hour'
  ORDER BY first_seen_at DESC
  LIMIT 20;
"

# 5. Reproduce the signature the caller should be sending.
#    Replace $SECRET with the webhook_secret returned when the agent was created.
printf '%s' "$PAYLOAD" | \
  openssl dgst -sha256 -hmac "$SECRET" | awk '{print "sha256=" $2}'

# 6. Tail the application logs for this path.
docker compose logs -f --since 5m app | grep -E 'webhooks|webhook_received'
```

## Remediation

### 401 Unauthorized (bad signature)

- The `x-webhook-signature` header must be `sha256=<hex>` where `<hex>` is
  HMAC-SHA256 of the **raw request body** using the agent's `webhook_secret`.
  No JSON re-serialisation, no trailing newline.
- Confirm the caller is using the secret returned by
  `POST /api/agents` (only shown once, on create). If it has been lost,
  rotate:

  ```bash
  psql "$DATABASE_URL" -c "
    UPDATE agents
    SET webhook_secret = encode(gen_random_bytes(32), 'hex')
    RETURNING id, webhook_secret;
  " # record the new secret and re-share with the integration.
  ```

### 403 Forbidden (agent disabled)

```bash
psql "$DATABASE_URL" -c "
  UPDATE agents SET is_enabled = true, disabled_reason = NULL
  WHERE id = '$AGENT';
"
```

### 202 Accepted but agent never wakes

- Confirm `pg_notify` fired. The application logs should include
  `NOTIFY agent_wake` for that agent id.
- Confirm the background listener is alive. `/ready` should return `200`.
  If it returns `503 {"failing":"background_tasks"}`, restart the app:

  ```bash
  docker compose restart app
  ```

- If the listener is alive but the agent is already in `awake` status, the
  drain check (AC-9) will pick up the new message after the current wake
  finishes.

### 200 duplicate on every call

- The caller is re-sending the same `x-idempotency-key`. Either:
  - Ask them to use a unique key per event (e.g. the upstream event id), or
  - Allow retries by having them omit the header when they genuinely want
    to re-deliver.

## Escalation

- If signature failures come from the expected caller IP suddenly after a
  long period of success, suspect secret rotation or a proxy stripping the
  header. Check the reverse proxy config before rotating the secret.
- If webhook ingress is overwhelming the runtime (see
  [`rate-limit-tuning.md`](./rate-limit-tuning.md)), disable the specific
  agent until the integration is fixed rather than relaxing the global rate
  limits.
- Persistent delivery failures from a critical integration should page on-call
  and be treated as an incident.
