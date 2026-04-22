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

| AC    | Planned test                          | Planned runtime proof                                                                                                                                 |
| ----- | ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC-38 | `tests/vault_roundtrip_test.rs`       | 100 seal/open round-trips with distinct nonces; tamper on ciphertext/nonce/name/workspace/key → `VaultError::Authentication`, no panic                |
| AC-39 | `tests/vault_api_test.rs`             | admin POST/GET/DELETE green-paths; non-admin 403; list JSON byte-scan finds zero secret-value matches; 409 on duplicate; revoke-then-readd            |
| AC-40 | `tests/cli_credential_test.rs`        | `Cli::try_parse_from([..."--value","bar"])` returns clap error; `--stdin` round-trip succeeds against an `ApiClient`; grep finds one `rpassword` site |
| AC-41 | `tests/list_credentials_tool_test.rs` | dispatch returns 2 summaries for WS-A (1 revoked filtered), `[]` for WS-B; output bytes contain zero occurrences of any stored value                  |
| AC-42 | `tests/reasoner_refusal_test.rs`      | active `wake_system_prompt` row = v2; template contains `pcy credential add`, `REFUSE`, `POST /api/workspaces/:id/credentials`; v1 preserved inactive |
| AC-43 | `tests/placeholder_envelope_test.rs`  | missing → `credential_unresolved` + zero spawns; hit → child env contains literal `PLACEHOLDER:<name>`; revoked → `credential_unresolved`             |

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

---

## v8 Readiness Addendum — Unified API Surface

> v7 is shipped; AC-38..AC-43 coverage is locked by the v7 suite and not
> re-planned here. v8 covers AC-44 through AC-52 only. v8 is
> surface-only: no schema changes, no runtime-semantic changes to any
> existing handler, no change to the authenticated contract shape.
> Every v1–v7 AC must still pass unchanged after v8 BUILD; regressing
> an older AC is a v8 blocker.

### Verdict

READY

Every AC-44..AC-52 has a named design component, a named test file, a
concrete runtime proof path, and an unambiguous pass/fail assertion.
The four design-time scope adjustments (kubectl JSONPath subset, pinned
MCP `2025-06-18`, Windows-via-WSL for `install.sh`, PUT-ban as lint not
arch) sharpen AC semantics without softening any invariant. No
outstanding clarification would change the pass/fail meaning of any
AC. BUILD may begin.

### Truths

Non-negotiable statements that must be true in the shipped v8 system:

- **T-v8-1** `src/api/openapi.rs` defines a single
  `#[derive(utoipa::OpenApi)] pub struct ApiDoc` whose `paths(...)`
  list contains **every** route registered by `api::router()` plus the
  unauth routes (`/api/bootstrap`, `/api/webhooks/*`). `AC-44` lint
  fails closed if the two enumerations diverge.
- **T-v8-2** `GET /openapi.json` returns a JSON body that parses as
  `openapiv3::OpenAPI` with `openapi == "3.1.0"`, shares the `/health`
  rate-limit bucket, is unauthenticated, and sets
  `Content-Type: application/json`. `GET /openapi.yaml` returns the
  YAML serialization with `Content-Type: application/yaml`.
- **T-v8-3** `pcy login` is idempotent: on a fresh server with
  `OPEN_PINCERY_BOOTSTRAP_TOKEN` set it calls `POST /api/bootstrap`;
  on an already-bootstrapped server it calls `POST /api/login`; on
  either path exit is 0 and stdout is exactly one line matching
  `^Logged in to <context> as <email>$`. **Re-running `pcy login`
  against an already-bootstrapped server never surfaces a `409`.**
- **T-v8-4** The clap root `Cli` exposes the v8 nouns
  (`agent credential budget event context auth api completion mcp
whoami login`) plus hidden shim variants (`bootstrap message events`)
  that emit exactly one `warning:` stderr line via
  `nouns::warn_deprecated` and delegate to the new verb. `--help`
  output lists the new tree; `--help --all` (or equivalent) surfaces
  the hidden shims.
- **T-v8-5** Every verb accepting an agent/credential/budget/event
  target resolves via `src/cli/resolve.rs`: valid UUID → single GET
  confirmation; non-UUID → LIST filtered by exact `name` equality;
  multiple matches → exit 2 with a two-column `ID  NAME` table on
  stderr; zero matches → exit 1 with `not found: <needle>` on stderr.
  **Name matching is never a substring or prefix match.**
- **T-v8-6** Every command that prints structured data accepts
  `--output {table|json|yaml|jsonpath=<expr>|name}`. Default is
  `table` when `io::stdout().is_terminal()`, `json` otherwise.
  `NO_COLOR` suppresses ANSI from `table`. `jsonpath=<expr>` evaluates
  through `jsonpath-rust` covering the kubectl subset
  (`.foo.bar`, `.items[*].name`, `.items[0]`, `[?(@.k==v)]`). `name`
  emits one name per line. `--format` is accepted for one release as a
  deprecated alias that warns once.
- **T-v8-7** `~/.config/open-pincery/config.toml` with v4 flat schema
  is auto-migrated on first v8 load: `src/cli/migrate.rs` writes a
  backup at `config.toml.pre-v8` then rewrites to
  `current-context = "default"` + `[contexts.default]` preserving
  `url`/`token`/`workspace_id`/`user_id`. Migration is idempotent
  (second load is a no-op). Context precedence is `--context` flag >
  `OPEN_PINCERY_CONTEXT` env > file `current-context`.
- **T-v8-8** `pcy mcp serve` is a stdio JSON-RPC server speaking MCP
  revision `2025-06-18` with newline-delimited framing. `tools/list`
  returns one tool per `ApiDoc::openapi()` operation named
  `<tag>.<operation>` (e.g. `agent.create`, `credential.list`) with
  `description` from the operation summary and `inputSchema` from the
  request body + path/query parameters. **The tool list is derived
  from `ApiDoc`, never hard-coded.** `tools/call` proxies through
  `ApiClient` using the active context's token; HTTP failures map to
  the fixed error-code table (`-32001` unreachable, `-32002`
  unauthorized, `-32003` rate-limited, `-32004` not-found, `-32000`
  generic). stdout carries only JSON-RPC; debug and framing errors go
  to stderr.
