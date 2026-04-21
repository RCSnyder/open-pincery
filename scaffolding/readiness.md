# Readiness: Open Pincery — v7 (Credential Vault & Reasoner-Secret Refusal)

> This file supersedes the prior v6 readiness record. v6 is shipped; its
> readiness artifact lives in git history (latest commit on the v6 branch
> before the v7 EXPAND commit `a532996`). v7 covers AC-38 through AC-43
> only — AC-1..AC-37 coverage is verified by the shipped v6 suite and is
> not re-planned here.

## Verdict

READY

v7 is strictly additive: a new AES-256-GCM credential vault module, a new
operator-only REST surface, a new CLI command group that never accepts a
secret via argv, a new `list_credentials` tool gated as `ReadLocal`, a
new prompt-template version with explicit vault redirect, and a new
`PLACEHOLDER:<name>` dispatch handshake that reserves the v9 proxy seam.
No existing AC regresses; no existing row is mutated (two additive
migrations). Every AC has unambiguous pass/fail criteria, a named test
file, and a concrete runtime proof path. Scope adjustments documented in
design.md are bounded and preserve every AC's core invariant.

## Truths

Non-negotiable statements that must be true in the shipped v7 system:

- **T-v7-1** `src/runtime/vault.rs` defines `pub struct Vault` with three
  methods: `from_base64(&str) -> Result<Vault, VaultError>`,
  `seal(&self, workspace_id: Uuid, name: &str, plaintext: &[u8]) -> SealedCredential`,
  and `open(&self, workspace_id: Uuid, name: &str, sealed: &SealedCredential) -> Result<Vec<u8>, VaultError>`.
- **T-v7-2** `Vault::seal` uses AES-256-GCM with a fresh 12-byte
  `OsRng`-sourced nonce per call and AAD bytes
  `format!("{workspace_id}:{name}").into_bytes()`.
- **T-v7-3** `Vault::open` returns `VaultError::Authentication` (never
  panics) on any of: tampered ciphertext, tampered nonce, mismatched
  `(workspace_id, name)`, wrong master key.
- **T-v7-4** Master key is loaded exactly once at startup from
  `OPEN_PINCERY_VAULT_KEY` (base64 → 32 bytes). Missing/malformed key
  fails the process with an actionable error before any HTTP listener
  binds.
- **T-v7-5** Migration `20260420000002_create_credentials.sql` creates
  the `credentials` table with columns
  `(id, workspace_id, name, ciphertext, nonce, created_by, created_at, revoked_at)`,
  `CHECK (length(nonce) = 12)`, `CHECK (length(ciphertext) >= 16)`,
  `CHECK (name ~ '^[a-z0-9_]{1,64}$')`, and a unique partial index on
  `(workspace_id, name) WHERE revoked_at IS NULL`.
- **T-v7-6** `POST /api/workspaces/:id/credentials`,
  `GET /api/workspaces/:id/credentials`, and
  `DELETE /api/workspaces/:id/credentials/:name` require
  `workspace_admin` on the target workspace (or `local_admin`);
  non-admin members receive 403 and an `auth_forbidden`-equivalent
  audit row; non-members receive 404.
- **T-v7-7** `GET /api/workspaces/:id/credentials` response body is a
  JSON array of `{name, created_at, created_by}` only. The response
  never contains `value`, `ciphertext`, `nonce`, or any other byte that
  could reconstruct the sealed secret.
- **T-v7-8** `POST` on a duplicate non-revoked `(workspace_id, name)`
  returns 409 Conflict. `DELETE` sets `revoked_at = NOW()` and a
  subsequent `POST` with the same name succeeds.
- **T-v7-9** `credential_added` / `credential_revoked` / `credential_forbidden`
  rows are appended to `auth_audit` with
  `details JSONB = {workspace_id, name, actor_user_id}`. Value bytes
  never appear in any audit row.
- **T-v7-10** `pcy credential add <name>` has no `--value` clap argument.
  `Cli::try_parse_from(["pcy","credential","add","foo","--value","bar"])`
  returns a clap error.
- **T-v7-11** `pcy credential add` in non-`--stdin` mode calls
  `rpassword::prompt_password(...)` (exactly one call site in
  `src/cli/commands/credential.rs`). In `--stdin` mode it reads from
  stdin and trims trailing newline.
- **T-v7-12** `pcy credential list` prints a two-column `NAME CREATED_AT`
  table populated from the `GET` response. It never prints a value.
  `pcy credential revoke <name>` prompts for confirmation unless `--yes`.
