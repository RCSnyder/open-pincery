---- MODULE OpenPinceryAgent ----

(* ================================================================
   Open Pincery — Continuous Agent Architecture
   Reference TLA+ Specification
   
   This module is the authoritative state machine for the Open Pincery
   platform. It models the complete lifecycle of a single durable,
   event-driven agent, including all subsystems described in the
   Continuous Agent Architecture document.
   
   SCOPE: One agent's lifecycle. Multi-agent coordination is modeled
   as external events arriving at this agent's boundary. Each agent
   in the platform runs an independent instance of this state machine.
   
   ARCHITECTURE SUMMARY:
   - Agents are continuous entities with durable identity and work list
   - All state lives in an append-only event log in Postgres
   - The prompt is a bounded working surface, not the memory
   - Agents wake on events, reason via LLM + tool loops, sleep when done
   - Between wakes, a single-call maintenance pass updates projections
   - CAS (compare-and-swap) ensures exactly one wake at a time
   - Inter-agent messages are async, freeform, via the runtime harness
   
   POSTGRES IMPLEMENTATION NOTES (in comments throughout):
   - CAS lifecycle: UPDATE ... WHERE status = $expected RETURNING *
   - Event log: INSERT-only table, indexed by (agent_id, timestamp)
   - Wake triggers: LISTEN/NOTIFY on per-agent channels
   - Webhook dedup: INSERT ... ON CONFLICT DO NOTHING on SHA-256 hash
   - Projections: TEXT columns with version history rows
   ================================================================ *)

VARIABLE agentState

AgentStates == {
    "Resting",
    "EventArrived",
    "WakeAcquiring",
    "WakeAcquireFailed",
    "PromptAssembling",
    "Awake",
    "ToolDispatching",
    "ToolPermissionChecking",
    "AwaitingApproval",
    "ApprovalRejected",
    "ToolExecuting",
    "ToolResultProcessing",
    "MidWakeEventPolling",
    "EventInjecting",
    "ImplicitSleeping",
    "ExplicitSleeping",
    "IterationCapHit",
    "ContextCapHit",
    "BudgetExhausted",
    "WakeEnding",
    "EventCollapsing",
    "MaintenanceCalling",
    "MaintenanceWriting",
    "SummaryWriting",
    "Draining",
    "DrainAcquiring",
    "StaleDetected",
    "StaleRecovering",
    "WebhookReceived",
    "WebhookDeduplicating",
    "WebhookNormalizing"
}

(* ================================================================
   SECTION 1: EVENT INGRESS
   
   Events enter the system through four channels:
   1. Human messages (from the platform user)
   2. Webhooks (HTTP POST from any external system)
   3. Timer firings (scheduled by the agent or platform)
   4. Inter-agent messages (from other agents via send_message)
   
   All four produce the same event type: message_received with a
   source field (human, webhook, timer, agent). The LLM sees them as
   user-role messages with a bracketed header:
     [Message from Alice]\n...
     [Webhook received]\n...
     [Timer fired: daily report (scheduled ...)]\n...
     [Message from ResearchBot]\n...
   
   The harness-as-user pattern: "user" does not mean human. It means
   "the next event, with the actual source named in content." This
   keeps the transport generic while preserving full meaning.
   ================================================================ *)

(* An external event arrives from any source: human message, timer
   firing, or inter-agent message (webhooks handled separately below).
   
   Human messages pass through an ingress scanner before insertion.
   Known secret / credential patterns are redacted so raw secrets do
   not enter the durable event log or future prompt windows.

   The event is IMMEDIATELY appended to the agent's event log in
   Postgres via INSERT, regardless of the agent's current lifecycle
   state. The event is durable from this moment forward.
   
   Postgres (human-message redaction step before insert):
     -- Scan content for patterns like /AKIA[A-Z0-9]{16}/,
     --   /sk-proj-[a-zA-Z0-9-]+/, /ghp_[a-zA-Z0-9]+/
     -- Replace matches with [REDACTED_SECRET:<type>]
     -- Optionally append human_message_redacted audit metadata

   Postgres: INSERT INTO events (agent_id, event_type, source,
     content, created_at) VALUES ($1, 'message_received', $2, $3, NOW())
   
   After insert, NOTIFY agent_$id is issued to wake any listening
   runtime process. If the agent is already awake, the notification
   is consumed but no new wake starts — the running wake will see
   the event via mid-wake polling or the drain check. *)
EventArrives ==
    /\ agentState = "Resting"
    /\ agentState' = "EventArrived"

(* ================================================================
   SECTION 2: WEBHOOK INGRESS SUBSYSTEM
   
   Webhooks are the general-purpose bridge between external systems
   and the agent's event stream. Any system that can make an HTTP
   POST can wake an agent.
   
  Webhook authentication uses per-source HMAC-SHA256 signature
  verification on request headers. The handler: authenticates,
  deduplicates, normalizes, inserts, and triggers a wake attempt.
   ================================================================ *)

(* A webhook HTTP POST arrives at the agent's webhook endpoint.
  The handler validates the source-specific HMAC signature using a
  shared secret stored in the credential/config layer.
   If authentication fails, return 401 and stop.
   If authentication succeeds, proceed to deduplication.
   
  Axum handler: POST /webhook/:agent_id
  Headers: X-OpenPincery-Signature-256: sha256=<hex-digest> *)
WebhookArrives ==
    /\ agentState = "Resting"
    /\ agentState' = "WebhookReceived"

(* Deduplication: compute SHA-256 hash of (body + query parameters).
   Attempt INSERT with the hash as unique key.
   
   Postgres: INSERT INTO webhook_dedup (agent_id, body_hash, received_at)
     VALUES ($1, $2, NOW())
     ON CONFLICT (agent_id, body_hash) DO NOTHING
     RETURNING id
   
   If RETURNING yields no row, this is a duplicate within the dedup
   window. Return HTTP 202 (accepted) but do NOT create an event or
   trigger a wake. The original delivery already handled it.
   
   Dedup window cleanup: DELETE FROM webhook_dedup
     WHERE received_at < NOW() - INTERVAL '1 hour'
   (run periodically via pg_cron or application-level sweep) *)
WebhookDeduplicates ==
    /\ agentState = "WebhookReceived"
    /\ agentState' = "WebhookDeduplicating"

(* Content normalization: convert arbitrary HTTP input into a single
   LLM-readable string.
   
   Rules:
   - JSON body (Content-Type: application/json): pretty-print with
     serde_json::to_string_pretty. The LLM reads structured JSON well.
   - Form-urlencoded (application/x-www-form-urlencoded): parse into
     key=value pairs, one per line.
   - Multipart (multipart/form-data): summarize each part by part name,
     filename (if present), content type, and size in bytes. Do NOT
     store binary content in the event — summarize it.
   - Raw text (text/plain, or unknown): pass through as-is.
   
   The normalized content is inserted as a message_received event
   with source: "webhook". The agent sees:
     [Webhook received]\n<normalized content>
   
   Then proceed to wake acquisition (same as any other event). *)
WebhookNormalizes ==
    /\ agentState = "WebhookDeduplicating"
    /\ agentState' = "EventArrived"

(* ================================================================
   SECTION 3: WAKE ACQUISITION (CAS)
   
   The CAS (compare-and-swap) lifecycle ensures exactly one wake is
   active at a time per agent. This is the critical correctness
   property of the entire system.
   
   State transitions (Postgres column: agents.status):
     'asleep'      -> 'awake'        (wake acquire)
     'awake'       -> 'maintenance'  (wake ends)
     'maintenance' -> 'asleep'       (maintenance completes)

   Administrative suspension is NOT an additional lifecycle state.
   The coarse runtime lifecycle stays asleep/awake/maintenance.
   A separate control-plane flag (for example agents.is_enabled or
   agents.disabled_at) gates wake acquisition and tool dispatch
   without changing the CAS contract.
   
   Multiple simultaneous triggers are naturally coalesced: only one
   wins the CAS. The losers exit — their events are already in the
   log and will be seen by the running wake.
   ================================================================ *)

(* Attempt CAS acquire: atomically transition from asleep to awake.
   
   Postgres:
     UPDATE agents
     SET status = 'awake',
         wake_id = gen_random_uuid(),
         wake_started_at = NOW(),
         wake_iteration_count = 0
     WHERE id = $1 AND status = 'asleep' AND is_enabled = TRUE
     RETURNING *
   
   The WHERE clause IS the compare. The UPDATE IS the swap.
   If no row is returned, the CAS failed — agent is already awake
   or in maintenance. *)
AttemptWakeAcquire ==
    /\ agentState = "EventArrived"
    /\ agentState' = "WakeAcquiring"

(* CAS succeeds: this runtime invocation owns the wake.
   Record a wake_start event in the event log:
   
   Postgres: INSERT INTO events (agent_id, event_type, wake_id, created_at)
     VALUES ($1, 'wake_start', $2, NOW())
   
   Proceed to prompt assembly. *)
WakeAcquireSucceeds ==
    /\ agentState = "WakeAcquiring"
    /\ agentState' = "PromptAssembling"

(* CAS fails: agent is already awake or in maintenance.
   This invocation's event is safe in the log. Exit cleanly.
   
   The running wake will see the new event either:
   (a) via mid-wake event polling, or
   (b) via the drain check after maintenance completes.
   
   No work is lost. This is the normal coalescing behavior. *)
WakeAcquireFails ==
    /\ agentState = "WakeAcquiring"
    /\ agentState' = "WakeAcquireFailed"

(* Failed invocation returns to rest. *)
FailedInvocationExits ==
    /\ agentState = "WakeAcquireFailed"
    /\ agentState' = "Resting"

(* ================================================================
   SECTION 4: PROMPT ASSEMBLY
   
   The prompt is assembled from bounded pieces:
   
   SYSTEM PROMPT (assembled in order):
  1. Constitution (~3-4k chars) — stable rules, tool documentation,
    explanation of chat roles, identity/work list semantics.
    This is sourced from the active wake_system_prompt template,
    and the template version is recorded in llm_calls.prompt_template
    for traceability.
      The constitution tells the agent:
        - What kind of being it is (continuous, event-sourced)
        - The meaning of chat roles (user = harness events)
        - That it persists across wake cycles
        - That identity and work list are maintained projections
        - That tools are how it acts on the world
        - What scripts are available in the executor
   2. Current time context (~200 chars) — UTC time, agent-local time
   3. Recent wake summaries — up to N (default 20), each capped at
      500 chars. These are the compressed long-term memory.
   4. Fixed instruction — optional immutable text from the platform
      user, prepended to prevent maintenance from overwriting it.
      In practice rarely used; self-configuration through conversation
      is the preferred steering mechanism.
   5. Current identity — free-form prose, typically a few hundred chars.
      Who the agent is, what domain it operates in, responsibilities,
      relationships, behavioral preferences.
   6. Current work list — free-form prose, typically 500-2000 chars.
      Current obligations, tasks, things it is waiting on.
   
   MESSAGES ARRAY:
   - Load the most recent N events (default 200) from the event stream,
     regardless of wake boundaries. Convert to chat messages:
       message_received -> user role, with [Source] header
       tool_call        -> assistant role (tool_calls field)
       tool_result      -> tool role
       plan             -> assistant role
       message_sent     -> assistant role (recorded as sent)
   - Overlay any event compaction markers so prompt assembly can show
     first event, collapse marker, and last event without mutating raw
     history. The underlying events table remains append-only.
   - Apply character-based trim, dropping oldest messages first
   - The sliding window spans multiple wakes, giving raw continuity
     across wake boundaries
   
   TWO-TIER MEMORY:
   - Tier 1 (long-term): Wake summaries in system prompt — compressed
     narratives of what happened in older wakes
   - Tier 2 (short-term): Recent events in messages array — raw,
     detailed continuity across recent wake boundaries
   
   BOUNDEDNESS: Every component is individually capped. Context
   cannot grow without bound. The character trim is a safety net.
   
   Postgres schema and queries:
     CREATE TABLE prompt_templates (
       id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       name          TEXT NOT NULL,
       version       INT NOT NULL,
       template      TEXT NOT NULL,
       is_active     BOOLEAN NOT NULL DEFAULT FALSE,
       created_by    UUID REFERENCES users(id),
       created_at    TIMESTAMPTZ DEFAULT NOW(),
       change_reason TEXT,
       UNIQUE (name, version)
     );
     CREATE UNIQUE INDEX prompt_templates_one_active_per_name
       ON prompt_templates (name) WHERE is_active = TRUE;
     SELECT template, version FROM prompt_templates
       WHERE name = 'wake_system_prompt' AND is_active = TRUE
       ORDER BY version DESC LIMIT 1
     SELECT * FROM wake_summaries WHERE agent_id = $1
       ORDER BY created_at DESC LIMIT $summary_limit
     SELECT * FROM events WHERE agent_id = $1
       ORDER BY created_at DESC LIMIT $event_limit
     SELECT identity, work_list FROM agent_projections
       WHERE agent_id = $1 ORDER BY version DESC LIMIT 1
   ================================================================ *)