- **T-v8-9** `install.sh` at the repo root, when piped to `bash`,
  detects OS+arch via `uname`, resolves the release tag, downloads the
  matching asset and its `.sha256`, **enforces sha256**, and attempts
  cosign verification. Without `cosign` on `PATH` it prints a
  `warning:` line and proceeds; with `--require-cosign` it exits
  non-zero. `shellcheck -S warning` is clean. sha256 mismatch **always**
  exits non-zero and refuses to install the asset.
- **T-v8-10** `pcy completion {bash|zsh|fish|powershell}` emits a
  non-empty completion script via `clap_complete` containing a
  shell-specific marker (`_pcy`/`#compdef`/`complete -c pcy`/
  `Register-ArgumentCompleter`). README documents the one-line install
  for each shell.
- **T-v8-11** `tests/api_naming_test.rs` walks `ApiDoc::openapi()` at
  test time and asserts: every collection path segment is plural; every
  primary-key path parameter is named `{id}` (explicit allowlist for
  compound keys); every operation has a non-empty summary ≤ 72 chars
  ending without a period; no `PUT` method appears (allowlist is
  empty at v8 ship); no schema uses `format: "uuid-v7"` (only
  `format: "uuid"`). `tests/cli_naming_test.rs` walks the clap
  `Command` tree and asserts: every command/subcommand has a non-empty
  `about`; every leaf that prints data exposes `--output`; no leaf
  exposes `--format` except behind the hidden deprecated alias; no leaf
  uses `--yes` (only `--force`).
- **T-v8-12** `scripts/demo.sh` replaces the former `pcy demo`
  subcommand. `pcy demo` is deleted (not hidden). The smoke script in
  `scripts/smoke.sh` + `scripts/smoke.ps1` invokes `pcy login` and
  asserts `/openapi.json` returns 200.
- **T-v8-13** v1–v7 acceptance criteria remain green after v8 BUILD.
  `cargo test --all-targets -- --test-threads=1`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, and `cargo deny check advisories` all
  pass at the post-BUILD gate.

### Key Links

- **AC-44** → scope.md v8 AC-44 → `src/api/openapi.rs` (`ApiDoc`,
  `openapi_router`) + `#[utoipa::path]` annotations on every handler
  in `src/api/{agents,credentials,me,events,messages,webhooks,
bootstrap}.rs` + `src/api/mod.rs` (mount on unauth router) →
  `tests/openapi_spec_test.rs` → runtime proof: in-process
  `api::router()` spin-up; `GET /openapi.json` parses as
  `openapiv3::OpenAPI`; path enumeration diff vs `router()` is empty;
  `Content-Type` is `application/json`; rate-limit bucket is the
  `/health` bucket; YAML variant returns the same document.
- **AC-45** → scope.md v8 AC-45 → `src/cli/commands/login.rs`
  (`run_with_bootstrap`, bootstrap-or-login branch) + `src/cli/mod.rs`
  (sole `Login` variant; no `Bootstrap` variant) →
  `tests/cli_login_idempotent_test.rs` → runtime proof:
  docker-compose fresh reset + `pcy login --bootstrap-token $T` →
  exit 0 with `already_bootstrapped:false`; second `pcy login
--bootstrap-token $T` against same server → exit 0 with
  `already_bootstrapped:true` and **no 409**; `pcy --help` does not
  list `bootstrap` (matches `gh auth login` / `oc login` ergonomic).
- **AC-46** → scope.md v8 AC-46 → `src/cli/mod.rs` (v8 `Commands`
  enum) + `src/cli/nouns/{agent,credential,budget,event,context,auth,
completion,mcp}.rs` + `src/cli/resolve.rs` + `src/cli/commands/mod.rs`
  (shim delegates) → `tests/cli_noun_verb_test.rs` → runtime proof:
  parameterized `(legacy_cmd, new_cmd)` pairs produce byte-identical
  stdout against a common fixture; ambiguous-name case exits 2 with
  two-column `ID  NAME` table on stderr; not-found exits 1; UUID path
  works; hidden shims each emit exactly one deprecation warning.
- **AC-47** → scope.md v8 AC-47 → `src/cli/output.rs` (`OutputFormat`,
  `TableRow`, `render`, `default_for_tty`) + per-noun `TableRow`
  impls in `src/cli/nouns/*` + root `Cli` gains `--output` flag →
  `tests/cli_output_flag_test.rs` → runtime proof: `--output json`
  parses as JSON; `--output yaml` parses as YAML; `--output name`
  emits one name per line; `--output jsonpath='{.items[*].name}'`
  filters correctly over fixture data; PTY fixture confirms TTY
  default is `table` and pipe default is `json`; `NO_COLOR=1`
  suppresses ANSI in `table`; `--format json` warns once then behaves
  as `--output json`; `--yes` warns once then behaves as `--force`.
- **AC-48** → scope.md v8 AC-48 → `src/cli/config.rs` (v8
  `ContextConfig`/`CliConfig`) + `src/cli/migrate.rs` +
  `src/cli/nouns/context.rs` (list/current/use/set/delete) + root
  `Cli` gains `--context` flag → `tests/cli_context_test.rs` →
  runtime proof: v4 flat fixture file on disk → `pcy context list`
  migrates in place, writes `config.toml.pre-v8` backup, idempotent
  on re-run; two-context file → `pcy context use prod` flips
  `current-context`; `--context prod` flag overrides env; env
  overrides file; `pcy whoami` against a context with a bad token
  exits non-zero; atomic save (tempfile + rename) verified by
  mid-write crash simulation.
- **AC-49** → scope.md v8 AC-49 → `src/mcp/mod.rs` (`run_stdio` event
  loop) + `src/mcp/protocol.rs` (`JsonRpcRequest`/`Response`/`Tool`/
  `CallToolResult`) + `src/mcp/tools.rs` (`OpenApiToolRegistry`) +
  `src/mcp/bridge.rs` (tool → HTTP) + `src/cli/nouns/mcp.rs` (`serve`
  verb) + `src/lib.rs` (`pub mod mcp`) → `tests/mcp_smoke_test.rs` →
  runtime proof: spawn `pcy mcp serve` subprocess with a running
  compose; `initialize` returns `serverInfo`/`capabilities`;
  `tools/list` diff against `ApiDoc::openapi()` operations is empty
  (not hard-coded); `tools/call name="agent.create"` creates an agent
  and the corresponding `agent_created` row lands in `events`; stdout
  carries only JSON-RPC (no stray log bytes); framing error on stdin
  is logged to stderr, not stdout.
