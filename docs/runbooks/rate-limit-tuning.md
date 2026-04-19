# Rate Limit Tuning

## Symptom

One of two patterns:

1. **Legitimate users are hitting 429 Too Many Requests.** The Prometheus
   counter `open_pincery_rate_limit_rejected_total` climbs while real clients
   complain that bootstrap, login, or webhook calls fail with `Retry-After: 60`.
2. **Rate limits are too permissive** and abusive traffic (for example, a
   misbehaving integration) is overwhelming the runtime.

The two rate limiters are hard-coded in [`src/api/mod.rs`](../../src/api/mod.rs):

- `unauth_limiter`: 10 requests per IP per minute (covers bootstrap + webhooks)
- `auth_limiter`: 60 requests per IP per minute (covers the `/api/*` routes)

## Diagnostic Commands

```bash
# 1. Confirm the current rejection rate.
curl -s "$METRICS_ADDR/metrics" | grep open_pincery_rate_limit_rejected_total

# 2. Watch it for a minute to see if it is growing.
for i in 1 2 3 4 5 6; do
  curl -s "$METRICS_ADDR/metrics" | awk '/open_pincery_rate_limit_rejected_total/ {print strftime("%T"), $2}'
  sleep 10
done

# 3. Identify which IP(s) are getting rejected by looking at the access logs
#    (structured JSON if LOG_FORMAT=json is set).
docker compose logs --since 15m app | \
  grep -E '"status":429' | \
  jq -r '.client_ip' 2>/dev/null | sort | uniq -c | sort -rn | head

# 4. If you suspect a specific webhook source, capture a sample request.
docker compose logs --since 15m app | grep -E '"path":"/api/agents/[^/]+/webhooks"'
```

## Remediation

### Raising or lowering the limits

The limits live in `AppState::new()`. To change them, edit the two
`Quota::per_minute(NonZeroU32::new(N).unwrap())` calls and ship a new build.

```rust
let unauth_limiter = Arc::new(RateLimiter::keyed(
    Quota::per_minute(NonZeroU32::new(30).unwrap()), // was 10
));
let auth_limiter = Arc::new(RateLimiter::keyed(
    Quota::per_minute(NonZeroU32::new(120).unwrap()), // was 60
));
```

```bash
# Rebuild and redeploy.
cargo build --release
docker compose up -d --build app

# Confirm new limit applied by watching the rejection counter stop climbing.
curl -s "$METRICS_ADDR/metrics" | grep open_pincery_rate_limit_rejected_total
```

### Short-term relief without a rebuild

If you cannot rebuild immediately, restarting the process clears the in-memory
rate-limit buckets. This does not raise the ceiling — it only resets the
current window.

```bash
docker compose restart app
```

### Blocking abusive IPs

Open Pincery does not currently support IP allow/deny lists in code. Push IP
blocks up to the reverse proxy or firewall:

```bash
# Example with iptables; adapt to your environment.
sudo iptables -I INPUT -s 203.0.113.7 -p tcp --dport 8080 -j DROP
```

## Escalation

- If 429s are concentrated on a single tenant's webhooks, disable that agent's
  ingress until the upstream behaves:

  ```bash
  psql "$DATABASE_URL" -c "
    UPDATE agents SET is_enabled = false, disabled_reason = 'rate_limit_abuse'
    WHERE id = '$AGENT';
  "
  ```

- If rejections are across many IPs simultaneously, treat as a potential DoS:
  page on-call, enable upstream caching/CDN if available, and tighten the
  firewall before relaxing limits.
- If users report inability to bootstrap an environment that is otherwise
  idle, the 10/min unauth limit is probably too low for your setup. Raise it
  permanently rather than fighting the symptoms.