- **T-v7-13** `list_credentials` is registered in `tool_definitions()`
  with an empty `parameters` object. `required_for("list_credentials")`
  maps to `ToolCapability::ReadLocal`, so every `PermissionMode` allows it.
- **T-v7-14** Dispatching `list_credentials` returns a
  `ToolResult::Output` whose body is a JSON array of
  `{name, created_at}` scoped to the calling agent's `workspace_id`,
  filtered `revoked_at IS NULL`. Values / ciphertext / nonces are
  absent. Cross-workspace agents see `[]`.
- **T-v7-15** Migration `20260420000003_prompt_template_credentials.sql`
  sets `wake_system_prompt` v1 to `is_active = FALSE` and inserts a v2
  row with `is_active = TRUE` in a single transaction. v2 template text
  contains literal substrings `pcy credential add`, `REFUSE`, and
  `POST /api/workspaces/:id/credentials`.
- **T-v7-16** The one-active-per-name partial unique index is respected
  by the migration: after it runs, exactly one row with
  `name = 'wake_system_prompt'` has `is_active = TRUE`, and it is v2.
- **T-v7-17** `ShellArgs` has an optional `env: HashMap<String, String>`
  (`#[serde(default)]`). The `shell` tool's JSON-Schema `parameters`
  now declares `env` as an optional object-of-string property.
- **T-v7-18** `tools::dispatch_tool` signature is
  `(tool_call, mode, pool, workspace_id, agent_id, wake_id, executor)`.
  `wake_loop::run_wake_loop` reads `agent.workspace_id` and threads it
  per dispatch.
- **T-v7-19** Before invoking the executor for a `shell` call,
  `dispatch_tool` scans the parsed `env` map. For every value starting
  with literal `PLACEHOLDER:`, it looks up the suffix name in
  `credentials` with `revoked_at IS NULL`. On miss/revoked it appends
  exactly one `credential_unresolved` event
  (`event_type = "credential_unresolved"`, `source = "runtime"`,
  `tool_name = "shell"`, `tool_input` JSON
  `{tool_name, credential_name, reason}`), returns
  `ToolResult::Error(format!("credential not found: {name}"))`, and
  never invokes the executor.
- **T-v7-20** On a placeholder hit, the env value passes through
  unchanged to `ProcessExecutor::run`. v7 performs no substitution; the
  child process observes the literal `PLACEHOLDER:<name>` string. This
  is the seam v9 will fill.
- **T-v7-21** No existing v1–v6 AC regresses: CAS lifecycle (AC-1),
  event log (AC-2), prompt assembly (AC-3) — assembly continues to pass
  against the new active template row, wake loop (AC-4), maintenance
  (AC-5), HTTP API (AC-6), wake triggers (AC-7), stale recovery (AC-8),
  drain (AC-9), bootstrap (AC-10), and every v2..v6 AC are unchanged.
- **T-v7-22** `cargo deny check advisories` continues to exit 0 on v7
  HEAD (AC-37 floor preserved). New dependencies (`aes-gcm`,
  `rpassword`) have no known high/critical advisories as of v7 BUILD.

## Key Links

- **AC-38** → scope.md v7 AC-38 → design.md v7 Vault interface →
  `src/runtime/vault.rs` + `migrations/20260420000002_create_credentials.sql` +
  `src/config.rs` (vault key load) + `src/main.rs` (startup failure
  path) → `tests/vault_roundtrip_test.rs` → runtime proof: 100 sealed
  round-trips with distinct nonces; tamper tests return
  `VaultError::Authentication`.
- **AC-39** → scope.md v7 AC-39 → design.md v7 credentials router →
  `src/api/credentials.rs` + `src/api/mod.rs` (workspace_admin helper) +
  `src/models/credential.rs` → `tests/vault_api_test.rs` → runtime
  proof: admin POST/GET/DELETE succeed; non-admin 403; list response
  JSON scan finds zero secret-value bytes; duplicate active name 409;
  revoke-then-readd succeeds.
- **AC-40** → scope.md v7 AC-40 → design.md v7 CLI interface →
  `src/cli/commands/credential.rs` + `src/cli/mod.rs` + `src/api_client.rs`
  → `tests/cli_credential_test.rs` → runtime proof: clap rejects
  `--value`; stdin round-trip succeeds against an `ApiClient`; static
  grep confirms exactly one `rpassword::prompt_password` call site.