- **AC-50** → scope.md v8 AC-50 → `install.sh` at repo root +
  `tests/installer_test.rs` (behind `#[cfg(feature = "installer-e2e")]`)
  - `docs/runbooks/cli-install.md` → runtime proof: `bash -n
install.sh` clean; `shellcheck -S warning install.sh` clean;
    local-fixture GitHub mirror drives end-to-end install; sha256
    mismatch exits non-zero and leaves no binary installed; cosign
    absent + `--require-cosign` exits non-zero; cosign absent + default
    prints `warning:` and installs; cosign present with bad signature
    exits non-zero.
- **AC-51** → scope.md v8 AC-51 → `src/cli/nouns/completion.rs`
  (clap_complete dispatch) + `Cargo.toml` (`clap_complete` dev-dep or
  runtime dep per design) + README + `docs/runbooks/cli-install.md` →
  `tests/cli_completion_test.rs` → runtime proof: `pcy completion
bash` exits 0 with non-empty stdout containing `_pcy`; zsh output
  contains `#compdef`; fish contains `complete -c pcy`; powershell
  contains `Register-ArgumentCompleter`.
- **AC-52** → scope.md v8 AC-52 → `tests/api_naming_test.rs` (AC-52a,
  walks `ApiDoc::openapi()`) + `tests/cli_naming_test.rs` (AC-52b,
  walks clap `Command` tree) → runtime proof: every collection
  segment plural; every primary-key param is `{id}` (allowlist
  empty); every operation summary non-empty, ≤ 72 chars, no trailing
  period; no `PUT` appears; no `format: "uuid-v7"` schemas; every
  clap command has `about`; every data-printing leaf exposes
  `--output`; `--format` only under the deprecated alias; `--yes`
  absent outside deprecation.

### Acceptance Criteria Coverage

| AC     | Planned Test                              | Planned Runtime Verification                                                                                                                                   | Status  |
| ------ | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| AC-44  | `tests/openapi_spec_test.rs`              | `/openapi.json` returns 200 + parses as `openapiv3::OpenAPI`; path diff vs `api::router()` is empty; YAML variant parses; unauth + `/health` rate-limit bucket | Planned |
| AC-45  | `tests/cli_login_idempotent_test.rs`      | Fresh compose + `pcy login` × 2 both exit 0 (no 409); `bootstrap` alias emits one warn; `--help` excludes `bootstrap`; stdout regex match                      | Planned |
| AC-46  | `tests/cli_noun_verb_test.rs`             | Parameterized (legacy, new) pairs → byte-identical stdout; ambiguous-name exit 2 + stderr table; not-found exit 1; UUID path works                             | Planned |
| AC-47  | `tests/cli_output_flag_test.rs`           | json/yaml/name/jsonpath all parse; PTY fixture confirms TTY default table, pipe default json; NO_COLOR suppresses ANSI; `--format`/`--yes` warn                | Planned |
| AC-48  | `tests/cli_context_test.rs`               | v4 flat → v8 migration writes `.pre-v8` backup, idempotent; `use` switches `current-context`; flag > env > file precedence; atomic save                        | Planned |
| AC-49  | `tests/mcp_smoke_test.rs`                 | `pcy mcp serve` subprocess: `initialize` + `tools/list` diff vs `ApiDoc` empty + `tools/call agent.create` → event lands server-side                           | Planned |
| AC-50  | `tests/installer_test.rs` (feature-gated) | `bash -n` + `shellcheck -S warning` clean; fixture-served install succeeds; sha256 mismatch + cosign-required both exit non-zero                               | Planned |
| AC-51  | `tests/cli_completion_test.rs`            | Four shells each exit 0 with non-empty stdout containing shell-specific marker (`_pcy`/`#compdef`/`complete -c pcy`/`Register-Argument…`)                      | Planned |
| AC-52a | `tests/api_naming_test.rs`                | `ApiDoc::openapi()` walk: plural collection paths, `{id}` params, summaries ≤72 no-period, no PUT, no `format:"uuid-v7"`                                       | Planned |
| AC-52b | `tests/cli_naming_test.rs`                | clap `Command` walk: every command has `about`; every data leaf exposes `--output`; `--format`/`--yes` absent outside deprecated shim                          | Planned |

### Scope Reduction Risks

Concrete places BUILD may be tempted to ship a shell/placeholder. Each
is locked by a named assertion in the coverage table above.

- **AC-44 — utoipa annotations skipped on "obvious" endpoints.** Tempting
  to annotate only new routes. `openapi_spec_test.rs`'s path-diff
  assertion fails closed if any route in `api::router()` is absent from
  `ApiDoc::paths(...)`. Webhooks and bootstrap are in scope.
- **AC-44 — `/openapi.json` returns a hand-maintained JSON file.** The
  source of truth must be `ApiDoc::openapi()` serialized at request
  time (or once at startup, cached). A checked-in JSON would drift.
  Test asserts the served document equals `ApiDoc::openapi()` exactly
  after canonicalization.
- **AC-45 — `pcy login` 409s on re-run.** Tempting to just call
  `/api/bootstrap` unconditionally. Scope locks: first call probes (or
  handles `409` by falling through to `/api/login`). Re-run must exit
  0, not "already bootstrapped" non-zero.
- **AC-46 — name-or-UUID resolver only handles UUIDs.** Falling back
  to "not found" for a valid name would silently break operator
  muscle memory. Scope locks: non-UUID input triggers a LIST filtered
  by exact name; ambiguity and zero-match have distinct exit codes
  (2 vs 1). Substring match is **explicitly forbidden**.
- **AC-46 — legacy shim commands become no-ops or error.** Shims must
  delegate and warn once. The parameterized (legacy, new) byte-equal
  test would fail if the shim printed nothing.
