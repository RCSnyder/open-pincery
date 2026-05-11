# Onboarding — Open Pincery

> **Audience**: a single operator standing up their first Open Pincery
> instance on their own machine. From `git clone` to a message
> answered by an agent in under 15 minutes.
>
> **AC-92**: this page is the one source of truth for first-run setup.
> If anything below is wrong, that is a v9.1 bug — open an issue.

## 1. Prerequisites

| What                        | Why                                                            | How to check                       |
| --------------------------- | -------------------------------------------------------------- | ---------------------------------- |
| Linux 6.7+ kernel           | Landlock ABI 6, cgroup v2, unprivileged userns                 | `uname -r`                         |
| `bubblewrap` ≥ 0.8          | Sandboxed tool execution                                       | `bwrap --version`                  |
| Docker 24+                  | Postgres + optional Caddy reverse proxy                        | `docker version`                   |
| Rust toolchain ≥ 1.88       | Building the binary (or use the prebuilt release tarball)      | `rustc --version`                  |
| 4 GB free RAM + 2 GB disk   | Postgres + the binary + a small event log                      | —                                  |

If you are on macOS or Windows, install the Linux devshell (see
`scripts/devshell.sh`). The native sandbox surface (landlock + seccomp)
is **Linux-only**; `pcy doctor` will emit a `WARN: native sandbox
unavailable, use devshell` row but the rest of the system runs.

## 2. Five commands

These five commands take you from an empty directory to a running
instance answering a message. Run them in order.

```sh
# 1. Generate strong random secrets into .env
pcy init

# 2. Bring up Postgres (and optionally Caddy)
docker compose up -d

# 3. Start the server in another terminal
cargo run --release --bin pincery-server
#    (or, from a release: ./pincery-server)

# 4. Verify everything looks healthy
pcy doctor

# 5. Sign in as the bootstrap admin and send a first message
pcy login
pcy message ask "Summarise the changelog so far."
```

After step 1 you will have a `.env` with a 64-character hex bootstrap
token, a 44-character base64 vault key, and (if you provided one) an
`LLM_API_KEY` line. The file is created with mode `0600` on Unix; on
Windows the default ACL is used (best-effort — see your filesystem's
inheritance rules).

## 3. Doctor check

`pcy doctor` runs seven ordered checks and prints `OK | WARN | FAIL`
plus a one-line remediation for each. Re-run it after every change to
`.env` or after a server restart. (An eighth sandbox-preflight check
is planned for v9.2 — see scope AC-90b.)

```sh
pcy doctor              # human-readable table
pcy doctor --output json  # machine-readable
pcy doctor --strict     # promote WARN to a non-zero exit code,
                        # except for kernel-floor WARN on non-Linux
                        # hosts (see CR-v91-3)
```

Typical good run on a fresh install:

```
STATUS  CHECK             DETAIL
------  ----------------  ----------------------------------------
OK      .env file         .env present in cwd
OK      docker            docker reachable: 27.0.0
OK      kernel floor      landlock ABI 6, all checks passed
OK      database          SELECT 1 ok
OK      migrations        30/30 applied
OK      bootstrap         1 admin user(s) present
OK      llm               provider responded 200
```

A single `FAIL` exits non-zero; a `WARN` exits zero by default.

## 4. Add your first credential

The LLM API key you typed into `pcy init` lives in your `.env` only as
a one-shot bootstrap value. For day-two operations, store keys inside
the encrypted credential vault so the server can rotate, audit, and
revoke them without an environment-variable bounce.

```sh
pcy credential add --name openrouter-prod
# (prompts for the secret — never echoed to stdout)

pcy credential list
```

Credentials are AES-GCM encrypted at rest with the vault key from
`.env`. Loss of `VAULT_KEY` makes them unrecoverable — see Section 6
on backups.

## 5. Send your first message

```sh
pcy agent create --name scratchpad
pcy message ask --agent scratchpad "Hello, who is on the other end?"
pcy events tail --agent scratchpad
```

`pcy events tail` is the operator-facing window into the hash-chained
event log. Every prompt, response, sandboxed tool call, and admin
action is appended there.

## 6. Backup before trust

Before you point real work at your instance, confirm that you can
restore it. The backup pipeline (AC-91) ships in this same v9.1
release. A round-trip takes about 30 seconds for a clean install:

```bash
pcy backup --file pcy.bak.tar.gz
# optional: bundle the vault envelope for an air-gapped restore
pcy backup --file pcy.bak.tar.gz --include-vault-key
pcy restore --input pcy.bak.tar.gz
pcy doctor --strict
```

If `pcy doctor` fails on a freshly-restored backup, **do not trust the
instance with real data**. File the issue and re-run the round-trip
after the fix.

## 7. Where next

- **Add a second LLM provider** — `pcy provider add` registers an
  OpenAI-compatible base URL paired with a stored credential so the
  wake loop talks to the right vendor per workspace, no `.env` edits
  required:

  ```bash
  pcy credential add openrouter
  pcy provider add openrouter \
      --base-url https://openrouter.ai/api/v1 \
      --credential openrouter
  pcy provider list
  pcy provider use openrouter
  ```
- **Reverse proxy** — see `Caddyfile.example` and
  `docker-compose.caddy.yml` for a TLS-terminating front door.
- **Audit verification** — `pcy audit verify` walks the per-agent
  event-log hash chain and flags any tampering.
- **Runbooks** — `docs/runbooks/` covers incident response, key
  rotation, and disaster recovery in depth.

If you got this far without surprises, the onboarding gate works.
Welcome.