- **AC-41** → scope.md v7 AC-41 → design.md v7 list_credentials →
  `src/runtime/tools.rs` (tool def + dispatch arm) +
  `src/runtime/capability.rs` (ReadLocal classification) →
  `tests/list_credentials_tool_test.rs` → runtime proof: workspace-A
  agent sees 2 non-revoked names; workspace-B agent sees `[]`; response
  bytes contain zero occurrences of any stored value.
- **AC-42** → scope.md v7 AC-42 → design.md v7 prompt template →
  `migrations/20260420000003_prompt_template_credentials.sql` →
  `tests/reasoner_refusal_test.rs` → runtime proof: active row is v2;
  template text contains all three required substrings; v1 row still
  exists with `is_active = FALSE`; AC-3 prompt assembly continues to
  pass.
- **AC-43** → scope.md v7 AC-43 → design.md v7 placeholder envelope →
  `src/runtime/tools.rs` (scan + unresolved event) +
  `src/runtime/sandbox.rs` (env pass-through) + `src/runtime/wake_loop.rs`
  (workspace_id thread) → `tests/placeholder_envelope_test.rs` →
  runtime proof: miss → `credential_unresolved` + zero spawns; hit →
  dispatch proceeds, child env contains literal `PLACEHOLDER:<name>`;
  revoke-then-redispatch → `credential_unresolved`.

## Acceptance Criteria Coverage

| AC    | Planned test                           | Planned runtime proof                                                                                                                                  |
| ----- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| AC-38 | `tests/vault_roundtrip_test.rs`        | 100 seal/open round-trips with distinct nonces; tamper on ciphertext/nonce/name/workspace/key → `VaultError::Authentication`, no panic                 |
| AC-39 | `tests/vault_api_test.rs`              | admin POST/GET/DELETE green-paths; non-admin 403; list JSON byte-scan finds zero secret-value matches; 409 on duplicate; revoke-then-readd             |
| AC-40 | `tests/cli_credential_test.rs`         | `Cli::try_parse_from([..."--value","bar"])` returns clap error; `--stdin` round-trip succeeds against an `ApiClient`; grep finds one `rpassword` site  |
| AC-41 | `tests/list_credentials_tool_test.rs`  | dispatch returns 2 summaries for WS-A (1 revoked filtered), `[]` for WS-B; output bytes contain zero occurrences of any stored value                   |
| AC-42 | `tests/reasoner_refusal_test.rs`       | active `wake_system_prompt` row = v2; template contains `pcy credential add`, `REFUSE`, `POST /api/workspaces/:id/credentials`; v1 preserved inactive  |
| AC-43 | `tests/placeholder_envelope_test.rs`   | missing → `credential_unresolved` + zero spawns; hit → child env contains literal `PLACEHOLDER:<name>`; revoked → `credential_unresolved`              |

## Scope Reduction Risks

- **AC-38 — Vault falls back to a fixed test key in production**:
  Tempting to hardcode a dev key for ergonomics. `main.rs` must fail the
  process on missing/malformed `OPEN_PINCERY_VAULT_KEY` before binding
  HTTP. Lockdown test: a CLI/integration run without the env var fails
  with a specific, actionable error message.
- **AC-38 — AAD dropped "for performance"**: Scope locks AAD =
  `{workspace_id}:{name}`. Without AAD, a sealed row could be swapped
  across names or workspaces without detection.
- **AC-38 — 12-byte nonce reused across seals**: OsRng nonce per seal
  is mandatory. The 100-iteration test asserts unique nonces; a constant
  or counter-based nonce would fail it.
- **AC-39 — List endpoint leaks ciphertext "for debugging"**: Response
  schema is frozen at `{name, created_at, created_by}`. The byte-scan
  test asserts zero occurrences of the stored value in the response.
- **AC-39 — Role gate weakened to "any workspace member"**: Scope locks
  `workspace_admin` (or `local_admin`). Non-admin members must receive
  403 with an audit row.
- **AC-39 — Duplicate name silently upserts**: Scope locks 409 on an
  active duplicate. An upsert path would let a compromised session
  silently replace a secret.
- **AC-40 — `--value` argv flag added "for scripting"**: Scope locks
  stdin/TTY-only input. The argv-rejection test asserts the clap shape.
- **AC-40 — `rpassword` replaced with a raw `readline`**: Raw readline
  echoes the secret to the terminal. Scope locks
  `rpassword::prompt_password` in the interactive branch.