- **AC-47 — `--output table` falls through to JSON.** Tempting to
  defer `TableRow` impls ("we have JSON, ship it"). The PTY fixture
  test asserts `table` output structure; absence of headers or a
  JSON object on stdout fails it.
- **AC-47 — `jsonpath` silently accepts unsupported expressions.**
  Scope locks the kubectl subset; unsupported syntax must exit
  non-zero with a specific error, not silently return `[]` or the
  whole document.
- **AC-48 — context migration deferred to a manual command.** Scope
  locks **automatic** migration on first v8 load with backup written
  to `config.toml.pre-v8`. A "`pcy context migrate`" subcommand is
  not a substitute. Migration is idempotent.
- **AC-49 — MCP `tools/list` returns a hard-coded list.** Scope locks
  derivation from `ApiDoc::openapi()`. Smoke test diffs the tool-name
  set against the operation set; any manual list drifts the moment
  a new handler lands.
- **AC-49 — `tools/call` proxies via a shell-out to `pcy` instead of
  `ApiClient`.** Scope locks direct HTTP via `src/mcp/bridge.rs`.
  Shelling out would reparse JSON, double-log, and lose typed errors.
- **AC-50 — `install.sh` skips cosign verification silently when the
  binary is absent.** Scope locks a visible `warning:` stderr line on
  soft-fail and a hard exit under `--require-cosign`. A silent skip
  would make the signing pipeline theater.
- **AC-50 — sha256 mismatch warns and installs anyway.** Scope locks
  non-zero exit with no binary installed on mismatch. Checksum is
  mandatory; cosign is the optional second factor.
- **AC-51 — completion scripts generated but never tested for
  correctness.** Marker-string assertions per shell are the minimum;
  empty stdout or generic stub fails the test.
- **AC-52 — lint tests allowlist every existing violation at ship.**
  Scope locks a clean run: the allowlists for `{id}` compound keys,
  PUT methods, and `--format` usages are **empty** at v8 ship. Any
  future exception requires a justification comment in the allowlist,
  reviewed at REVIEW time.
- **v1–v7 regression risk.** Annotating existing handlers and
  restructuring the CLI tree both touch code the v1–v7 suite
  exercises. BUILD must rerun the full test suite per slice; a slice
  that passes its own new test but breaks an older test is not done.

### Clarifications Needed

None with BUILD impact. The four design-time resolutions below are
bounded and do not change pass/fail for any AC:

1. **AC-47 `jsonpath` is a kubectl-compatible subset**
   (`.foo.bar`, `.items[*].name`, `.items[0]`, `[?(@.k==v)]`) via
   `jsonpath-rust`. Full JQ is reachable via `-o json | jq`. Test
   fixtures only assert the documented subset.
2. **AC-49 MCP spec version is pinned to `2025-06-18`** for v8 ship.
   Version constant + `initialize` response are the only change points
   for a later revision bump.
3. **AC-50 `install.sh` on Windows is supported via git-bash / WSL
   only.** Native PowerShell installer is deferred (`winget` is the
   right seam and lands with the deferred package-manager track).
4. **AC-52 "no `PUT`" is a lint, not an architectural ban.** Allowlist
   is empty at v8 ship; future exceptions are a one-line addition
   with justification comment.

### Build Order

Slices are sized to ship as 1–3 commits each. Dependencies flow top to
bottom; each slice's tests must pass before the next begins, and the
full v1–v7 suite must remain green at every checkpoint.

1. **Slice 1 — AC-44 OpenAPI foundation.** Add `utoipa` + `utoipa-axum`
   to `Cargo.toml`. Create `src/api/openapi.rs` with `ApiDoc` +
   `openapi_router()` + `openapi_json`/`openapi_yaml` handlers + the
   `BearerAuthAddon` security modifier. Add `#[utoipa::path]` on every
   handler in `src/api/{me,agents,credentials,events,messages,
webhooks,bootstrap}.rs` and `#[derive(ToSchema)]` on every DTO.
   Mount `openapi_router()` on the unauth side in `src/api/mod.rs`.
   Write `tests/openapi_spec_test.rs` (spec served, 3.1 parses, route
   diff empty, Content-Type correct, rate-limit shared with `/health`).
   **Unblocks AC-49 and AC-52a.**
2. **Slice 2 — AC-46 CLI restructure + AC-48 contexts + AC-47 output
   flag.** These three land together because they share the root
   `Cli` struct surgery. Create `src/cli/nouns/` (mod.rs +
   agent/credential/budget/event/context/auth/completion/mcp) by
   moving the current command bodies, keeping thin shim variants in
   `src/cli/commands/mod.rs` (`bootstrap_shim`, `message_shim`,
   `events_shim`) that call `warn_deprecated` + delegate. Rewrite
   `src/cli/config.rs` with v8 `ContextConfig`/`CliConfig` and
   `src/cli/migrate.rs` auto-migration + atomic save. Add
   `--context`/`--output` to the root `Cli`. Create `src/cli/output.rs`
   (enum + `TableRow` trait + `render` + `default_for_tty`) and
   per-noun `TableRow` impls. Create `src/cli/resolve.rs` with
   `resolve_agent`/`resolve_credential`/`resolve_event` covering UUID,
   exact-name, ambiguous, not-found. Write
   `tests/cli_noun_verb_test.rs`, `tests/cli_context_test.rs`,
   `tests/cli_output_flag_test.rs`. Keep `src/cli/output.rs` ≤ 250
   lines; push overflow into noun modules.