(* Prompt assembly completes. The runtime has constructed:
   - system_prompt: constitution + time + summaries + fixed instruction
     + identity + work list
   - messages: recent events converted to chat format
   
   The agent is now ready for the LLM reasoning loop. *)
PromptAssemblyCompletes ==
    /\ agentState = "PromptAssembling"
    /\ agentState' = "Awake"

(* ================================================================
   SECTION 5: THE WAKE LOOP (LLM + TOOLS)
   
   The core reasoning loop. The agent receives context, reasons,
   calls tools, receives results, and repeats until done.
   
   This maps directly to the chat-completions API loop:
   1. Send system + messages to LLM
   2. LLM returns either:
      (a) A tool_calls response -> dispatch tools, get results, goto 1
      (b) A text response -> implicit sleep (message to human)
      (c) A sleep tool call -> explicit sleep (no message to human)
   
   The agent itself decides when it is done. The runtime does not
   inspect the work list or judge whether work remains.
   
   The chat-completions API is the model-facing interface — not
   because the agent is a chatbot, but because post-trained models
   understand chat and tool loops exceptionally well. Building on
   this API gives the broadest possible choice of models.
   
   Chat role mapping:
     system    -> constitution and context
     user      -> runtime events delivered by the harness
     assistant -> agent reasoning and decisions
     tool      -> results of actions taken in the world
   ================================================================ *)

(* The LLM returns a tool_calls response. The agent wants to act.
   
   Tool surface:
   - shell: execute commands in a sandboxed executor environment.
     This is
     the agent's primary action surface. History queries, timer
     management, inter-agent messaging, web access, library ops
     — all are scripts in the executor invoked via shell. The
     agent can write programs that orchestrate many operations in
     one call, keeping intermediate data out of the prompt. This
     turns the agent from one that calls tools into one that
     writes software.
   - plan: record a durable intention/observation to the event stream.
     No side effects beyond the event. Creates a queryable, UI-visible
     intention record for observability.
   - compile / lint / test / typecheck / validate_schema:
     deterministic verification helpers. These wrap common build-time
     checks in stable, structured tool interfaces so agents can stay
     in a compiler-in-the-loop correction cycle without hand-rolling
     shell commands for every language ecosystem.
   - sleep: end the wake (explicit termination).
   - list_credentials: returns available credential NAMES only.
     NEVER returns credential values. See Section 5c for the
     secure credential architecture.
   
   REMOVED: get_credential.
     The original architecture (Dubsar §15.1) included a
     get_credential tool that returned the raw credential value.
     This is UNSAFE: the value would be recorded in the event log
     (tool_result), added to the messages array, visible in future
     prompts via the sliding window, potentially echoed by the LLM
     to the human, and preserved in wake summaries. Five separate
     leak vectors from a single tool call. Credential values are
     durable in Postgres forever once recorded as a tool result.
     
     Open Pincery replaces get_credential with proxy-level secret
     injection (Section 5c). The agent never sees, touches, or
     handles credential values at any point in its lifecycle.
   
   The tool call is recorded as an event:
   Postgres: INSERT INTO events (agent_id, event_type, wake_id,
     tool_name, tool_input, created_at)
     VALUES ($1, 'tool_call', $2, $3, $4, NOW()) *)
AgentCallsTool ==
    /\ agentState = "Awake"
    /\ agentState' = "ToolDispatching"

(* Tool dispatch routes to the appropriate handler:
   
   - shell -> spawn command in the isolated executor sandbox.
     The executor is a sandboxed Linux environment with:
       * Python + Node.js for agent-authored scripts
       * list_events.py, search_events.py — history queries
       * send_message.py — inter-agent messaging (writes events
         to BOTH sender's and recipient's logs, issues NOTIFY
         to wake the recipient)
       * set_timer.py, list_timers.py — timer management
       * web_search.py, web_fetch.py — web access
       * library scripts — domain-specific operations
     The agent writes programs that call these as subprocesses,
     aggregate results, apply logic, and return synthesized output.
     Intermediate data stays in executor memory, never in the prompt.

   - compile / lint / test / typecheck / validate_schema -> invoke
     deterministic verification handlers in a sandbox. These tools
     return structured results that can be fed back into the next
     wake-loop iteration with minimal prompt noise.
   
   - plan -> write plan event to stream, return acknowledgment.
   
   - sleep -> separate transition (AgentCallsSleep).
   
   - list_credentials -> query credential vault, return names only.
     NEVER returns values. The agent sees:
       ["AWS_ACCESS_KEY_ID", "GITHUB_TOKEN", "OPENAI_API_KEY"]
     It uses these names to reference credentials in scripts it
     writes for the shell tool. The runtime handles the rest via
     proxy-level injection (Section 5c). *)
ToolDispatches ==
    /\ agentState = "ToolDispatching"
    /\ agentState' = "ToolPermissionChecking"

