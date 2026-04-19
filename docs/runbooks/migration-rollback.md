# Migration Rollback

## Symptom

A new migration deployed alongside a binary upgrade has caused problems:
`/ready` returns `503`, application logs show `missing column` or
`relation does not exist`, or existing queries have begun failing in ways
that map directly to a schema change.

Open Pincery uses `sqlx migrate` with forward-only migrations in
[`migrations/`](../../migrations/). There is no built-in `DOWN` step — rollback
is done by shipping a new forward migration (or by restoring from backup).

## Diagnostic Commands

```bash
# 1. List applied migrations with timestamps.
psql "$DATABASE_URL" -c "
  SELECT version, description, installed_on, success
  FROM _sqlx_migrations
  ORDER BY version DESC
  LIMIT 10;
"

# 2. Show the most recent migration's SQL (replace $TS with the version
#    returned above).
ls migrations/ | grep "^${TS}" || true
cat migrations/${TS}_*.sql

# 3. Compare running schema to what the binary expects.
psql "$DATABASE_URL" -c "\dt"
psql "$DATABASE_URL" -c "\d+ agents"

# 4. Look for the specific error in the app logs.
docker compose logs --since 10m app | grep -iE 'relation|column|migration'
```

## Remediation

Pick exactly one path. Option A is always preferred.

### Path A — revert the binary to the previous version

If the failing migration was introduced by the new binary and the previous
binary still runs against the _pre-migration_ schema, revert first and
investigate later.

```bash
# 1. Stop the new version.
docker compose stop app

# 2. Redeploy the previous tag.
git checkout $PREVIOUS_TAG
docker compose build app
docker compose up -d app

# 3. If the migration has already been partially applied, run a cleanup
#    migration (see Path B).
```

### Path B — ship a compensating forward migration

When you cannot revert the binary (e.g. the previous version had a critical
bug), write a new migration that undoes the damage of the bad one.

```bash
# 1. Create a new timestamped migration file.
TS=$(date -u +%Y%m%d%H%M%S)
cat > migrations/${TS}_revert_bad_change.sql <<'SQL'
-- Compensating migration for $BAD_MIGRATION.
-- Undo the broken schema change here.
ALTER TABLE agents ADD COLUMN IF NOT EXISTS old_column TEXT;
-- ... or whatever reversal is required.
SQL

# 2. Commit, build, deploy.
git add migrations/${TS}_*.sql
git commit -m "fix(db): compensate for $BAD_MIGRATION"
docker compose up -d --build app

# 3. Verify the compensating migration applied.
psql "$DATABASE_URL" -c "
  SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 3;
"
```

### Path C — restore from backup

Use only when A and B are both impossible. Follow
[`db-restore.md`](./db-restore.md) Path A, pointing at a backup taken before
the bad migration.

## Escalation

- If the bad migration deleted or dropped columns that contained data, Path B
  cannot restore that data. Jump to Path C.
- If Path C is also impossible (no backup), open an incident and announce
  the data loss honestly to affected users.
- Never hand-edit `_sqlx_migrations` rows to "unstick" a migration without
  simultaneously reverting the schema change. That creates divergence between
  what the binary expects and what the database has.
