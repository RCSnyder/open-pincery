# Database Restore

## Symptom

The PostgreSQL instance backing Open Pincery is corrupted, has been lost, or
was rolled back by accident. `/ready` returns `503 {"failing":"database"}` and
the application logs show connection failures or missing tables.

Open Pincery is event-sourced: the events table is the source of truth for
agent behaviour. A restore must preserve event ordering for every `agent_id`.

## Diagnostic Commands

```bash
# 1. Confirm which instance is down (local Docker compose vs managed).
docker compose ps db

# 2. Check whether the database exists at all.
psql "$DATABASE_URL" -c "SELECT 1" || echo "DB unreachable"

# 3. List available backups (convention: /var/backups/open-pincery/*.dump).
ls -lh /var/backups/open-pincery/ | sort -r | head -10

# 4. Inspect the newest backup's header to confirm it is valid.
pg_restore --list /var/backups/open-pincery/latest.dump | head

# 5. Verify the running binary's migration set matches the backup schema.
psql "$DATABASE_URL" -c "SELECT version FROM _sqlx_migrations ORDER BY version" || true
```

## Remediation

Pick exactly one of these paths. Do not mix.

### Path A — restore from `pg_dump` backup

```bash
# 1. Stop Open Pincery so nothing writes while we restore.
docker compose stop app  # or: systemctl stop open-pincery

# 2. Drop and recreate the target database.
psql "$ADMIN_DATABASE_URL" -c "DROP DATABASE IF EXISTS open_pincery;"
psql "$ADMIN_DATABASE_URL" -c "CREATE DATABASE open_pincery OWNER open_pincery;"

# 3. Restore.
pg_restore --no-owner --no-acl \
  --dbname="$DATABASE_URL" \
  /var/backups/open-pincery/latest.dump

# 4. Start the runtime — migrations auto-apply on startup, so a
#    backup older than the binary is brought forward automatically.
#    Watch the logs for "Migrations complete" to confirm.
docker compose start app
docker compose logs --tail=50 app | grep -E "Migrations complete|migrate"

# 5. Verify readiness.
curl -fsS "$APP_URL/ready"
```

### Path B — rebuild from a fresh database

Only acceptable when no backup exists and loss of event history is understood
and accepted.

```bash
docker compose down -v
docker compose up -d db
docker compose up -d app
```

This creates a new bootstrap admin token — see `scaffolding/scope.md` for the
bootstrap flow.

## Escalation

- If Path A fails with "relation does not exist" errors, the backup schema is
  newer than the binary. Upgrade the binary first, then retry.
- If the backup itself is corrupt, escalate to whoever owns the backup system
  before touching the remaining database. Do not `DROP DATABASE` until you
  have at least one verified artefact.
- If restoration takes longer than 10 minutes, announce the outage to users
  and post the status in the team channel.