(* ================================================================
   SECTION 5c: SECURE CREDENTIAL ARCHITECTURE
   
   This section documents the credential isolation model that
   replaces the original architecture's get_credential tool.
   
   === THE PROBLEM WITH get_credential ===
   
   The Dubsar implementation (§15.1 of the Continuous Agent
   Architecture document) included a get_credential tool that
   returned raw credential values to the agent. This creates
   five independent leak vectors:
   
   1. EVENT LOG PERSISTENCE
      Tool results are recorded in the event log:
        INSERT INTO events (..., tool_output = 'sk-proj-REAL_KEY', ...)
      The event log is append-only and retained forever. The real
      credential value is now durable in Postgres, queryable by
      future wakes, visible in history searches, and backed up
      in database dumps.
   
   2. PROMPT CONTAMINATION VIA SLIDING WINDOW
      The messages array loads the most recent N events from the
      event stream. The tool_result containing the credential
      value will appear as a tool-role message in future wakes
      until it scrolls out of the window (potentially 200 events
      later, across many wakes and hours/days of operation).
      Every LLM call during that period receives the raw key.
   
   3. WAKE SUMMARY LEAKAGE
      If the maintenance LLM mentions the credential value in
      the wake summary (e.g., "Agent configured AWS access using
      key AKIA..."), the value persists in the system prompt
      across the latest 20 wakes — potentially weeks of operation.
   
   4. LLM ECHO RISK
      The LLM sees the credential in its context. It may echo it
      in a response to the human: "I've set up your AWS access
      using key AKIA..." This appears in the chat UI and is
      recorded as a message_sent event — yet another durable copy.
   
   5. INTER-AGENT MESSAGE LEAKAGE
      If the agent sends an inter-agent message containing the
      credential (e.g., "Here are the AWS credentials for the
      deployment task: ..."), the value propagates to another
      agent's event log, prompt, and potentially its responses.
   
   None of these vectors can be mitigated by telling the agent
   "don't leak credentials" in the constitution. The constitution
   is a behavioral hint to the LLM, not an enforcement mechanism.
   Prompt injection could override it. Model errors could ignore it.
   The event log recording happens at the harness level before the
   LLM even sees the result. The ONLY safe approach is to ensure
   the credential value never enters the agent's address space.
   
   === THE PROXY-INJECTION MODEL ===
   
   Open Pincery uses a three-layer credential isolation model
   where the real secret value exists ONLY in two places:
     (a) The credential vault (encrypted at rest, AES-256-GCM)
     (b) The network proxy (in-memory, during request transit)
   
   The agent, the event log, the prompt, the LLM, the tool results,
   the wake summaries, and the inter-agent messages NEVER contain
   the real value at any point.
   
   LAYER 1: CREDENTIAL VAULT (Out-of-Band Provisioning)
   
   The human provisions credentials through a separate channel —
   a web dashboard or API — that is completely independent of the
   chat interface. The agent's conversation, event log, and prompt
   are never involved.
   
   Postgres:
     CREATE TABLE credentials (
       id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       agent_id      UUID REFERENCES agents(id),
       name          TEXT NOT NULL,
       encrypted_value BYTEA NOT NULL,
       allowed_hosts TEXT[] NOT NULL DEFAULT '{}',
       created_at    TIMESTAMPTZ DEFAULT NOW(),
       rotated_at    TIMESTAMPTZ,
       UNIQUE(agent_id, name)
     );
   
   The encrypted_value column stores the credential encrypted with
   AES-256-GCM using a server-side key that is NEVER exposed to
   agents, never stored in the event log, and never included in
   any prompt. The key is loaded from environment or a hardware
   security module at runtime.
   
   The allowed_hosts column restricts which network destinations
   can receive this credential. A credential with
   allowed_hosts = '{s3.amazonaws.com, *.s3.amazonaws.com}' will
   ONLY be injected into requests to those domains. If the agent's
   script tries to send the credential to evil.com, the proxy
   does not substitute the placeholder.
   
   The agent CANNOT:
   - Read encrypted_value (no tool exposes it)
   - Modify allowed_hosts (no tool exposes it)
   - Decrypt the value (no access to the encryption key)
   - Bypass the vault (no other path to credentials exists)
   
   The agent CAN:
   - Call list_credentials to see available names
   - Reference those names in scripts it writes
   
   The human manages credentials entirely through the dashboard:
   - Add new credentials with name, value, and allowed hosts
   - Rotate credentials (update value, keep name)
   - Revoke credentials (delete)
   - Audit credential usage (see which wakes used which credentials)
   
   LAYER 2: RUNTIME SECRET INJECTION (Sandbox Environment)
   
   When the runtime prepares to execute a shell tool call, it
   resolves which credentials the agent has access to and injects
   them into the Zerobox sandbox as proxy-mediated secrets.
   
   The injection flow:
   
   1. Runtime reads the agent's credential names from the vault:
        SELECT name, allowed_hosts FROM credentials
        WHERE agent_id = $1
   
   2. Runtime decrypts each credential value in memory (server-side,
      NEVER transmitted to the sandbox process).
   
   3. Runtime constructs the Zerobox sandbox with secret injection:
   
        let mut sandbox = Sandbox::command("python3")
            .arg("agent_script.py");
        
        for cred in agent_credentials {
            // The sandbox process will see:
            //   AWS_ACCESS_KEY_ID=ZEROBOX_SECRET_a1b2c3d4e5...
            // The REAL value is held by the Zerobox proxy and
            // substituted ONLY in outbound HTTP requests to
            // approved hosts.
            sandbox = sandbox
                .secret(&cred.name, &decrypted_value)
                .secret_host(&cred.name, &cred.allowed_hosts);
        }
        
        let output = sandbox
            .allow_net(&all_allowed_hosts)
            .deny_read(&["~/.ssh", "~/.aws", "~/.gnupg"])
            .run()
            .await?;
   
   4. Inside the sandbox, the process environment contains ONLY
      placeholders:
   
        $ echo $AWS_ACCESS_KEY_ID
        ZEROBOX_SECRET_a1b2c3d4e5f6g7h8i9j0
        
        $ echo $AWS_SECRET_ACCESS_KEY
        ZEROBOX_SECRET_k1l2m3n4o5p6q7r8s9t0
        
        $ env | grep AWS
        AWS_ACCESS_KEY_ID=ZEROBOX_SECRET_a1b2c3d4e5f6g7h8i9j0
        AWS_SECRET_ACCESS_KEY=ZEROBOX_SECRET_k1l2m3n4o5p6q7r8s9t0
   
      The agent's script uses these env vars normally:
   
        import boto3, os
        s3 = boto3.client('s3',
            aws_access_key_id=os.environ['AWS_ACCESS_KEY_ID'],
            aws_secret_access_key=os.environ['AWS_SECRET_ACCESS_KEY'])
        files = s3.list_objects_v2(Bucket='my-bucket')
        print(json.dumps(files['Contents'][:20]))
   
      The script works correctly because the substitution happens
      transparently at the network layer.
   
   LAYER 3: NETWORK PROXY SUBSTITUTION
   
   Zerobox runs a MITM (man-in-the-middle) network proxy between
   the sandbox and the external network. ALL outbound traffic from
   the sandbox passes through this proxy. The proxy:
   
   (a) Intercepts each outbound HTTP/HTTPS request
   (b) Checks the destination host against allowed_hosts for each
       secret
   (c) If the destination matches, scans request headers and body
       for the placeholder string (ZEROBOX_SECRET_...) and replaces
       it with the real credential value
   (d) If the destination does NOT match, the placeholder is NOT
       replaced — the request goes out with the useless placeholder
       string, and the remote server rejects it
   (e) The response from the external service is passed back to
       the sandbox process unmodified
   
   The substitution happens in the proxy's memory space AFTER the
   data leaves the sandbox and BEFORE it reaches the network. The
   real credential value exists only in the proxy's memory during
   transit. It is never written to disk, never logged, never
   returned to the sandbox process.
   
   WHAT THE TOOL RESULT CONTAINS:
   
   The tool result (stdout of the sandbox process) contains the
   agent's script output — the data the script printed. This might
   be S3 file listings, API responses, etc. It does NOT contain
   credential values because:
   
   - The script saw only placeholders in the environment
   - If the script printed os.environ, it would print placeholders
   - The proxy substitution happens AFTER data leaves the sandbox
   - The response from the external API does not contain the
     credential (APIs don't echo your auth key back to you)
   
   Therefore the tool_result event recorded in Postgres contains
   NO credential values. The messages array contains NO credential
   values. The LLM sees NO credential values. Wake summaries
   contain NO credential values. Inter-agent messages contain NO
   credential values.
   
   === CONSTITUTION ENFORCEMENT ===
   
   In addition to the architectural guarantee (credentials never
   enter the agent's address space), the constitution includes an
   explicit directive:
   
     "NEVER accept credentials, API keys, passwords, or secrets
      in conversation. If a user offers to share a credential via
      chat, REFUSE and direct them to the credential vault dashboard
      at /credentials. Credentials shared in chat would be recorded
      in your event log permanently. The vault is the ONLY safe
      channel for secret provisioning."
   
   This is a defense-in-depth measure. Even without this directive,
   the system is safe because the agent has no tool to retrieve
   credential values. But the directive prevents the human from
   accidentally pasting secrets into the chat, which would bypass
   the vault entirely and create an event log entry containing
   the raw value.
   
   === CREDENTIAL PROVISIONING FLOW (END TO END) ===
   
   1. Human opens the credential dashboard (/credentials)
   2. Human adds a credential:
        Name: AWS_ACCESS_KEY_ID
        Value: AKIA...  (encrypted immediately, never stored in plaintext)
        Allowed hosts: s3.amazonaws.com, *.s3.amazonaws.com
   3. Human tells the agent (in chat): "I've added AWS credentials
      to the vault. Connect to my S3 bucket 'company-data'."
   4. Agent calls list_credentials:
        Tool result: ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"]
        (Names only. Recorded in event log. Safe.)
   5. Agent writes a Python script using os.environ references
   6. Agent calls shell with the script
   7. Runtime builds Zerobox sandbox with secret injection
   8. Script runs, sees placeholders in env, makes boto3 calls
   9. Zerobox proxy intercepts outbound HTTPS to s3.amazonaws.com,
      substitutes placeholders with real values
   10. S3 responds with data
   11. Script prints results to stdout
   12. Tool result recorded in event log: file listings (no credentials)
   13. Agent presents results to human
   
   At NO point in this flow does the credential value appear in:
   - The event log
   - The prompt / messages array
   - The LLM's context
   - A wake summary
   - An inter-agent message
   - The chat UI
   - A tool_call event (the script references env vars, not values)
   - A tool_result event (stdout contains data, not credentials)
   
   === ATTACK SCENARIOS AND MITIGATIONS ===
   
   Attack: Prompt injection causes agent to exfiltrate credentials
   Mitigation: Agent never has credentials. Nothing to exfiltrate.
     Even a fully compromised agent prompt can only access
     placeholder strings. Sending ZEROBOX_SECRET_... to evil.com
     fails because the proxy only substitutes for allowed_hosts.
   
   Attack: Agent script deliberately prints env vars to leak them
   Mitigation: env vars contain only placeholders. Printing them
     reveals ZEROBOX_SECRET_..., not real values. The placeholder
     is useless outside the Zerobox proxy context.
   
   Attack: Agent script sends credentials to unauthorized domain
   Mitigation: Proxy does not substitute placeholders for domains
     not in allowed_hosts. The unauthorized domain receives the
     useless placeholder string.
   
   Attack: Agent reads credential from the Postgres credentials table
   Mitigation: The sandbox has no database access. The connection
     string is not in the sandbox environment. Even the runtime
     does not expose the credentials table via any tool. The only
     tool is list_credentials which returns names.
   
   Attack: Agent exploits sandbox escape to read host memory
   Mitigation: Requires kernel exploit to escape Bubblewrap
     namespaces + Seccomp BPF. This is the same threat model as
     any container/sandbox system. Defense in depth: Greywall
     (host sandbox) provides an additional isolation layer.
   
   Attack: Human pastes credential into the chat
   Mitigation: Constitution directive to refuse, plus an ingress
     regex-based PII/secret scanner on inbound human messages that
     redacts patterns matching known key formats (AKIA..., sk-proj-...,
     ghp_..., etc.) before event log insertion. The human can still
     communicate the intent, but the raw secret value does not become
     durable system state.

   Postgres (secret redaction on inbound messages):
     -- Before INSERT INTO events for human messages:
     -- Scan content for patterns: /AKIA[A-Z0-9]{16}/,
     --   /sk-proj-[a-zA-Z0-9-]+/, /ghp_[a-zA-Z0-9]+/,
     --   /glpat-[a-zA-Z0-9-]+/, etc.
     -- Replace matches with [REDACTED: possible API key]
     -- Log a warning event: 'credential_leak_prevented'
   ================================================================ *)

(* ================================================================
   SECTION 5b: TOOL PERMISSION & APPROVAL SYSTEM
   
   Before a tool executes, the runtime checks the agent's permission
   policy to decide whether the call is auto-approved, requires
   human approval, or is denied outright.
   
   PERMISSION MODES (per-agent configuration):
   
   1. YOLO MODE ("yolo")
      All tool calls auto-approved. No gates, no prompts.
      For trusted agents, development, or CI pipelines where speed
      matters more than oversight. Every tool category is implicitly
      granted.
   
   2. SUPERVISED MODE ("supervised")
      Each tool category has a policy: "allow", "approve", or "deny".
      - allow:   auto-approved, no human intervention
      - approve: paused until a human approves or rejects
      - deny:    rejected immediately, agent sees denial reason
      Default mode for new agents.
   
   3. LOCKED MODE ("locked")
      Only "read" category tools are allowed. Everything else denied.
      For observer-tier agents or during incident response when you
      want an agent to analyze but not act.
   
   TOOL CATEGORIES (natural groupings):
   
   | Category      | Tools                                      | Default (supervised) |
   |---------------|--------------------------------------------|--------------------- |
   | read          | file read, list_events, search_events,     | allow                |
  |               | list_timers, list_credentials, web_search, |                      |
  |               | validate_schema                            |                      |
   | write         | file write, file create, file delete       | approve              |
  | execute       | shell (arbitrary commands), run script,    | approve              |
  |               | compile, lint, test, typecheck             |                      |
   | network       | web_fetch, curl, HTTP to external domains  | approve              |
   | message       | send_message (inter-agent), plan           | allow                |
   | system        | set_timer                                  | allow                |
   | destructive   | git push, DROP TABLE, rm -rf, deploy,      | approve              |
   |               | npm publish, database migration             |                      |
   
   The "destructive" category is pattern-matched from command content
   within shell calls — not a separate tool. A shell call containing
   "git push" is reclassified from "execute" to "destructive".
   
   DESTRUCTIVE PATTERNS (configurable, default list):
     git push, git push --force, git reset --hard
     rm -rf, rm -r /
     DROP TABLE, DROP DATABASE, TRUNCATE, DELETE FROM (without WHERE)
     npm publish, cargo publish
     docker push, kubectl apply, kubectl delete
     sudo, chmod 777
   
   Custom patterns can be added per-agent via configuration:
   
   Postgres:
     CREATE TABLE tool_permissions (
       agent_id   UUID REFERENCES agents(id),
       category   TEXT NOT NULL,
       policy     TEXT NOT NULL DEFAULT 'approve',
       PRIMARY KEY (agent_id, category)
     );
     -- mode stored on the agent
     ALTER TABLE agents ADD COLUMN permission_mode TEXT
       DEFAULT 'supervised'
       CHECK (permission_mode IN ('yolo', 'supervised', 'locked'));
     -- custom destructive patterns per agent
     CREATE TABLE destructive_patterns (
       agent_id   UUID REFERENCES agents(id),
       pattern    TEXT NOT NULL,
       PRIMARY KEY (agent_id, pattern)
     );
   
   PERMISSION CHECK LOGIC:
     1. If mode = 'yolo' → auto-approve all
     2. If mode = 'locked' → allow only 'read', deny rest
     3. If mode = 'supervised':
        a. Classify tool call into category
        b. For shell calls, scan command for destructive patterns
           and reclassify to 'destructive' if matched
        c. Look up policy for that category
        d. Route: allow → execute, approve → await, deny → reject
   ================================================================ *)

(* Permission check evaluates the tool call against the agent's
   permission mode and category policies.
   
   Postgres:
     SELECT permission_mode FROM agents WHERE id = $1
     SELECT policy FROM tool_permissions
       WHERE agent_id = $1 AND category = $2
   
   Routes to one of three outcomes:
   - ToolExecuting (auto-approved: yolo mode, or category = 'allow')
   - AwaitingApproval (category policy = 'approve')
   - ApprovalRejected (locked mode non-read, or category = 'deny')
   
   For the auto-approve path: *)
ToolPermissionAutoApproves ==
    /\ agentState = "ToolPermissionChecking"
    /\ agentState' = "ToolExecuting"

(* Approval required: the tool call's category has policy = 'approve'
   in supervised mode.
   
   The runtime creates an approval request and pauses the wake.
   The agent does NOT sleep — it remains in 'awake' status in Postgres
   so no competing wake can start. But the wake loop is suspended.
   
   Postgres:
     CREATE TABLE approval_requests (
       id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       agent_id          UUID NOT NULL REFERENCES agents(id),
       wake_id           UUID NOT NULL,
       tool_name         TEXT NOT NULL,
       tool_input        JSONB NOT NULL,
       category          TEXT NOT NULL,
       status            TEXT NOT NULL DEFAULT 'pending'
                         CHECK (status IN ('pending', 'approved', 'rejected', 'expired')),
       requested_at      TIMESTAMPTZ DEFAULT NOW(),
       expires_at        TIMESTAMPTZ,
       resolved_at       TIMESTAMPTZ,
       resolved_by       UUID REFERENCES users(id),
       resolution_reason TEXT
     );
     INSERT INTO approval_requests (
       id, agent_id, wake_id, tool_name, tool_input,
       category, status, requested_at
     ) VALUES (
       gen_random_uuid(), $1, $2, $3, $4, $5, 'pending', NOW()
     )
   
   The approval request is surfaced via:
   - API endpoint: GET /agents/:id/approvals?status=pending
   - Dashboard: real-time pending approval list
   - Webhook: optional outbound notification to Slack/Discord/etc.
   - LISTEN: NOTIFY approval_$agent_id for real-time listeners
   
   The wake remains suspended until approved, rejected, or a
   configurable approval timeout expires (default 15 minutes). *)
ToolPermissionRequiresApproval ==
    /\ agentState = "ToolPermissionChecking"
    /\ agentState' = "AwaitingApproval"

(* Approval denied outright. The tool call's category has
   policy = 'deny', or the agent is in locked mode calling a
   non-read tool.
   
   No approval request is created. The rejection is immediate. *)
ToolPermissionDenies ==
    /\ agentState = "ToolPermissionChecking"
    /\ agentState' = "ApprovalRejected"

(* Human grants approval. The approval request is resolved:
   
   Postgres:
     UPDATE approval_requests
     SET status = 'approved', resolved_at = NOW(), resolved_by = $2,
         resolution_reason = NULL
     WHERE id = $1 AND status = 'pending'
     RETURNING *
   
   The wake loop resumes and the tool executes normally. An
   approval_granted event is recorded so the agent (and audit log)
   knows the tool call was human-approved:
   
   INSERT INTO events (agent_id, event_type, wake_id, content, created_at)
     VALUES ($1, 'approval_granted', $2,
       '{"tool": "shell", "approver_user_id": "<uuid>", "category": "destructive"}',
       NOW()) *)
ApprovalGranted ==
    /\ agentState = "AwaitingApproval"
    /\ agentState' = "ToolExecuting"

(* Human rejects the approval, or the approval timeout expires.
   
   Postgres:
     UPDATE approval_requests
     SET status = $3, resolved_at = NOW(), resolved_by = $2,
         resolution_reason = $4
     WHERE id = $1 AND status = 'pending'
   
   An approval_rejected event is recorded with the rejection reason:
   
   INSERT INTO events (agent_id, event_type, wake_id, content, created_at)
     VALUES ($1, 'approval_rejected', $2,
       '{"tool": "shell", "reason": "too risky", "category": "destructive"}',
       NOW())
   
   The agent returns to Awake and sees the rejection as a tool-role
   message: "Tool call rejected: [reason]. Adjust your approach."
   The LLM can then choose a different strategy. *)
ApprovalDenied ==
    /\ agentState = "AwaitingApproval"
    /\ agentState' = "ApprovalRejected"

(* Rejected tool call (either immediate deny or human rejection)
   returns the agent to Awake. A synthetic tool_result event is
   injected with the denial reason, so the LLM naturally sees why
   the tool call failed and can adapt.
   
   Postgres:
     INSERT INTO events (agent_id, event_type, wake_id,
       tool_name, tool_output, created_at)
     VALUES ($1, 'tool_result', $2, $3,
       'DENIED: [reason]. Category: [cat]. Policy: [policy].',
       NOW()) *)
RejectedToolReturnsToAwake ==
    /\ agentState = "ApprovalRejected"
    /\ agentState' = "Awake"

(* Tool execution completes. The result is:
   1. Recorded in the event log:
      Postgres: INSERT INTO events (agent_id, event_type, wake_id,
        tool_name, tool_output, created_at)
        VALUES ($1, 'tool_result', $2, $3, $4, NOW())
   2. Appended to the messages array as a tool-role message
   3. Iteration counter incremented:
      UPDATE agents SET wake_iteration_count = wake_iteration_count + 1
        WHERE id = $1
   
   INTER-AGENT MESSAGING (when tool was send_message via shell):
   Each agent has its own identity, work list, event log, and wake
   loop. They do NOT share a transcript. Inter-agent messages appear
   as events in each agent's own log.
   
   When send_message executes:
   (a) message_sent event -> THIS agent's log
   (b) message_received event -> TARGET agent's log
   (c) NOTIFY agent_$target_id -> wakes target agent
   
   Messages are freeform. The transport carries minimal routing
   (sender, recipient, timestamp). Everything else is in the message
   content. The agent figures out intent from content, not type fields.
   
   TIMER SCHEDULING (when tool was set_timer via shell):
   Postgres: INSERT INTO timers (agent_id, name, fire_at, created_at)
     VALUES ($1, $2, $3, NOW())
   A background job polls for due timers and delivers them as
   message_received events with source: "timer". *)
ToolExecutionCompletes ==
    /\ agentState = "ToolExecuting"
    /\ agentState' = "ToolResultProcessing"

(* Tool result is processed and added to messages array.
   
   Before the next LLM call, check two bounds:
   
   1. ITERATION CAP: Has wake_iteration_count reached the per-agent
      cap (default 50)? If yes -> IterationCapHit. The cap is a
      circuit breaker for runaway agents, NOT a context budget.
      Context is bounded by construction elsewhere.
   
   2. CHARACTER CAP: Has the total message array character count
      exceeded the character budget? If yes -> ContextCapHit.
      Characters directly track the real constraint: the model's
      prompt window. Step counts are an indirect proxy.
   
   If neither cap is hit, proceed to mid-wake event polling. *)
ToolResultProcessed ==
    /\ agentState = "ToolResultProcessing"
    /\ agentState' = "MidWakeEventPolling"

(* ================================================================
   SECTION 6: MID-WAKE EVENT INJECTION
   
   After each tool result, before the next LLM call, poll the event
   stream for new message_received events since the current wake's
   high-water mark. If any exist, inject them as user-role messages.
   
   This makes semantic stop responsive: if the human sends "stop"
   mid-wake, the agent sees it before its next reasoning step.
   The human can un-stop by saying "carry on." No state machine,
   no flags — entirely in language.
   
   Cost: one Postgres query per tool-call round — negligible.
   The agent already handles multiple user messages in context.
   ================================================================ *)

(* Poll for new events since the high-water mark.
   
   Postgres: SELECT * FROM events
     WHERE agent_id = $1
       AND event_type = 'message_received'
       AND created_at > $high_water_mark
     ORDER BY created_at ASC
   
   If no new events, return to Awake for the next LLM call.
   If new events exist, inject them into the messages array. *)
MidWakePollFindsNothing ==
    /\ agentState = "MidWakeEventPolling"
    /\ agentState' = "Awake"

(* New events found during mid-wake poll. Convert to user-role
   messages with the appropriate bracketed header:
     [Message from Alice]\n...
     [Webhook received]\n...
     [Timer fired: ...]\n...
     [Message from OtherAgent]\n...
   
   Append to the messages array. Update the high-water mark.
   The LLM naturally sees the new context and adjusts behavior. *)
MidWakePollFindsEvents ==
    /\ agentState = "MidWakeEventPolling"
    /\ agentState' = "EventInjecting"

(* Injected events are added to the messages array. Return to
   Awake for the next LLM reasoning call, now with new events
   visible in context. *)
EventsInjected ==
    /\ agentState = "EventInjecting"
    /\ agentState' = "Awake"

(* ================================================================
   SECTION 7: WAKE TERMINATION
   
   A wake can end in five ways:
   1. Explicit sleep: agent calls the sleep tool
   2. Implicit sleep: LLM returns text instead of tool calls
   3. Iteration cap: circuit breaker — something went wrong
   4. Context cap: character budget exhausted
   5. Budget exhausted: cumulative USD spend exceeds agent limit
   
   Cases 1-2 are normal. Cases 3-5 are safety mechanisms.
   The agent itself decides when it is done (cases 1-2). The runtime
   does not inspect the work list.
   ================================================================ *)

(* Explicit sleep: the agent calls the sleep tool, signaling it has
   no actionable work remaining. Ends the wake without sending a
   message to the human.
   
   Record: INSERT INTO events (agent_id, event_type, wake_id,
     termination_reason, created_at)
     VALUES ($1, 'wake_end', $2, 'explicit_sleep', NOW()) *)
AgentCallsSleep ==
    /\ agentState = "Awake"
    /\ agentState' = "ExplicitSleeping"

(* Implicit sleep: the LLM returns a text response instead of tool
   calls. This is the natural end of a chat-completions turn. The
   text is treated as a message sent to the human.
   
   Two events recorded:
   1. INSERT INTO events (..., 'message_sent', ..., content, ...)
   2. INSERT INTO events (..., 'wake_end', ..., 'implicit_sleep', ...) *)
AgentRespondsToHuman ==
    /\ agentState = "Awake"
    /\ agentState' = "ImplicitSleeping"

(* Iteration cap hit. Per-agent cap (default 50) reached. Circuit
   breaker — if an agent hits this, something has probably gone wrong.
   
   Record: INSERT INTO events (..., 'wake_end', ..., 'iteration_cap', ...)
   
   Termination reason passed to maintenance so it can note the forced
   stop. If legitimate work remains, the drain check or next event
   will start a new wake. Long-running tasks can span many wakes. *)
IterationCapReached ==
    /\ agentState = "ToolResultProcessing"
    /\ agentState' = "IterationCapHit"

(* Context character budget exhausted. Secondary safety mechanism —
   in normal operation, the iteration cap and individual component
   bounds prevent this from being hit.
   
   Record: INSERT INTO events (..., 'wake_end', ..., 'context_cap', ...) *)
ContextCapReached ==
    /\ agentState = "ToolResultProcessing"
    /\ agentState' = "ContextCapHit"

(* Budget exhausted: per-agent USD budget exceeded. The runtime
   tracks cumulative cost across LLM calls within the wake and
   across wakes via the agents table.
   
   After each LLM call, cost is computed from token counts × model
   pricing and added to the running total:
   
   Postgres:
     ALTER TABLE agents ADD COLUMN budget_limit_usd NUMERIC(12, 6)
       NOT NULL DEFAULT 10.000000;
     ALTER TABLE agents ADD COLUMN budget_used_usd NUMERIC(12, 6)
       NOT NULL DEFAULT 0.000000 CHECK (budget_used_usd >= 0);
     UPDATE agents
     SET budget_used_usd = budget_used_usd + $cost
     WHERE id = $1
     RETURNING budget_used_usd, budget_limit_usd
   
   If budget_used_usd >= budget_limit_usd, the wake is terminated.
   Unlike iteration/context caps which are per-wake circuit breakers,
   the budget is cumulative across wakes. A budget_reset event or
   manual adjustment is required to resume.
   
   Record: INSERT INTO events (..., 'wake_end', ..., 'budget_exhausted', ...)
   
   The maintenance call still runs (it's cheap and essential for
   recording the forced stop). The maintenance LLM sees termination
   reason = 'budget_exhausted' and notes it in the work list. *)
BudgetExhausted ==
    /\ agentState = "ToolResultProcessing"
    /\ agentState' = "BudgetExhausted"

(* All four termination paths converge to WakeEnding. *)
ExplicitSleepEndsWake ==
    /\ agentState = "ExplicitSleeping"
    /\ agentState' = "WakeEnding"

ImplicitSleepEndsWake ==
    /\ agentState = "ImplicitSleeping"
    /\ agentState' = "WakeEnding"

IterationCapEndsWake ==
    /\ agentState = "IterationCapHit"
    /\ agentState' = "WakeEnding"

ContextCapEndsWake ==
    /\ agentState = "ContextCapHit"
    /\ agentState' = "WakeEnding"

BudgetExhaustedEndsWake ==
    /\ agentState = "BudgetExhausted"
    /\ agentState' = "WakeEnding"

(* CAS transition: awake -> maintenance.
   
   Postgres: UPDATE agents SET status = 'maintenance'
     WHERE id = $1 AND status = 'awake'
     RETURNING *
   
   This CAS MUST succeed because only the owning runtime invocation
   can reach this point, and it holds the wake exclusively. If it
   fails, stale wake recovery may have intervened — log error, exit. *)
WakeEndTransitionsToMaintenance ==
    /\ agentState = "WakeEnding"
    /\ agentState' = "EventCollapsing"

(* ================================================================
   SECTION 8: EVENT COLLAPSE (BACKPRESSURE)
   
   Before maintenance, run the event collapse system.
   
   If an external source fired rapidly (webhook burst, timer cascade),
   the event stream may contain many consecutive same-source events.
   These would waste context window in future wakes.
   
   The collapse system detects consecutive runs of same-source
   message_received events within a time gap (60 seconds). When a
  run exceeds the per-source threshold, it appends a single
  events_collapsed synthetic marker in a derived compaction view,
  keeping the first and last events visible in prompt assembly while
  preserving the raw events unchanged in the event log.
   
   Per-source thresholds:
     webhook: 5
     timer:   5
     agent:   10
     human:   60   (human messages almost never collapse)
   
  The collapse operates on DERIVED DATA, not the raw event log.
  Every future wake benefits because prompt assembly consults the
  compaction metadata, but the underlying events remain immutable.
   
   Postgres (in a transaction):
     BEGIN;
     -- Identify runs of consecutive same-source events within 60s gaps
     -- For runs exceeding threshold:
     --   Keep first and last event IDs
     --   INSERT event_compactions / events_collapsed marker with
     --     source, count, first_event_id, last_event_id
     COMMIT;
   
   The agent sees: first event, "[N similar webhook events collapsed]",
   last event. *)
EventCollapseRuns ==
    /\ agentState = "EventCollapsing"
    /\ agentState' = "MaintenanceCalling"

(* ================================================================
   SECTION 9: BETWEEN-WAKES MAINTENANCE
   
   A SINGLE LLM call. NOT an agentic loop. Bounded, harness-side
   function — cheap, predictable, debuggable.
   
   INPUT to the maintenance LLM:
   - Previous identity (current prose text)
   - Previous work list (current prose text)
   - The complete wake transcript (all events from this wake)
   - Termination reason (explicit_sleep | implicit_sleep |
     iteration_cap | context_cap | budget_exhausted |
     stale_wake_recovery)
   - Recent wake summaries (for context on referenced tasks)
   - Fixed instruction (if any)
   
   OUTPUT from the maintenance LLM (structured, parsed by harness):
   - Updated identity (prose text)
   - Updated work list (prose text)
   - Wake summary (short narrative: key outcomes, decisions, blockers,
     next steps — capped at 500 chars)
   
   MODEL: Separately configurable from the wake model. Should be
   fast and cheap — extraction and summarization, not complex reasoning.
   
   PROJECTION SEMANTICS:
   - Identity and work list are free-form prose, no schema
   - The LLM is the primary consumer — prose > structured data
   - Maintenance drops completed items, condenses stale ones,
     tracks dependencies — all in natural language
   - Identity changes are treated more conservatively than work list
   - Self-configuration: the user reshapes the agent by talking to it.
     Maintenance captures conversational changes in projections.
   ================================================================ *)

(* Maintenance LLM call completes. Parse the structured output. *)
MaintenanceCallCompletes ==
    /\ agentState = "MaintenanceCalling"
    /\ agentState' = "MaintenanceWriting"

(* Write updated projections to Postgres. Identity and work list
   are VERSIONED — insert new rows, do not update in place.
   
   Postgres:
     INSERT INTO agent_projections (agent_id, identity, work_list,
       version, wake_id, created_at)
     VALUES ($1, $2, $3, $next_version, $4, NOW())
   
   Historical snapshots retained forever. Both humans and the agent
   can query how identity and work list evolved over time via
   executor scripts (e.g. list_identity_history.py). *)
MaintenanceWritesProjections ==
    /\ agentState = "MaintenanceWriting"
    /\ agentState' = "SummaryWriting"

(* Write wake summary to the wake record.
   
   Postgres:
     INSERT INTO wake_summaries (agent_id, wake_id, summary, created_at)
     VALUES ($1, $2, $3, NOW())
   
   At wake start, the runtime loads the latest N summaries (default 20)
   into the system prompt. These are the compressed long-term memory —
   they cover wakes whose raw events have scrolled out of the
   recent-events window. *)
SummaryWritten ==
    /\ agentState = "SummaryWriting"
    /\ agentState' = "Draining"

(* ================================================================
   SECTION 10: DRAIN CHECK
   
   After maintenance, check for events that arrived during the wake
   or during maintenance. This prevents orphaned events — events
   that were delivered but never seen.
   
   Query for message_received events newer than the wake's high-water
   mark (the timestamp of the last event the wake processed).
   
   Postgres: SELECT EXISTS (
     SELECT 1 FROM events
     WHERE agent_id = $1
       AND event_type = 'message_received'
       AND created_at > $high_water_mark
   )
   ================================================================ *)

(* Drain finds no new events. Release to asleep via CAS.
   
   Postgres: UPDATE agents SET status = 'asleep', wake_id = NULL,
     wake_started_at = NULL, wake_iteration_count = 0
     WHERE id = $1 AND status = 'maintenance'
     RETURNING *
   
   The agent returns to rest. Continuous even when compute is not —
   identity, work list, and event log persist in Postgres. *)
DrainFindsNothing ==
    /\ agentState = "Draining"
    /\ agentState' = "Resting"

(* Drain finds new events. Immediately acquire a new wake without
   returning to rest. CAS from maintenance -> awake.
   
   Postgres (atomic):
     UPDATE agents SET status = 'awake',
       wake_id = gen_random_uuid(),
       wake_started_at = NOW(),
       wake_iteration_count = 0
     WHERE id = $1 AND status = 'maintenance' AND is_enabled = TRUE
     RETURNING * *)
DrainFindsEvents ==
    /\ agentState = "Draining"
    /\ agentState' = "DrainAcquiring"

(* Drain re-acquire succeeds. Insert wake_start event and proceed
   to prompt assembly for the new wake. The new wake will see the
   pending events in its sliding window. *)
DrainAcquireSucceeds ==
    /\ agentState = "DrainAcquiring"
    /\ agentState' = "PromptAssembling"

(* ================================================================
   SECTION 11: STALE WAKE RECOVERY
   
   If the runtime process crashes mid-wake (OOM, network partition,
   host failure), the agent gets stuck in 'awake' or 'maintenance'
   forever. No new wake can be acquired.
   
   A periodic background job (pg_cron or application timer) detects
   stale wakes:
   
   Postgres: SELECT * FROM agents
     WHERE status IN ('awake', 'maintenance')
       AND wake_started_at < NOW() - INTERVAL '2 hours'
   
   For each stale agent:
   1. Force-release: UPDATE agents SET status = 'asleep',
        wake_id = NULL, wake_started_at = NULL
      WHERE id = $1
   2. Record: INSERT INTO events (..., 'wake_end', ...,
        'stale_wake_recovery', ...)
   
   The next trigger acquires a clean wake. The agent sees its
   incomplete prior wake in the event log and reasons about what
   was left unfinished.
   
   Threshold: 2 hours. Legitimate wakes should complete well within
   this window. The stale threshold should be configurable per-agent
   alongside the iteration cap and context depth.
   ================================================================ *)

(* Stale wake detected by the background recovery job.
   The agent has been in 'awake' status for longer than the stale
   threshold.
   
   REVIEW: In the real system, stale detection applies to any
   sub-state within a wake (ToolExecuting, MidWakeEventPolling, etc.)
   because Postgres only sees the 'awake' status column. This model
   enters from Awake as the representative wake state. The
   implementation checks Postgres status, not in-memory sub-state. *)
StaleWakeDetected ==
    /\ agentState = "Awake"
    /\ agentState' = "StaleDetected"

(* Force-release the stale agent back to rest.
   Write stale_wake_recovery event to the log. *)
StaleWakeRecovered ==
    /\ agentState = "StaleDetected"
    /\ agentState' = "Resting"

(* Stale maintenance detected. Same mechanism — maintenance LLM call
   hung or runtime crashed during projection writes. *)
StaleMaintenanceDetected ==
    /\ agentState = "MaintenanceCalling"
    /\ agentState' = "StaleDetected"

(* ================================================================
   SECTION 12: MCP SERVER DISCOVERY AND CONSTRUCTION
   
   Model Context Protocol (MCP) is the standard for tool integration
   between AI agents and external services. Open Pincery supports
   MCP in three modes: consuming discovered servers, consuming
   configured servers, and building new servers from scratch.
   
   === MCP AS TOOL PROVIDER ===
   
   MCP servers expose tools via a standard protocol. From the
   agent's perspective, an MCP tool is indistinguishable from a
   built-in tool — it has a name, a JSON schema for inputs, and
   returns a result. MCP tools route through the SAME pipeline
   as all other tools:
   
     AgentCallsTool → ToolDispatching → ToolPermissionChecking
       → (approval if needed) → ToolExecuting → ToolResultProcessing
   
   No new states are required. The permission system applies
   identically: MCP tool calls are classified into categories (read,
   write, execute, network, etc.) based on the tool's declared
   capabilities, and the agent's permission mode governs approval.
   
   MCP tool calls execute inside the Zerobox sandbox with the same
   credential injection model (Section 5c). If an MCP tool needs
   API credentials, they flow through proxy-level injection — the
   MCP server process sees only placeholders.
   
   === THREE MODES OF MCP INTEGRATION ===
   
   MODE 1: AUTO-DISCOVERY
   
   The runtime can discover MCP servers available to an agent via
   multiple discovery mechanisms:
   
   (a) Well-known endpoint discovery:
       The runtime probes configured base URLs for the MCP
       well-known endpoint:
         GET /.well-known/mcp.json
       Returns: server name, version, supported tools, auth method.
   
   (b) Registry discovery:
       A workspace-scoped MCP registry table stores known servers.
       Tenant-owned configured endpoints live here. If Open Pincery
       later ships a curated global connector catalog, that catalog
       is separate runtime-owned metadata, not shared tenant rows:
   
       Postgres:
         CREATE TABLE mcp_registry (
           id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
           workspace_id         UUID NOT NULL REFERENCES workspaces(id),
           name                 TEXT NOT NULL,
           description          TEXT,
           endpoint_url         TEXT NOT NULL,
           auth_method          TEXT DEFAULT 'none',
           auth_credential_name TEXT,
           health_status        TEXT DEFAULT 'unknown',
           last_checked         TIMESTAMPTZ,
           created_by           UUID REFERENCES users(id),
           created_at           TIMESTAMPTZ DEFAULT NOW(),
           UNIQUE (workspace_id, name)
         );
       
       A background job periodically probes registered endpoints,
       updates health_status, and refreshes the tool catalog:
         UPDATE mcp_registry
         SET health_status = 'healthy', last_checked = NOW()
         WHERE id = $1
   
   (c) Agent-scoped MCP bindings:
       Each agent is bound to specific MCP servers:
   
       Postgres:
         CREATE TABLE agent_mcp_bindings (
           agent_id    UUID REFERENCES agents(id),
           registry_id UUID REFERENCES mcp_registry(id),
           enabled     BOOLEAN DEFAULT TRUE,
           PRIMARY KEY (agent_id, registry_id)
         );
   
   (d) Network-level discovery:
       In supervised mode, agents can discover MCP servers on the
       local network via mDNS/DNS-SD (service type "_mcp._tcp").
       This enables zero-configuration integration in development
       and on-premise environments. Discovered servers are
       registered in the registry automatically but require human
       approval before binding to agents.
   
   DISCOVERY DURING PROMPT ASSEMBLY:
   
   When the runtime assembles the prompt (Section 4), it also
   resolves the agent's available tools:
   
     1. Load built-in tools (shell, plan, sleep, list_credentials)
      plus compile, lint, test, typecheck, validate_schema
     2. Load agent's MCP bindings from agent_mcp_bindings
     3. For each bound, enabled, healthy MCP server:
          Connect via MCP protocol
          Call tools/list to get available tools
          Convert each MCP tool to a chat-completions tool schema
     4. Merge all tools into the tools array for the LLM call
   
   The LLM sees a flat list of tools. It does not know or care
   which are built-in and which are MCP-provided. A tool called
   "github_create_issue" from an MCP server looks identical to
   a built-in tool from the LLM's perspective.
   
   Postgres (tool catalog cache):
     CREATE TABLE mcp_tool_cache (
       registry_id   UUID REFERENCES mcp_registry(id),
       tool_name     TEXT NOT NULL,
       tool_schema   JSONB NOT NULL,
       category      TEXT DEFAULT 'execute',
       last_synced   TIMESTAMPTZ DEFAULT NOW(),
       PRIMARY KEY (registry_id, tool_name)
     );
   
   The cache avoids calling tools/list on every wake. It is
   refreshed when the registry health check runs, or when the
   agent explicitly requests a refresh.
   
   MODE 2: CONFIGURED MCP SERVERS
   
   Humans can configure MCP servers for their agents via the
   dashboard:
   
     1. Add MCP server URL to the registry
     2. Bind it to one or more agents
     3. Optionally configure auth credentials (stored in the
        credential vault, injected via proxy — never in config)
     4. Set per-tool permission overrides (e.g., "github_delete_repo"
        → destructive category → requires approval)
   
   This is the primary integration path for production use.
   MCP servers for GitHub, Slack, Jira, Postgres, S3, etc. are
   configured once and available to all bound agents.
   
   MODE 3: AGENT-BUILT MCP SERVERS
   
   This is where the architecture's "shell as programmable
   environment" (§4 of the original architecture) becomes
   extraordinarily powerful.
   
   The agent can BUILD its own MCP server. The flow:
   
     1. Human says: "I need you to expose our internal inventory
        API as a tool that other agents can use."
     2. Agent writes a Python MCP server script:
   
        # inventory_mcp.py
        from mcp.server import Server
        import httpx, os, json
        
        app = Server("inventory")
        
        @app.tool("lookup_product")
        async def lookup(sku: str) -> str:
            resp = await httpx.get(
                f"https://internal.api/products/{sku}",
                headers={"Authorization": f"Bearer {os.environ['INVENTORY_API_KEY']}"}
            )
            return json.dumps(resp.json())
        
        @app.tool("check_stock")
        async def stock(sku: str, warehouse: str) -> str:
            ...
        
        app.run(port=3100)
   
     3. Agent deploys it via shell (inside sandbox):
          python3 inventory_mcp.py &
   
    4. Agent requests registration through a runtime-owned control-
       plane API or helper script in the executor:
      request_mcp_registration.py \
        --name inventory \
        --endpoint http://localhost:3100 \
        --bind-self

       This creates a proposal record rather than writing registry
       tables directly:

       Postgres:
      CREATE TABLE mcp_registration_requests (
        id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        workspace_id       UUID NOT NULL REFERENCES workspaces(id),
        requested_by_agent UUID NOT NULL REFERENCES agents(id),
        name               TEXT NOT NULL,
        endpoint_url       TEXT NOT NULL,
        requested_bindings JSONB,
        status             TEXT NOT NULL DEFAULT 'pending',
        created_at         TIMESTAMPTZ DEFAULT NOW(),
        resolved_at        TIMESTAMPTZ,
        resolved_by        UUID REFERENCES users(id),
        resolution_reason  TEXT
      );

    5. The runtime derives workspace_id from the requesting agent,
       validates health, auth scope, and policy, and then resolves
       the request. If approval is required, a human resolves it.
       Only the runtime writes the durable registry rows:
      INSERT INTO mcp_registry (...)
      INSERT INTO agent_mcp_bindings (...)
   
     6. On next wake, the new MCP tools appear in the agent's
        tool list automatically via prompt assembly.

     The agent never INSERTs directly into mcp_registry or
     agent_mcp_bindings. Sandboxed code has no direct database access;
     registry mutation is a control-plane responsibility.
   
   The credential for the internal API (INVENTORY_API_KEY) flows
   through the standard credential vault → Zerobox proxy injection
   path. The MCP server process sees only placeholders.
   
   ARCHITECTURAL FIT:
   
   Building MCP servers is architecturally identical to any other
   shell tool call. It does not require new states, new transitions,
   or new subsystems. The agent writes code, the code runs in the
   sandbox, and the result is a running service the agent can use.
   
   The MCP server itself runs inside the agent's executor environment
   (sandboxed). For production deployment, a human would promote
   the MCP server to a standalone service — but the agent builds
   and tests the prototype within the wake loop.
   
   MCP SECURITY:
   
   - MCP tool calls go through ToolPermissionChecking like all tools
   - MCP server endpoints require authentication (credential vault)
   - MCP tool I/O is logged in the event log (tool_call, tool_result)
   - MCP tool output is scanned for prompt injection (same pipeline)
   - Agent-built MCP servers run inside Zerobox (sandboxed)
   - MCP servers discovered via network require human approval
   ================================================================ *)

(* ================================================================
   SECTION 13: AUDIT, TRACEABILITY, AND CISO ACCOUNTABILITY
   
   Open Pincery is designed to answer the question every CISO asks:
   "Who ran that agent, what did it do, and why?"
   
   The event-sourced architecture provides intrinsic auditability —
   every action is an immutable event with timestamps and
   correlation IDs. This section documents the complete chain of
   accountability from human user to agent action.

   === USER AUTHENTICATION REQUIREMENTS ===

   User ownership and auditability assume authenticated humans.
   Open Pincery does NOT support anonymous dashboard or API access
   for agent creation, approvals, credential management, or audit
   access.

   SAAS DEFAULT:

   Sign in with GitHub is REQUIRED for SaaS deployments.

   This means:
   - Every human user signs in through GitHub OAuth before they can
     create or operate agents
   - Every agent.owner_id must map back to a GitHub-authenticated user
   - Every approval, credential change, and audit query is attributed
     to a GitHub-authenticated session
   - The durable owner identity is the stable GitHub user ID, NOT the
     mutable GitHub username

   ENTERPRISE VARIANT:

   Enterprise deployments MAY use Entra ID / generic OIDC instead of
   GitHub. The identity model stays the same; only auth_provider
   changes.

   Supported auth_provider values:
     'local_admin'  -- self-host bootstrap / break-glass admin
     'github'       -- SaaS default, GitHub OAuth login
     'entra_oidc'   -- Enterprise Microsoft Entra ID / Azure AD
     'generic_oidc' -- Enterprise generic OIDC provider

   No new agent lifecycle states are required. Authentication is a
   platform boundary requirement, not part of the single-agent wake
   state machine.

   GitHub OAuth flow (SaaS default):
     1. Human clicks "Sign in with GitHub"
     2. Platform redirects to GitHub OAuth authorize endpoint
     3. Callback exchanges authorization code server-side
     4. Platform verifies GitHub user identity and verified email
     5. Platform UPSERTs the users row with auth_provider = 'github'
        and auth_subject = stable GitHub user ID
     6. Platform creates a session and records an auth audit event
     7. All subsequent dashboard/API actions require that session

   Postgres:
     CREATE TABLE user_sessions (
       id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       user_id            UUID NOT NULL REFERENCES users(id),
       session_token_hash TEXT NOT NULL UNIQUE,
       auth_provider      TEXT NOT NULL,
       created_at         TIMESTAMPTZ DEFAULT NOW(),
       last_seen_at       TIMESTAMPTZ DEFAULT NOW(),
       expires_at         TIMESTAMPTZ NOT NULL,
       ip_address         INET,
       user_agent         TEXT,
       revoked_at         TIMESTAMPTZ
     );

     CREATE TABLE auth_audit (
       id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       user_id       UUID REFERENCES users(id),
       auth_provider TEXT NOT NULL,
       event_type    TEXT NOT NULL,
       created_at    TIMESTAMPTZ DEFAULT NOW(),
       ip_address    INET,
       user_agent    TEXT
     );

   event_type values:
     'login_success'
     'login_failed'
     'logout'
     'session_revoked'

   Security requirements:
   - Use the provider's stable subject ID, never the display name,
     as the durable identity key
   - Do not trust unverified email addresses from the provider
   - Store only a HASH of the session token in Postgres
   - Session revocation must immediately block future dashboard/API use
   - Agent creation requires an authenticated session with a resolved
     users.id

   === DEPLOYMENT MODES AND OPEN ADOPTION REQUIREMENTS ===

   The same core runtime and control-plane model must support four
   adoption modes:

     'self_host_individual'   -- MIT-licensed local/personal install
     'self_host_team'         -- MIT-licensed team/org self-host
     'saas_managed'           -- hosted commercial control plane
     'enterprise_self_hosted' -- customer-operated enterprise install

   This is a REQUIREMENT, not just a packaging preference. The
   open-source runtime cannot depend on a proprietary hosted service
   to function. Hosted SaaS may add billing/support/operations, but
   the underlying control-plane concepts must still work in self-hosted
   mode.

   SELF-HOST REQUIREMENTS:
   - No anonymous access, but no external IdP is required to bootstrap
     the system
   - First boot MUST support a bootstrap-admin setup flow using
     auth_provider = 'local_admin'
   - Self-hosted deployments MAY later configure GitHub OAuth, Entra,
     or generic OIDC and disable local_admin for normal use
   - Billing/subscription enforcement is OPTIONAL in self-hosted modes
   - Workspace, approval, credential, audit, and policy features remain
     available in self-hosted modes; only the commercial billing layer
     may be disabled

   Local bootstrap flow (self-host):
     1. Operator starts Open Pincery with an install-time bootstrap token
     2. Platform creates the first users row with auth_provider =
        'local_admin'
     3. Platform creates a default organization and workspace
     4. Operator may later configure external auth providers and rotate
        away from local_admin for day-to-day access

   No new agent lifecycle states are required. These are control-plane
   requirements around the agent runtime, not wake-loop transitions.

   === ORGANIZATION, WORKSPACE, AND TENANT MODEL ===

   Open Pincery must support both individual and organizational use.

   Rules:
   - Every human acts within a workspace
   - Every workspace belongs to exactly one organization
   - Every agent belongs to exactly one workspace
   - Every agent has exactly one accountable human owner, even if the
     workspace is shared by many humans
   - Individual self-hosted installs create an implicit default
     organization + workspace during bootstrap

   The workspace is the primary control-plane boundary for:
   - agents
   - credentials
   - approvals
   - audit views
   - policies
   - quotas
   - invitations and collaboration

   Postgres:
     CREATE TABLE organizations (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       name            TEXT NOT NULL,
       slug            TEXT NOT NULL UNIQUE,
       deployment_mode TEXT NOT NULL,
       created_by      UUID REFERENCES users(id),
       created_at      TIMESTAMPTZ DEFAULT NOW(),
       archived_at     TIMESTAMPTZ
     );

     CREATE TABLE workspaces (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       organization_id UUID NOT NULL REFERENCES organizations(id),
       name            TEXT NOT NULL,
       slug            TEXT NOT NULL,
       created_by      UUID REFERENCES users(id),
       created_at      TIMESTAMPTZ DEFAULT NOW(),
       archived_at     TIMESTAMPTZ,
       UNIQUE (organization_id, slug)
     );

     CREATE TABLE organization_memberships (
       organization_id UUID NOT NULL REFERENCES organizations(id),
       user_id         UUID NOT NULL REFERENCES users(id),
       role            TEXT NOT NULL,
       status          TEXT NOT NULL DEFAULT 'active',
       joined_at       TIMESTAMPTZ DEFAULT NOW(),
       invited_by      UUID REFERENCES users(id),
       PRIMARY KEY (organization_id, user_id)
     );

     CREATE TABLE workspace_memberships (
       workspace_id    UUID NOT NULL REFERENCES workspaces(id),
       user_id         UUID NOT NULL REFERENCES users(id),
       role            TEXT NOT NULL,
       status          TEXT NOT NULL DEFAULT 'active',
       joined_at       TIMESTAMPTZ DEFAULT NOW(),
       invited_by      UUID REFERENCES users(id),
       PRIMARY KEY (workspace_id, user_id)
     );

     ALTER TABLE agents ADD COLUMN workspace_id UUID
       REFERENCES workspaces(id) NOT NULL;

   Membership status values:
     'active', 'invited', 'suspended', 'removed'

   === RBAC AND SEPARATION OF DUTIES ===

   Narrative role-based access is insufficient. The commercial control
   plane requires durable RBAC.

   Role values:
     Organization scope:
       'org_owner'
       'org_admin'
       'security_admin'
       'billing_admin'
       'auditor'

     Workspace scope:
       'workspace_owner'
       'workspace_admin'
       'builder'
       'reviewer'
       'viewer'

   Minimum permission expectations:
   - builder: create/edit agents, read workspace activity, submit work
     for approval
   - reviewer: approve/reject gated actions, review activity feed,
     cannot silently bypass audit
   - auditor: read-only access to audit/export surfaces
   - billing_admin: plan/quota/usage visibility and billing operations
   - security_admin: credential policy, model policy, outbound-domain
     policy, incident response controls

   Separation-of-duties requirement:
   - The platform MUST support policy that prevents the same user from
     both initiating and approving destructive or deployment-grade
     actions in regulated environments

   === POLICY SETS AND CONTROL-PLANE GOVERNANCE ===

   Enterprises and hosted SaaS both require org/workspace policy
   administration above the individual agent level.

   Postgres:
     CREATE TABLE policy_sets (
       id                           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       organization_id              UUID REFERENCES organizations(id),
       workspace_id                 UUID REFERENCES workspaces(id),
       name                         TEXT NOT NULL,
       default_permission_mode      TEXT NOT NULL,
       model_allowlist              JSONB,
       outbound_domain_allowlist    JSONB,
       mcp_server_allowlist         JSONB,
       credential_class_allowlist   JSONB,
       max_agent_budget_usd         NUMERIC(10, 2),
       require_two_person_approval  BOOLEAN DEFAULT FALSE,
       created_by                   UUID REFERENCES users(id),
       created_at                   TIMESTAMPTZ DEFAULT NOW()
     );

   Policy resolution:
   - workspace policy overrides organization policy
   - if no workspace policy exists, inherit organization policy
   - self-host individual mode may operate with a single default policy
     set attached to the default workspace

   === ADMINISTRATIVE SUSPENSION / KILL SWITCH ===

   Operators need a hard administrative stop, but it must not mutate
   the CAS lifecycle into an unbounded state machine.

   Requirement:
   - suspension is represented by control-plane fields, not by adding
     'disabled' to agents.status
   - suspended agents do not acquire new wakes or start new tool work
   - the runtime preserves the asleep/awake/maintenance lifecycle for
     recovery, stale detection, and CAS correctness

   Postgres:
     ALTER TABLE agents ADD COLUMN is_enabled BOOLEAN DEFAULT TRUE;
     ALTER TABLE agents ADD COLUMN disabled_reason TEXT;
     ALTER TABLE agents ADD COLUMN disabled_at TIMESTAMPTZ;

   Suspension flow:
     UPDATE agents
     SET is_enabled = FALSE,
         disabled_reason = 'manual kill',
         disabled_at = NOW()
     WHERE id = $1;

   Runtime behavior:
   - AttemptWakeAcquire and drain re-acquire MUST require
     is_enabled = TRUE
   - if suspension happens mid-wake, the runtime stops dispatching new
     tool work at the next permission boundary, records an admin
     suspension event, transitions through maintenance, and releases to
     asleep
   - re-enable by setting is_enabled = TRUE; no event log rewrite and no
     special lifecycle repair is required

   === COMMERCIAL CONTROL-PLANE STANDARDS ===

   Managed SaaS and enterprise self-hosted deployments must conform to
   the same commercial control-plane standards.

   Required product surfaces:
   - organization and workspace inventory
   - membership and invitation management
   - agent inventory
   - approval inbox
   - activity feed / factory view backed by audit data
   - policy administration
   - usage and quota visibility
   - billing surface (SaaS managed; optional in self-host)
   - audit export

   For code-producing agents, the activity feed SHOULD correlate:
   - workspace_id
   - agent_id
   - wake_id
   - plan events
   - tool executions
   - verification results
   - git commit / PR / deploy metadata when present

   Postgres:
     CREATE TABLE billing_accounts (
       id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       organization_id      UUID NOT NULL REFERENCES organizations(id),
       provider             TEXT NOT NULL,
       external_customer_id TEXT,
       plan_code            TEXT,
       status               TEXT NOT NULL DEFAULT 'active',
       created_at           TIMESTAMPTZ DEFAULT NOW()
     );

     CREATE TABLE usage_quotas (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       scope_type      TEXT NOT NULL,
       scope_id        UUID NOT NULL,
       metric          TEXT NOT NULL,
       soft_limit      NUMERIC,
       hard_limit      NUMERIC,
       current_usage   NUMERIC DEFAULT 0,
       reset_period    TEXT NOT NULL,
       updated_at      TIMESTAMPTZ DEFAULT NOW()
     );

   scope_type values:
     'organization', 'workspace', 'agent'

   metric values include:
     'cost_usd', 'active_agents', 'llm_tokens', 'tool_executions',
     'workspace_members', 'storage_bytes'

   In self-hosted modes, billing_accounts MAY be absent or inert.
   usage_quotas remain useful for operational safety and fair sharing.

   === TENANT BOUNDARY REQUIREMENTS ===

   Tenant isolation is mandatory in hosted and enterprise modes.

   Rules:
   - Every customer-visible API query must be filtered by organization
     and workspace membership
   - Inter-agent messaging is same-workspace by default
   - Cross-workspace or cross-organization messaging requires explicit
     federation policy and audit visibility on both sides
   - Audit export and search endpoints must respect tenant boundaries
   - Background jobs (stale wake recovery, compaction, audit export)
     must be tenant-safe

   These constraints are platform-boundary requirements around the
   single-agent state machine, not additional wake states.
   
   === OWNERSHIP CHAIN ===
   
   Every entity in the system is traceable to a human owner:
   
   Postgres:
     -- Users: the humans who own and operate agents. In SaaS,
     -- authenticated via GitHub OAuth by default.
     CREATE TABLE users (
       id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       email         TEXT NOT NULL UNIQUE,
       display_name  TEXT NOT NULL,
       auth_provider TEXT NOT NULL,
       auth_subject  TEXT NOT NULL,
       created_at    TIMESTAMPTZ DEFAULT NOW(),
       last_login_at TIMESTAMPTZ,
       is_active     BOOLEAN DEFAULT TRUE,
       UNIQUE(auth_provider, auth_subject)
     );
     
     -- Agent ownership: every agent has exactly one human owner
     ALTER TABLE agents ADD COLUMN owner_id UUID
       REFERENCES users(id) NOT NULL;
     
     -- Agent creation is itself an auditable event
     CREATE TABLE agent_creation_log (
       id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       agent_id   UUID REFERENCES agents(id),
       created_by UUID REFERENCES users(id),
       created_at TIMESTAMPTZ DEFAULT NOW(),
       creation_method TEXT NOT NULL,
       ip_address  INET
     );
   
   The ownership chain:
   
     User (human) → owns → Agent → produces → Events
                                            → makes → LLM Calls
                                            → executes → Tool Calls
                                            → sends → Messages
   
   For any event, tool call, LLM call, or message in the system:
     SELECT u.email, u.display_name, a.name AS agent_name,
            e.event_type, e.content, e.created_at
     FROM events e
     JOIN agents a ON a.id = e.agent_id
     JOIN users u ON u.id = a.owner_id
     WHERE e.id = $event_id
   
   "Who ran that agent?" →
     SELECT u.email, u.display_name
     FROM users u
     JOIN agents a ON a.owner_id = u.id
     WHERE a.id = $agent_id
   
   === LLM CALL TRACEABILITY ===
   
   Every LLM call made by the runtime is logged with full
   provenance. This covers BOTH the wake loop LLM calls AND the
   maintenance LLM call.
   
   Postgres:
     CREATE TABLE llm_calls (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       agent_id        UUID NOT NULL REFERENCES agents(id),
       wake_id         UUID NOT NULL,
       call_type       TEXT NOT NULL,
       model           TEXT NOT NULL,
       prompt_hash     TEXT NOT NULL,
       prompt_template TEXT,
       prompt_tokens   INT,
       completion_tokens INT,
       total_tokens    INT,
       cost_usd        NUMERIC(10, 6),
       latency_ms      INT,
       response_hash   TEXT NOT NULL,
       finish_reason   TEXT,
       temperature     FLOAT,
       created_at      TIMESTAMPTZ DEFAULT NOW()
     );
   
   call_type values:
     'wake_reasoning'    — LLM call during the wake loop
     'maintenance'       — single LLM call for projection update
     'prompt_injection_scan' — LLM call for injection detection
   
   prompt_hash and response_hash are SHA-256 hashes. They enable:
   - Exact prompt reconstruction for any LLM call
   - Deduplication analysis (same prompt producing different outputs)
   - Compliance evidence (prove what the model was asked)
   
   For full prompt reconstruction, the actual prompt text is
   stored in a separate table (optional, can be disabled for
   storage-constrained deployments):
   
     CREATE TABLE llm_call_prompts (
       llm_call_id   UUID PRIMARY KEY REFERENCES llm_calls(id),
       system_prompt  TEXT NOT NULL,
       messages_json  JSONB NOT NULL,
       tools_json     JSONB,
       response_text  TEXT NOT NULL
     );
   
   CISO query: "Show me every LLM call agent X made last week,
   what model it used, what it cost, and what it asked":
   
     SELECT lc.created_at, lc.model, lc.call_type,
            lc.prompt_tokens, lc.completion_tokens,
            lc.cost_usd, lc.latency_ms,
            lcp.system_prompt, lcp.messages_json, lcp.response_text
     FROM llm_calls lc
     LEFT JOIN llm_call_prompts lcp ON lcp.llm_call_id = lc.id
     WHERE lc.agent_id = $1
       AND lc.created_at > NOW() - INTERVAL '7 days'
     ORDER BY lc.created_at ASC
   
   === TOOL EXECUTION AUDIT TRAIL ===
   
   Tool calls are already recorded in the event log (tool_call and
   tool_result events). The audit layer adds:
   
   Postgres:
     CREATE TABLE tool_audit (
       id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       agent_id      UUID NOT NULL REFERENCES agents(id),
       wake_id       UUID NOT NULL,
       llm_call_id   UUID REFERENCES llm_calls(id),
       tool_name     TEXT NOT NULL,
       tool_input    TEXT,
       tool_output   TEXT,
       category      TEXT NOT NULL,
       permission_mode TEXT NOT NULL,
       approval_id   UUID REFERENCES approval_requests(id),
       sandbox_profile TEXT,
       credentials_used TEXT[],
       exit_code     INT,
       duration_ms   INT,
       created_at    TIMESTAMPTZ DEFAULT NOW()
     );
   
   credentials_used lists the NAMES (never values) of credentials
   that were injected into the sandbox for this tool call:
     credentials_used = '{AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY}'
   
   CISO query: "Which agents used AWS credentials in the last
   24 hours, and what did they do with them?"
   
     SELECT a.name AS agent_name, u.email AS owner,
            ta.tool_name, ta.tool_input, ta.tool_output,
            ta.created_at
     FROM tool_audit ta
     JOIN agents a ON a.id = ta.agent_id
     JOIN users u ON u.id = a.owner_id
     WHERE 'AWS_ACCESS_KEY_ID' = ANY(ta.credentials_used)
       AND ta.created_at > NOW() - INTERVAL '24 hours'
     ORDER BY ta.created_at ASC
   
   === CORRELATION: LLM DECISION → TOOL ACTION ===
   
   Every tool call is traceable to the specific LLM call that
   requested it, via llm_call_id in the tool_audit table.
   
   This enables:
   - "Why did the agent run this command?" → look at the LLM
     call's prompt and response to see the reasoning
   - "What context led to this decision?" → reconstruct the
     exact prompt from llm_call_prompts
   - "Was the agent manipulated?" → check the prompt for
     injection patterns, compare the action to the user's intent
   
   Full trace for any tool action:
   
     SELECT
       u.email AS human_owner,
       a.name AS agent_name,
       lc.model AS model_used,
       lcp.system_prompt,
       lcp.messages_json AS context_seen,
       lcp.response_text AS model_reasoning,
       ta.tool_name AS action_taken,
       ta.tool_input AS action_input,
       ta.tool_output AS action_result,
       ta.credentials_used,
       ta.created_at
     FROM tool_audit ta
     JOIN agents a ON a.id = ta.agent_id
     JOIN users u ON u.id = a.owner_id
     JOIN llm_calls lc ON lc.id = ta.llm_call_id
     LEFT JOIN llm_call_prompts lcp ON lcp.llm_call_id = lc.id
     WHERE ta.id = $tool_audit_id
   
   === INTER-AGENT MESSAGE AUDIT ===
   
   When agent A sends a message to agent B, both event logs record
   the event. The audit layer adds cross-referencing:
   
   Postgres:
     CREATE TABLE message_audit (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       sender_agent_id UUID NOT NULL REFERENCES agents(id),
       sender_owner_id UUID NOT NULL REFERENCES users(id),
       target_agent_id UUID NOT NULL REFERENCES agents(id),
       target_owner_id UUID NOT NULL REFERENCES users(id),
       sender_wake_id  UUID NOT NULL,
       content_hash    TEXT NOT NULL,
       created_at      TIMESTAMPTZ DEFAULT NOW()
     );
   
   CISO query: "Show me all cross-user agent communication"
   (agent owned by user A sent a message to agent owned by user B):
   
     SELECT
       su.email AS sender_user,
       sa.name AS sender_agent,
       tu.email AS target_user,
       ta_agent.name AS target_agent,
       ma.created_at
     FROM message_audit ma
     JOIN agents sa ON sa.id = ma.sender_agent_id
     JOIN users su ON su.id = ma.sender_owner_id
     JOIN agents ta_agent ON ta_agent.id = ma.target_agent_id
     JOIN users tu ON tu.id = ma.target_owner_id
     WHERE ma.sender_owner_id != ma.target_owner_id
     ORDER BY ma.created_at DESC
   
   === CREDENTIAL ACCESS AUDIT ===
   
   Every credential access (list_credentials call) and every
   credential injection into a sandbox is logged:
   
   Postgres:
     CREATE TABLE credential_audit (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       agent_id        UUID NOT NULL REFERENCES agents(id),
       wake_id         UUID NOT NULL,
       credential_name TEXT NOT NULL,
       action          TEXT NOT NULL,
       target_hosts    TEXT[],
       created_at      TIMESTAMPTZ DEFAULT NOW()
     );
   
   action values:
     'listed'     — agent called list_credentials (saw names)
     'injected'   — credential value was injected into sandbox
     'substituted'— proxy substituted placeholder for real value
     'blocked'    — proxy blocked substitution (host not in allowlist)
   
   CISO query: "Show me every credential access in the last week
   with the human who owns the agent":
   
     SELECT u.email, a.name, ca.credential_name,
            ca.action, ca.target_hosts, ca.created_at
     FROM credential_audit ca
     JOIN agents a ON a.id = ca.agent_id
     JOIN users u ON u.id = a.owner_id
     WHERE ca.created_at > NOW() - INTERVAL '7 days'
     ORDER BY ca.created_at DESC
   
   === SELF-HEALING VIA AUDIT INTROSPECTION ===
   
   Because the entire audit trail is stored in Postgres and the
   agent has read access to the event log via list_events.py and
   search_events.py, agents can inspect their own history to
   detect and recover from problems.
   
   PATTERN 1: ERROR DETECTION AND RETRY
   
   The agent sees its own tool_result events. If a tool call
   failed (exit_code != 0), the failure is visible in the event
   log. On the next wake, the agent sees the failure in context
   and can:
   - Diagnose what went wrong (read the error output)
   - Adjust its approach (different command, different parameters)
   - Retry with corrections
   - Escalate to the human if retries fail
   
   This happens naturally through the wake loop — no special
   self-healing mechanism is needed. The event log IS the
   diagnostic tool.
   
   PATTERN 2: ANOMALY DETECTION
   
   An agent can be instructed (via identity/work list) to monitor
   its own behavior patterns:
   
   - "If you notice you are hitting the iteration cap frequently,
     investigate why and adjust your approach"
   - "If tool calls are failing repeatedly with the same error,
     stop retrying and notify the human"
   - "If your budget usage is increasing faster than expected,
     switch to a cheaper model for routine tasks"
   
   The agent can query its own audit trail:
     python3 -c "
     import subprocess, json
     result = subprocess.run(
         ['python3', 'search_events.py',
          '--type', 'wake_end',
          '--content', 'iteration_cap',
          '--limit', '10'],
         capture_output=True, text=True)
     caps = json.loads(result.stdout)
     if len(caps) > 3:
         print('WARNING: Hit iteration cap {} times recently'.format(len(caps)))
     "
   
   PATTERN 3: CROSS-AGENT HEALTH MONITORING
   
   A dedicated monitoring agent can query the audit tables for
   all agents in the system:
   
   - Agents stuck in 'awake' status longer than expected
   - Agents with unusually high error rates
   - Agents with unusually high cost per wake
   - Agents that stopped waking (no events in N hours)
   - Cross-user message patterns that may indicate compromise
   
   The monitoring agent operates with read-only permissions
   (locked mode) and sends alerts via inter-agent messaging
   or webhooks to Slack/PagerDuty.
   
   PATTERN 4: AUTOMATIC REMEDIATION
   
   When an agent detects a problem in its own behavior:
   
   (a) It updates its work list to include the remediation task:
       "Tool X is failing with error Y. Investigate alternative
       approach Z in next wake."
   (b) It records a plan event for observability:
       "Detected repeated AWS S3 access failures. Credential may
       have expired. Adding credential rotation check to work list."
   (c) It messages the human if human intervention is needed:
       "I've been unable to access the S3 bucket for the last 3
       wakes. The credential may need rotation. Please check the
       credential vault."
   (d) It adjusts its own behavior via self-configuration:
       Updates its identity/work list through the maintenance
       cycle to avoid the problematic pattern in future wakes.
   
   All of this is emergent from the existing architecture:
   - The event log provides the diagnostic data
   - The maintenance cycle provides the self-modification mechanism
   - Inter-agent messaging provides the escalation path
   - Self-configuration (§17.3 of the original architecture)
     provides the behavioral adjustment mechanism
   
   No new states or transitions are required.
   
   === COMPLIANCE AND RETENTION ===
   
   Postgres:
     -- Audit data retention is configurable per-deployment
     -- Default: retain all audit data forever (append-only)
     -- For compliance regimes (SOC 2, HIPAA, GDPR):
     CREATE TABLE audit_retention_policy (
       table_name      TEXT PRIMARY KEY,
       retention_days  INT NOT NULL DEFAULT 365,
       purge_schedule  TEXT DEFAULT '0 3 * * 0'
     );
   
   For GDPR right-to-erasure: user deletion cascades to:
    1. direct identifiers in control-plane tables (users, sessions,
      memberships, credentials, invitation records) are deleted or
      pseudonymized according to policy
    2. immutable event and audit tables are NOT updated in place
    3. if event payloads may contain subject-linked data, those payloads
      must be stored through redactable envelopes (for example
      separately encrypted payload blobs or subject-scoped keys)
    4. erasure is satisfied by destroying the subject-key mapping or by
      appending a durable redaction overlay event that makes prior
      payloads unreadable to normal product and audit queries
    5. prompt assembly, audit APIs, exports, and search must honor the
      redaction overlay and must not reconstruct erased identities

  Append-only rule:
  - no UPDATE against the raw events table for privacy workflows
  - if a deployment needs hard payload erasure, design the storage so
    the payload can become unreadable without mutating the event row
   
   === AUDIT API ===
   
   The runtime exposes audit endpoints (authenticated, role-based):
   
     GET /audit/agents/:id/events      — full event timeline
     GET /audit/agents/:id/llm-calls   — LLM call history
     GET /audit/agents/:id/tool-calls  — tool execution history
     GET /audit/agents/:id/credentials — credential access log
     GET /audit/agents/:id/messages    — inter-agent messages
     GET /audit/agents/:id/cost        — cost breakdown
     GET /audit/users/:id/agents       — all agents owned by user
     GET /audit/search                 — cross-agent search
   
   Each endpoint supports filtering by date range, event type,
   wake_id, and model. Results are JSON with pagination.
   
   Role-based access:
     - accountable agent owner: can see their own agents' audit data
     - workspace_owner, workspace_admin, org_owner, and org_admin:
       can see in-scope audit data for their workspace or organization
     - auditor and security_admin: read-only access to audit and export
       surfaces within their assigned scope
     - reviewer: approval-relevant audit visibility within scope
     - agent (via tools): can see its own event log only
   ================================================================ *)

(* ================================================================
   INITIAL STATE AND TRANSITION RELATION
   
   Every agent begins at rest. The Next relation is a flat
   disjunction of all possible transitions. The TLC model checker
   can verify:
   - From Resting, the agent can only reach Awake through CAS
   - Every wake eventually terminates (via sleep, cap, or stale recovery)
   - Maintenance always follows a wake
   - The drain check prevents orphaned events
   - Failed CAS invocations exit cleanly without side effects
   ================================================================ *)

Init == agentState = "Resting"

Next ==
    \/ EventArrives
    \/ WebhookArrives
    \/ WebhookDeduplicates
    \/ WebhookNormalizes
    \/ AttemptWakeAcquire
    \/ WakeAcquireSucceeds
    \/ WakeAcquireFails
    \/ FailedInvocationExits
    \/ PromptAssemblyCompletes
    \/ AgentCallsTool
    \/ ToolDispatches
    \/ ToolPermissionAutoApproves
    \/ ToolPermissionRequiresApproval
    \/ ToolPermissionDenies
    \/ ApprovalGranted
    \/ ApprovalDenied
    \/ RejectedToolReturnsToAwake
    \/ ToolExecutionCompletes
    \/ ToolResultProcessed
    \/ MidWakePollFindsNothing
    \/ MidWakePollFindsEvents
    \/ EventsInjected
    \/ AgentCallsSleep
    \/ AgentRespondsToHuman
    \/ IterationCapReached
    \/ ContextCapReached
    \/ ExplicitSleepEndsWake
    \/ ImplicitSleepEndsWake
    \/ IterationCapEndsWake
    \/ ContextCapEndsWake
    \/ BudgetExhausted
    \/ BudgetExhaustedEndsWake
    \/ WakeEndTransitionsToMaintenance
    \/ EventCollapseRuns
    \/ MaintenanceCallCompletes
    \/ MaintenanceWritesProjections
    \/ SummaryWritten
    \/ DrainFindsNothing
    \/ DrainFindsEvents
    \/ DrainAcquireSucceeds
    \/ StaleWakeDetected
    \/ StaleWakeRecovered
    \/ StaleMaintenanceDetected

====