3. **Slice 3 — AC-45 idempotent login.** Implement `src/cli/nouns/
auth.rs::login` with the bootstrap-or-login decision tree (probe
   `/api/me`, fall through to `/api/bootstrap` on 401 "not
   bootstrapped", fall through to `/api/login` on 409 "already
   bootstrapped"), persist token into active context. Wire
   `bootstrap_shim` to delegate to `login` with one warning. Write
   `tests/cli_login_idempotent_test.rs` (compose fresh + login × 2,
   alias warning count, `--help` exclusion). **Depends on Slice 2
   context storage.**
4. **Slice 4 — AC-49 MCP server.** Create `src/mcp/mod.rs` (stdio event
   loop, `run_stdio`), `src/mcp/protocol.rs` (JsonRpc types +
   newline-delimited framing), `src/mcp/tools.rs` (`OpenApiToolRegistry`
   reading `ApiDoc::openapi()`), `src/mcp/bridge.rs` (tool → HTTP via
   `ApiClient`, error-code table). Wire `src/cli/nouns/mcp.rs::serve`.
   Add `pub mod mcp` to `src/lib.rs`. Write `tests/mcp_smoke_test.rs`
   (subprocess spawn, initialize, tools/list diff vs `ApiDoc`,
   tools/call agent.create → server-side event lands). Keep
   `src/mcp/mod.rs` ≤ 300 lines; beyond that split into `event_loop.rs`
   - `dispatch.rs`. **Depends on Slice 1 (ApiDoc) and Slice 2 (active
     context).**
5. **Slice 5 — AC-50 installer + AC-51 completions.** Finalize
   `install.sh` at repo root (platform detect, release resolve,
   sha256 enforce, cosign verify with soft/hard fail modes, install
   to `$PCY_PREFIX/bin`). Move the former `pcy demo` flow into
   `scripts/demo.sh` and delete `pcy demo`. Implement
   `src/cli/nouns/completion.rs` using `clap_complete`. Add the
   `installer-e2e` feature to `Cargo.toml`. Write
   `tests/installer_test.rs` (feature-gated) with `bash -n` +
   shellcheck + fixture GitHub mirror + sha256 mismatch + cosign
   required gate. Write `tests/cli_completion_test.rs` (four shells
   × marker string). Update README + create
   `docs/runbooks/cli-install.md` and `docs/runbooks/mcp-setup.md`.
   **Independent of Slices 3–4; may overlap if capacity permits.**
6. **Slice 6 — AC-52 lint guardrails.** Last because they audit
   everything that came before. Write `tests/api_naming_test.rs`
   (walks `ApiDoc::openapi()`) and `tests/cli_naming_test.rs` (walks
   clap `Command` tree) with empty allowlists at v8 ship. Fix any
   violations surfaced in Slices 1–5 (expected small: rename any
   `{agentId}` → `{id}`, trim summaries > 72 chars, convert any
   lingering `PUT` → `POST`/`PATCH`). Update the smoke script to
   hit `/openapi.json`.

**Post-Slice-6 gate**: `cargo test --all-targets --
--test-threads=1` + `cargo clippy --all-targets -- -D warnings` +
`cargo fmt --all -- --check` + `cargo deny check advisories` all
pass; the full v1–v7 AC suite is still green; then REVIEW.

### Complexity Exceptions

Carried forward from `design.md` v8 addendum — four bounded
exceptions, each with a hard ceiling and a predefined split plan.

1. **`src/mcp/mod.rs` may exceed the 200-line soft target.** JSON-RPC
   stdio event loops (framing + dispatch + error mapping + graceful
   shutdown) are irreducible below that threshold. **Hard ceiling
   300 lines**; beyond that split into `event_loop.rs` + `dispatch.rs`.
2. **`src/cli/output.rs` hosts the enum + `render` + `TableRow` impls
   for the common cases.** **Hard ceiling 250 lines**; beyond that
   push per-resource `TableRow` impls into their noun modules.
3. **Legacy-shim compatibility duplicates some test paths** (hidden
   `bootstrap`/`message`/`events` commands, `--format`/`--yes` flag
   aliases). Accepted for one release; removed in v1.2.0 along with
   the duplicate tests.
4. **`utoipa::path` annotations above every handler are verbose.**
   Accepted — they are the source of truth for AC-44 (machine-readable
   contract) and AC-52a (schema-layer lints).

No other complexity exceptions beyond the v1–v7 exceptions already
recorded. No new soft-ceiling extensions. No deferred file budgets.

---

## v9 ANALYZE — Trust Gate Readiness (2026-04-22)

**Verdict: READY.** Scope v9 (AC-53..AC-75, 23 ACs) and design.md v9 DESIGN section are consistent. All four scope clarifications are resolved in writing, and the audit addendum adds AC-73/74/75 plus a risk register. Every AC has a named test file and a runtime proof path. Build order is sequenced so each slice gates the next.

### Truths (non-negotiable statements that must be true in shipped v9.0)

1. **No tool call on Linux reaches `execve` without passing through all six sandbox layers** (bwrap, cgroup v2, landlock, seccomp allowlist, cap/uid drop, netns+slirp4netns). Any layer missing → `sandbox_unavailable` error, no execution.
2. **Plaintext credentials never reside in the agent process address space.** The secret proxy (`src/runtime/secret_proxy.rs`) is the sole component with vault-key read access.
3. **Every `sqlx` query in `src/api/` flows through `ScopedPool`.** The tenancy lint fails CI on any direct `sqlx::query*` call in handler code.
4. **Cross-workspace reads return HTTP 404, never 403.** Presence leaks are a tenancy bug.
5. **Session tokens expire.** No session survives past `expires_at`; `/api/sessions/refresh` is the only extension path.
6. **Deposit tokens are single-use and expire in 24h.** No reuse, no long-lived secret URLs.
7. **Every P0 AC has an adversarial test.** Happy-path tests alone do not close a P0.
8. **Sandbox bypass requires explicit dual opt-in.** `OPEN_PINCERY_SANDBOX_MODE=disabled` is invalid unless `OPEN_PINCERY_ALLOW_UNSAFE=true` is also set.

### Key Links (AC → design component → test → runtime proof)

| AC    | Title                              | Design component                                                                | Test file                                                                   | Runtime proof                                                                                                                         |
| ----- | ---------------------------------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | --------- | ---------------- |
| AC-53 | Industry-leading sandbox           | `src/runtime/sandbox/{mod,bwrap,seccomp,landlock,cgroup,netns}.rs`              | `tests/sandbox_escape_test.rs`                                              | 12 adversarial payloads executed live; every failure emits `sandbox_blocked` event visible via `pcy events <agent>`                   |
| AC-54 | SECURITY.md threat model           | `docs/SECURITY.md` + README link                                                | `tests/security_doc_test.rs`                                                | `curl http://localhost:8080/docs/SECURITY.md` or repo view                                                                            |
| AC-55 | Credential request tool            | `src/runtime/tools.rs` + `src/api/credential_requests.rs` + migration           | `tests/credential_request_tool_test.rs`                                     | Agent emits `credential_requested` event; `pcy credential request list` shows row; `deposit_token` absent from event payload          |
| AC-56 | Deposit page                       | `src/api/deposit.rs` + HTML template                                            | `tests/credential_deposit_test.rs`                                          | Open `/deposit/<token>` in browser → form renders; POST → 303; second POST → 410                                                      |
| AC-57 | Credential inbox (CLI + UI)        | `src/cli/commands/credential_request.rs` + `static/views/credential_inbox.html` | `tests/cli_credential_request_test.rs`                                      | `pcy credential request {list,approve,reject}` verbs work against live DB                                                             |
| AC-58 | Session TTL + refresh + revoke     | `src/api/sessions.rs` + migration                                               | `tests/session_ttl_test.rs`                                                 | `curl` with expired token → 401; `POST /api/sessions/refresh` extends; `pcy session revoke` invalidates                               |
| AC-59 | Users + roles                      | `src/api/users.rs` + `src/cli/commands/user.rs` + migration                     | `tests/rbac_test.rs`                                                        | 3 roles × endpoint matrix: viewer blocked from POST; operator blocked from user-mgmt; admin open                                      |
| AC-60 | Auth README rewrite                | `README.md` Authentication section                                              | `tests/readme_auth_section_test.rs`                                         | README grep asserts three-box diagram + token table                                                                                   |
| AC-61 | UI rebuild (HTMX + Pico)           | `static/{js,css,views}/`                                                        | `tests/ui_smoke_test_v9.rs`                                                 | `curl /login`, `/agents`, `/agents/:id`, `/events`, `/budget`, `/credentials/requests` return 200 with CSP header                     |
| AC-62 | Event search + export              | `src/api/events_export.rs`                                                      | `tests/event_search_export_test.rs`                                         | `curl /api/agents/:id/events.jsonl?q=foo&type=tool_call` streams NDJSON                                                               |
| AC-63 | Cost reports                       | `src/api/cost.rs` + `src/cli/commands/cost.rs`                                  | `tests/cost_report_test.rs`                                                 | `pcy cost <agent> --group-by model` renders table; matches `llm_calls` sum                                                            |
| AC-64 | Retention + archive                | `src/background/retention.rs` + `src/cli/commands/events_archive.rs`            | `tests/event_retention_test.rs`                                             | Seed old events; `pcy events archive --older-than 90d`; rows pruned, gzipped JSONL on disk                                            |
| AC-65 | Multi-tenant enforcement           | `src/tenancy.rs` + every `src/api/*.rs` handler                                 | `tests/multi_tenant_isolation_test.rs` + `tests/tenancy_middleware_test.rs` | 5×5 matrix: alpha token on beta IDs returns 404; SQLi probes return 404; lint fails on bare query                                     |
| AC-66 | Tool catalog expansion             | `src/runtime/tools/{http_get,file_read,db_query}.rs`                            | `tests/tool_catalog_test.rs`                                                | Each tool registered; scoping test asserts host/path/SQL enforcement                                                                  |
| AC-67 | Workspace rate limiting            | `src/background/rate_limit.rs`                                                  | `tests/workspace_rate_limit_test.rs`                                        | 601 calls in 60s → 601st delayed 1s + `rate_limit_exceeded` event                                                                     |
| AC-68 | Ollama bullet                      | `README.md` + config loader                                                     | `tests/ollama_config_test.rs`                                               | README grep asserts bullet; config loader parses `host.docker.internal:11434` URL                                                     |
| AC-69 | Version handshake                  | `src/api/version.rs` + CLI version check                                        | `tests/version_handshake_test.rs`                                           | Stubbed v0.8 server vs v0.9 CLI → warning; v0 server vs v1 CLI → exit 3                                                               |
| AC-70 | Terminology lock                   | README opening paragraph                                                        | `tests/terminology_test.rs`                                                 | Regex assertion over README/DELIVERY/docs asserts no `bot                                                                             | assistant | worker` synonyms |
| AC-71 | Secret injection proxy             | `src/runtime/secret_proxy.rs` + IPC contract                                    | `tests/secret_proxy_test.rs`                                                | Agent memory via `/proc/<pid>/maps` sweep shows no credential bytes; sandboxed child sees value; `secret_injected` event emitted      |
| AC-72 | Per-agent network egress allowlist | `src/runtime/sandbox/netns.rs` + migration + CLI                                | `tests/network_egress_test.rs`                                              | Allowed host `curl` succeeds; denied host blocked + `network_blocked` event in log                                                    |
| AC-73 | Sandbox mode flag                  | `src/config.rs` + `src/runtime/sandbox/mod.rs`                                  | `tests/sandbox_mode_test.rs` + `tests/sandbox_perf_test.rs`                 | `enforce` blocks, `audit` emits `sandbox_would_block`, `disabled` requires `OPEN_PINCERY_ALLOW_UNSAFE=true`; startup self-test passes |
| AC-74 | Credential plaintext hygiene       | `src/observability/redaction.rs` + `src/runtime/secret_proxy.rs`                | `tests/credential_hygiene_test.rs`                                          | Logs redact credential-shaped values; event insert rejects plaintext; dropped buffers zeroized and `mlock`ed                          |
| AC-75 | Cross-platform dev environment     | `Dockerfile.devshell` + `scripts/devshell.{sh,ps1}` + runbooks                  | `tests/devshell_parity_test.rs`                                             | Mac/Windows contributors run `devshell cargo test`; parity test matches Linux verdict                                                 |

### Acceptance Criteria Coverage

Every AC in scope v9 appears in the table above with a planned test and a planned runtime proof. No AC is closed by a unit test alone; every P0 AC is closed by adversarial test + observable event.

### Scope Reduction Risks

1. **AC-53 landlock may be skipped if kernel < 5.13.** Mitigation: CI runs on ubuntu-24.04 (kernel 6.8+); docs document a minimum kernel floor for self-hosters. Scope reduction risk: ZERO on CI; self-hoster risk mitigated by explicit warning event.
2. **AC-65 middleware migration is one large slice.** Temptation: migrate half the endpoints, leave the rest. Guardrail: lint test blocks merge with any unscoped query remaining. REVIEW must confirm lint is active before slice merges.
3. **AC-71 injection-mode `HttpHeader` requires changes in `http_get` tool at the same time.** Risk: shipping secret proxy without the `http_get` integration leaves a half-feature. Slice A2c MUST include `http_get` cutover.
4. **AC-61 UI rebuild temptation to keep hand-rolled hash-routing as a fallback.** Guardrail: `static/js/` is wholesale replaced, not layered.
5. **AC-66 `db_query` read-only enforcement via server-side regex.** Risk: regex is bypassable via `;` stacking or comment-terminated statements. Mitigation: use a read-only role at the Postgres level (`SET TRANSACTION READ ONLY`) as defense-in-depth, regex is belt-and-suspenders.

### Clarifications Needed

None. All four original clarifications were resolved by user on 2026-04-22 and recorded verbatim in `scope.md` under "Clarifications Resolved."

### Build Order

Sequenced per scope.md Build Order. Summary:

- **Phase A** (Security Truth, ~4-5 weeks): A0 devshell → A1 SECURITY.md → A2a sandbox core + mode flag → A2b egress allowlist → A2c secret proxy + hygiene → A3 session TTL → A4 users+roles → A5 auth README
- **Phase B** (Credential Requests, ~1 week): B1 tool+schema → B2 deposit page → B3 CLI+UI inbox
- **Phase C** (UI Rebuild, ~1 week): C1 HTMX+Pico six views
- **Phase E** (Multi-tenant Enforcement, ~2 weeks — blocking v9.0): E1a schema → E1b middleware → E1c endpoint migration → E1d isolation matrix test
- **v9.0 ships here** (Phases A+B+C+E complete = full trust gate)
- **Phase D** (Observability, ~1 week, ships as v9.1): D1 search+export → D2 cost reports → D3 retention+archive
- **Phase F** (Polish, ~1 week, ships as v9.2): F1 tool catalog → F2 rate limit → F3 Ollama → F4 version handshake → F5 terminology lock

Total engineering budget: 8-10 weeks.

### Complexity Exceptions (carried from DESIGN)

1. `src/runtime/sandbox/mod.rs` budget 400 lines (compose + partial-failure cleanup).
2. `tests/sandbox_escape_test.rs` ~500 lines acceptable.
3. AC-65 endpoint-migration slice touches ~25 files at once — required, not optional.
4. `src/tenancy.rs::Binds` is a bespoke subset of `sqlx` binds.

All four are explicit and REVIEW-gated; none is a placeholder waiver.

---

## v9 AUDIT ADDENDUM — Risks, Mitigations, Hardening (2026-04-22T11:00Z)

An adversarial audit of the v9 plan surfaced 18 concrete risks. Three warranted new ACs (AC-73 Sandbox Mode Flag, AC-74 Credential Hygiene, AC-75 Cross-Platform Dev Env). The remaining 15 are hardening details internal to existing ACs, documented below with the mitigation embedded in the slice that owns it.

### Risk Register

| #   | Risk                                                                                                                                          | Owning AC / Slice  | Mitigation                                                                                                                                                                                                                                          | Evidence gate                                                                                        |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 1   | CI runner may not support user namespaces / unprivileged bwrap                                                                                | AC-53 / A2a        | CI job installs `bubblewrap slirp4netns uidmap` explicitly; preflight step greps `/proc/sys/kernel/unprivileged_userns_clone == 1`; if missing, sets it via `sudo sysctl -w`.                                                                       | `ci/sandbox-preflight.yml` green on ubuntu-24.04                                                     |
| 2   | Sandbox startup cost regresses tool-call latency past acceptance                                                                              | AC-73 / A2a        | Hard perf budget 300ms p95, 500ms hard fail. Counter `sandbox_exec_duration_ms` emitted per call; CI runs 100 warm tool calls and asserts histogram.                                                                                                | `tests/sandbox_perf_test.rs`                                                                         |
| 3   | `SANDBOX_MODE=disabled` footgun in production                                                                                                 | AC-73              | Requires paired `OPEN_PINCERY_ALLOW_UNSAFE=true`; emits `sandbox_mode_changed` event at startup; stderr warning every 60s while disabled.                                                                                                           | `tests/sandbox_mode_test.rs`                                                                         |
| 4   | HTMX + CSP incompatibility (inline `hx-on:` handlers require `unsafe-inline`)                                                                 | AC-61 / C1         | Use nonce-based CSP: server generates per-response nonce; HTMX 1.9 `htmx.config.inlineScriptNonce`. No `unsafe-inline`, no `unsafe-eval`.                                                                                                           | `tests/ui_smoke_test_v9.rs` asserts CSP header includes nonce, rejects inline without matching nonce |
| 5   | Deposit page is unauthenticated (AC-56) — vulnerable to CSRF + brute force                                                                    | AC-56 / B2         | Form includes a double-submit token derived from the deposit_token; IP-based rate-limit (10 POSTs/min/IP); every attempt (success OR fail) emits `deposit_attempt` event.                                                                           | `tests/credential_deposit_test.rs` + rate-limit assertion                                            |
| 6   | Session cookie flags missing `HttpOnly` / `Secure` / `SameSite`                                                                               | AC-58 / A3         | `Set-Cookie` contract documented in `src/api/sessions.rs`; `tests/session_cookie_flags_test.rs` asserts all three flags.                                                                                                                            | Cookie flags test green                                                                              |
| 7   | Session token comparison timing attack                                                                                                        | AC-58 / A3         | Use `subtle::ConstantTimeEq` for every session-token compare.                                                                                                                                                                                       | Code review checks for `==` on bytes in session lookup                                               |
| 8   | Existing rows have NULL `workspace_id` on upgrade (AC-65 migrations fail)                                                                     | AC-65 / E1a        | Migration `20260501000001_add_workspace_id_to_sessions.sql` CREATEs a "legacy" default workspace if none exists, backfills all existing rows, THEN adds NOT NULL. Rollback note in migration.                                                       | Migration dry-run on v8 snapshot + `tests/upgrade_from_v8_test.rs`                                   |
| 9   | Tenancy lint false positives (health checks, migrations legitimately use raw `sqlx::query`)                                                   | AC-65 / E1b        | Lint allowlist: files matching `src/db/**` or `src/background/startup/**`; else `#[allow(tenancy::unscoped)]` attribute required with a comment explaining why.                                                                                     | `tests/tenancy_middleware_test.rs` exercises both allow and deny paths                               |
| 10  | Concurrent tool calls collide on cgroup / netns names; leaked cgroups from crashed processes accumulate                                       | AC-53 / A2a        | Naming: `pincery-<uuid_v4>`; on startup, sweep `/sys/fs/cgroup/pincery-*` older than server uptime and remove. Drop-guard on `SandboxHandle` cleans up even on panic.                                                                               | `tests/sandbox_concurrency_test.rs` runs 50 parallel tool calls, asserts no leaked cgroups           |
| 11  | `zeroize` is best-effort; compiler may elide writes                                                                                           | AC-74 / A2c        | Use `zeroize` crate (which marks as `volatile`); `SecretBuffer` wraps `Vec<u8>` with `Drop` + `ZeroizeOnDrop` derives; `#[deny(unsafe_code)]` on the module.                                                                                        | `tests/credential_hygiene_test.rs` does post-drop memory grep                                        |
| 12  | Log redaction layer false negatives on secret-shaped values without obvious names                                                             | AC-74 / A2c        | Dual strategy: (a) name-matching (password, token, secret, bearer, api*key) via regex on log-record keys; (b) length+shape heuristic for values matching `sk-[a-zA-Z0-9]{16,}` / `ghp*[a-zA-Z0-9]{36}`/ JWT tri-dot format. Both yield`<REDACTED>`. | `tests/credential_hygiene_test.rs` test matrix of 6 credential shapes                                |
| 13  | New crates (`landlock`, `seccompiler`, `cgroups-rs`, `zeroize`, `subtle`, `slirp4netns-bindings`) may have questionable licensing/maintenance | AC-73 / A2a        | `deny.toml` updated with explicit allowlist + version pins; `cargo deny check licenses bans advisories sources` in CI; maintenance check: last commit within 12 months.                                                                             | `cargo deny check` green; `deny.toml` diff reviewed                                                  |
| 14  | Dev path on Mac/Windows breaks (kernel primitives Linux-only) → contributors can't test sandbox                                               | AC-75 / A0         | `scripts/devshell.sh` + `.ps1` launches pinned Docker image; parity test re-runs sandbox suite inside devshell from a Linux CI host.                                                                                                                | `tests/devshell_parity_test.rs` + manual Mac/Windows walkthrough                                     |
| 15  | Tool-call plaintext survives in kernel page cache / swap                                                                                      | AC-71 / A2c        | `mlock()` the SecretBuffer region via `region::lock`; document in SECURITY.md that swap must be disabled on prod hosts or encrypted.                                                                                                                | Code review + SECURITY.md section "Deployment Hardening"                                             |
| 16  | AC-65 endpoint migration (25 files at once) too large to review safely                                                                        | AC-65 / E1c        | Pre-slice preparatory slice E1b MUST ship the middleware + lint first; E1c then becomes a mechanical migration where every file-edit follows an identical pattern. REVIEW checks the PATTERN once, then samples 5 files.                            | REVIEW comment log in E1c commit                                                                     |
| 17  | No rollback plan if v9.0 production upgrade fails                                                                                             | Pre-v9 / bootstrap | Tag `v8.0.1-pre-v9-baseline` on current `v6-01_implementation` HEAD; v9.0 release notes include "to roll back: `git checkout v8.0.1-pre-v9-baseline && docker compose down && up --build`".                                                         | Tag exists before first v9 build commit                                                              |
| 18  | No canary / staged rollout for self-hosted operators                                                                                          | AC-73 / A2a        | `SANDBOX_MODE=audit` lets operators run a week in log-only mode, see what WOULD be blocked, adjust allowlists, THEN flip to `enforce`. Documented in DELIVERY.md "v9 Upgrade Playbook".                                                             | Upgrade playbook section exists                                                                      |

### Definition-of-Done Matrix (per slice, enforced by REVIEW)

| Check                                                | Mechanism                                                  | Pass condition                                                                                     |
| ---------------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Compiles + clippy clean                              | `cargo clippy --all-targets --all-features -- -D warnings` | Exit 0                                                                                             |
| Every new AC has a named test file that passes       | `cargo test`                                               | All targets green                                                                                  |
| Every P0 AC has an adversarial (not happy-path) test | Manual REVIEW                                              | Test file contains negative assertions that could only pass with the feature correctly implemented |
| New event types registered in `src/models/events.rs` | `tests/event_type_lint.rs`                                 | All new event types enumerated                                                                     |
| New CLI verbs in noun-verb tree                      | `tests/cli_naming_lint.rs` (AC-52b)                        | Lint green                                                                                         |
| Migrations are additive + include backfill           | REVIEW + `tests/upgrade_from_v8_test.rs`                   | No destructive DDL, backfill covers all existing rows                                              |
| `CHANGELOG.md` has Phase-tagged entry                | `tests/changelog_test.rs`                                  | Entry exists with AC-IDs                                                                           |
| `deny.toml` + `cargo deny check`                     | CI                                                         | No unreviewed new crates; licenses allowed                                                         |
| `cargo audit`                                        | CI                                                         | No high/critical advisories                                                                        |
| Threat-model impact signed off                       | REVIEW                                                     | If slice touches auth / sandbox / tenancy, reviewer records impact in commit trailer               |
| Rollback tag                                         | Git                                                        | `v9.0.0-phase<X>-slice<N>` tag on slice commit                                                     |

### Pre-v9 Baseline Tag

Before BUILD Slice A0 merges, tag the current HEAD:

```bash
git tag -a v8.0.1-pre-v9-baseline -m "Last v8 commit before v9 BUILD begins. Rollback target."
git push origin v8.0.1-pre-v9-baseline
```

Rollback recipe documented in `docs/runbooks/rollback_to_v8.md` (create in Slice A0).

### Upgraded Verdict

**READY** — with the 3 new ACs (AC-73/74/75) added and the 15 in-slice risks documented. Scope totals: **23 ACs**, **8-10 weeks**. v9.0 ships after Phases A + B + C + E complete.