- **AC-41 — Tool returns values "for agent convenience"**: Scope locks
  names-only. The payload byte-scan test asserts zero occurrences of
  any stored value.
- **AC-41 — Cross-workspace leakage**: Tempting to use a looser query
  that joins on `created_by` or similar. Scope locks
  `WHERE workspace_id = $1` exactly; cross-workspace agents must see
  `[]`.
- **AC-42 — Prompt v1 mutated in place instead of versioned**: Scope
  locks immutability. v1 row stays; v2 is a new row with `is_active=TRUE`.
- **AC-42 — "Credential Handling" section omits the redirect**: Scope
  locks the literal `pcy credential add` substring in the template
  text. The test fails closed if any of the three required substrings
  is missing.
- **AC-43 — Silent hit without audit**: On miss/revoked, exactly one
  `credential_unresolved` event is appended. No silent error.
- **AC-43 — v7 attempts real substitution**: Scope locks "no
  substitution in v7". The child-env-contains-`PLACEHOLDER:` assertion
  fails if anything else is passed. Real substitution is v9's job.

## Clarifications Needed

None with BUILD impact. Two design-resolved choices (documented under
`design.md` "Scope Adjustments"):

1. `credential_unresolved` `reason` is unified to `"missing_or_revoked"`
   — single query, bounded test relaxation.
2. Workspace-level audit rows land in `auth_audit` (already exists since
   v2), not in `events` (which is `agent_id`-scoped).

## Build Order

Each slice is sized to ship as 1–2 commits. Independent within reason;
later slices depend only on earlier ones' exported types.

1. **Slice 1 — AC-38 Vault module + migration.** Add `aes-gcm = "0.10"`
   to `Cargo.toml`. Create `src/runtime/vault.rs` with `Vault`,
   `SealedCredential`, `VaultError`, `from_base64`, `seal`, `open`.
   Add migration `20260420000002_create_credentials.sql`. Add
   `vault_key: [u8; 32]` to `Config` and load from
   `OPEN_PINCERY_VAULT_KEY`. Write `tests/vault_roundtrip_test.rs`.
   Update `.env.example` and `docker-compose.yml` to forward the new
   env var.
2. **Slice 2 — AC-39 Credentials API + model.** Create
   `src/models/credential.rs` with `Credential` struct and
   `create`/`list_active`/`find_active`/`revoke` helpers. Create
   `src/api/credentials.rs` with the three handlers. Add
   `require_workspace_admin` helper to `src/api/mod.rs`. Mount the
   router. Write `tests/vault_api_test.rs`. Construct a single `Vault`
   instance in `main.rs` and thread it into `AppState`.
3. **Slice 3 — AC-40 CLI command group.** Add `rpassword = "7"`. Create
   `src/cli/commands/credential.rs`. Add `Credential` subcommand to
   `src/cli/mod.rs`. Extend `src/api_client.rs` with
   `create_credential`/`list_credentials`/`revoke_credential`. Write
   `tests/cli_credential_test.rs`.
4. **Slice 4 — AC-41 `list_credentials` tool.** Add classification arm
   to `src/runtime/capability.rs` (extend `required_for`). Add tool
   definition + dispatch arm in `src/runtime/tools.rs`. Thread
   `workspace_id` through `dispatch_tool` signature + `run_wake_loop`
   (this change is shared with AC-43). Write
   `tests/list_credentials_tool_test.rs`. Update the existing
   `tests/capability_gate_test.rs` unit test to cover the new row.
5. **Slice 5 — AC-42 prompt template v2.** Create migration
   `20260420000003_prompt_template_credentials.sql` that deactivates
   v1 and inserts v2 in a single transaction. Write
   `tests/reasoner_refusal_test.rs`.
6. **Slice 6 — AC-43 placeholder envelope.** Extend `ShellArgs` with
   `env`. Extend `ShellCommand` / `SandboxProfile` to pass env through
   to `ProcessExecutor`. Implement the pre-spawn placeholder scan in
   `dispatch_tool` using `credential::find_active`. Write
   `tests/placeholder_envelope_test.rs`.

After Slice 6: `cargo test --all-targets -- --test-threads=1` + `cargo
clippy --all-targets -- -D warnings` + `cargo fmt --all -- --check` +
`cargo deny check advisories` all pass. Then REVIEW.

## Complexity Exceptions

None. File budgets tracked in `design.md` v7 addendum
("Complexity Exceptions" subsection).
