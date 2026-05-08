//! AC-82 (v9 Slice G7a) — DB-backed CAS round-trip tests for the
//! fine-grained lifecycle helpers added in `src/models/agent.rs`.
//!
//! Per scaffolding/readiness.md T-AC82-2: every fine-grained
//! transition is a single `UPDATE … WHERE id = $1 AND status = 'prev'`
//! returning `Some(agent)` if this caller won the race and `None`
//! otherwise. This file is the regression suite that pins that shape:
//!
//!   * driving the full canonical chain
//!     Resting → WakeAcquiring → PromptAssembling → Awake →
//!     ToolDispatching → ToolExecuting → ToolResultProcessing →
//!     MidWakeEventPolling → Awake → WakeEnding → Maintenance →
//!     Resting
//!     succeeds end-to-end with each helper returning `Some(agent)`,
//!   * each helper returns `None` when the agent is in any other
//!     state (no silent state jumps),
//!   * the terminal `enter_wake_ending` accepts every admissible
//!     "live wake" source state and rejects the rest.
//!
//! These tests do NOT exercise the wake-loop itself (G7b+); they
//! pin the model-layer CAS contract so subsequent slices can wire
//! events without re-litigating the state machine.

mod common;

use open_pincery::models::agent::{self, AgentStatus};
use open_pincery::models::{user, workspace};
use sqlx::PgPool;

async fn make_agent(pool: &PgPool, slug: &str) -> open_pincery::models::agent::Agent {
    let u = user::create_local_admin(pool, &format!("{slug}@test.com"), slug)
        .await
        .unwrap();
    let org = workspace::create_organization(pool, slug, slug, u.id)
        .await
        .unwrap();
    let ws = workspace::create_workspace(pool, org.id, slug, slug, u.id)
        .await
        .unwrap();
    agent::create_agent(pool, &format!("{slug}-agent"), ws.id, u.id)
        .await
        .unwrap()
}

/// Force the agent's `status` column to a specific value, bypassing
/// the CAS gates. Used only by the negative-path tests below to set
/// up "agent is in state X, helper-for-state-Y must refuse".
async fn force_status(pool: &PgPool, id: uuid::Uuid, status: AgentStatus) {
    sqlx::query("UPDATE agents SET status = $1 WHERE id = $2")
        .bind(status.as_db_str())
        .bind(id)
        .execute(pool)
        .await
        .expect("force_status must succeed (CHECK constraint widened in 20260507000001)");
}

/// G7a positive path: drive every CAS helper from its precondition
/// state through to the next, end-to-end, and assert every step
/// returned `Some(agent)` with the expected new status. This is the
/// minimum proof that the new helpers form a connected chain.
#[tokio::test]
async fn cas_helpers_round_trip() {
    let pool = common::test_pool().await;
    let a = make_agent(&pool, "ac82-g7a-rt").await;

    // 1. Resting → WakeAcquiring
    let r = agent::attempt_wake_acquire(&pool, a.id).await.unwrap();
    let r = r.expect("attempt_wake_acquire must succeed from Resting");
    assert_eq!(r.status, AgentStatus::DB_WAKE_ACQUIRING);
    assert!(r.wake_id.is_some(), "wake_id must be minted on entry");

    // 2. WakeAcquiring → PromptAssembling
    let r = agent::wake_acquire_succeeds(&pool, a.id).await.unwrap();
    assert_eq!(
        r.expect("WakeAcquiring → PromptAssembling").status,
        AgentStatus::DB_PROMPT_ASSEMBLING
    );

    // 3. PromptAssembling → Awake
    let r = agent::prompt_assembly_completes(&pool, a.id).await.unwrap();
    assert_eq!(
        r.expect("PromptAssembling → Awake").status,
        AgentStatus::DB_AWAKE
    );

    // 4. Awake → ToolDispatching
    let r = agent::enter_tool_dispatching(&pool, a.id).await.unwrap();
    assert_eq!(
        r.expect("Awake → ToolDispatching").status,
        AgentStatus::DB_TOOL_DISPATCHING
    );

    // 5. ToolDispatching → ToolExecuting
    let r = agent::enter_tool_executing(&pool, a.id).await.unwrap();
    assert_eq!(
        r.expect("ToolDispatching → ToolExecuting").status,
        AgentStatus::DB_TOOL_EXECUTING
    );

    // 6. ToolExecuting → ToolResultProcessing
    let r = agent::enter_tool_result_processing(&pool, a.id)
        .await
        .unwrap();
    assert_eq!(
        r.expect("ToolExecuting → ToolResultProcessing").status,
        AgentStatus::DB_TOOL_RESULT_PROCESSING
    );

    // 7. ToolResultProcessing → MidWakeEventPolling
    let r = agent::enter_mid_wake_event_polling(&pool, a.id)
        .await
        .unwrap();
    assert_eq!(
        r.expect("ToolResultProcessing → MidWakeEventPolling")
            .status,
        AgentStatus::DB_MID_WAKE_EVENT_POLLING
    );

    // 8. MidWakeEventPolling → Awake (loop back)
    let r = agent::mid_wake_poll_finds_nothing(&pool, a.id)
        .await
        .unwrap();
    assert_eq!(
        r.expect("MidWakeEventPolling → Awake").status,
        AgentStatus::DB_AWAKE
    );

    // 9. Awake → WakeEnding (terminal)
    let r = agent::enter_wake_ending(&pool, a.id).await.unwrap();
    assert_eq!(
        r.expect("Awake → WakeEnding").status,
        AgentStatus::DB_WAKE_ENDING
    );

    // 10. WakeEnding → Maintenance
    let r = agent::wake_end_transitions_to_maintenance(&pool, a.id)
        .await
        .unwrap();
    assert_eq!(
        r.expect("WakeEnding → Maintenance").status,
        AgentStatus::DB_MAINTENANCE
    );

    // 11. Maintenance → Resting (legacy helper, unchanged).
    let r = agent::release_to_asleep(&pool, a.id).await.unwrap();
    assert_eq!(
        r.expect("Maintenance → Resting").status,
        AgentStatus::DB_ASLEEP
    );
}

/// G7a negative path: every helper must return `None` when the agent
/// is not in its precondition state. This test runs each helper
/// against a Resting agent (or, where Resting *is* the precondition,
/// against an Awake agent) and asserts no row was returned. The
/// Resting baseline is sufficient because every helper except
/// `attempt_wake_acquire` requires a non-Resting precondition.
#[tokio::test]
async fn cas_helpers_refuse_wrong_state() {
    let pool = common::test_pool().await;
    let a = make_agent(&pool, "ac82-g7a-neg").await;
    // Agent is freshly created in `asleep`. Every helper that does
    // NOT have Resting as its precondition must refuse.
    assert!(agent::wake_acquire_succeeds(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::prompt_assembly_completes(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::enter_tool_dispatching(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::enter_tool_executing(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::enter_tool_result_processing(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::enter_mid_wake_event_polling(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::mid_wake_poll_finds_nothing(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::enter_wake_ending(&pool, a.id)
        .await
        .unwrap()
        .is_none());
    assert!(agent::wake_end_transitions_to_maintenance(&pool, a.id)
        .await
        .unwrap()
        .is_none());

    // Force the agent to Awake; now `attempt_wake_acquire` must
    // refuse (its precondition is Resting).
    force_status(&pool, a.id, AgentStatus::Awake).await;
    assert!(agent::attempt_wake_acquire(&pool, a.id)
        .await
        .unwrap()
        .is_none());
}

/// G7a: `enter_wake_ending` is the only multi-source helper. Its
/// admissible source set is exactly `{Awake, ToolDispatching,
/// ToolExecuting, ToolResultProcessing, MidWakeEventPolling}`. This
/// test pins both directions: every admissible source succeeds, and
/// the inadmissible sources `{Resting, WakeAcquiring,
/// PromptAssembling, WakeEnding, Maintenance}` are refused.
#[tokio::test]
async fn enter_wake_ending_admissible_sources() {
    let pool = common::test_pool().await;
    let a = make_agent(&pool, "ac82-g7a-wke").await;

    for src in [
        AgentStatus::Awake,
        AgentStatus::ToolDispatching,
        AgentStatus::ToolExecuting,
        AgentStatus::ToolResultProcessing,
        AgentStatus::MidWakeEventPolling,
    ] {
        force_status(&pool, a.id, src).await;
        let r = agent::enter_wake_ending(&pool, a.id).await.unwrap();
        let r = r.unwrap_or_else(|| panic!("enter_wake_ending must accept {src:?}"));
        assert_eq!(r.status, AgentStatus::DB_WAKE_ENDING);
        // Reset for the next iteration.
        force_status(&pool, a.id, AgentStatus::Resting).await;
    }

    for src in [
        AgentStatus::Resting,
        AgentStatus::WakeAcquiring,
        AgentStatus::PromptAssembling,
        AgentStatus::WakeEnding,
        AgentStatus::Maintenance,
    ] {
        force_status(&pool, a.id, src).await;
        let r = agent::enter_wake_ending(&pool, a.id).await.unwrap();
        assert!(r.is_none(), "enter_wake_ending must refuse {src:?}");
    }
}

// ---------------------------------------------------------------------------
// AC-82 review-fix (Critical): T-AC82-5 end-to-end DB scenarios.
//
// Drives the real `run_wake_loop` against a wiremock LLM endpoint and
// reads `events.event_type = 'lifecycle_transition'` rows for the
// driven wake_id, asserting the five invariants from readiness.md
// T-AC82-5:
//
//   1. The emitted canonical_action sequence matches a static chain
//      drawn from AC-82's pipe-list in spec_coverage.md.
//   2. The chain covers the agent's entire timeline (no missing CAS
//      sites — exactly one event per fine-grained CAS).
//   3. Successive (prev_to, next_from) pairs agree byte-for-byte —
//      no scope-reduction by hard-coding `from = 'awake'` (this is
//      what catches Required #1 mechanically).
//   4. Exactly one row per CAS — no dup-emits, no missed CASes.
//   5. Exactly one TerminalEndsWake event per wake (this is the
//      runtime witness for Inv_TerminalSuccession).
//
// We additionally read back the `content` column and parse it as
// canonical JSON to confirm byte-stable shape and key ordering as
// pinned by `src/runtime/lifecycle.rs`.
// ---------------------------------------------------------------------------

use open_pincery::config::Config;
use open_pincery::models::event;
use open_pincery::runtime::{
    llm::LlmClient,
    sandbox::{ProcessExecutor, ToolExecutor},
    vault::Vault,
    wake_loop,
};
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn lifecycle_test_config() -> Config {
    Config {
        database_url: String::new(),
        host: "127.0.0.1".into(),
        port: 0,
        bootstrap_token: "test".into(),
        llm_api_base_url: String::new(),
        llm_api_key: "fake".into(),
        llm_model: "test-model".into(),
        llm_maintenance_model: "test-model".into(),
        max_prompt_chars: 100_000,
        iteration_cap: 50,
        schema_invalid_retry_cap: 3,
        tool_call_rate_limit_per_wake: 32,
        stale_wake_hours: 2,
        wake_summary_limit: 20,
        event_window_limit: 200,
        vault_key_b64: common::TEST_VAULT_KEY_B64.into(),
        sandbox: open_pincery::config::ResolvedSandboxMode::default(),
    }
}

/// Read all `lifecycle_transition` events for one wake, ordered by
/// id (insertion order). Returns (canonical_action, from, to) tuples
/// extracted from the canonical-JSON `content` column.
async fn read_transitions(pool: &PgPool, wake_id: uuid::Uuid) -> Vec<(String, String, String)> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT content FROM events
         WHERE wake_id = $1 AND event_type = 'lifecycle_transition'
         ORDER BY created_at, ctid",
    )
    .bind(wake_id)
    .fetch_all(pool)
    .await
    .expect("read lifecycle_transition rows");
    rows.into_iter()
        .map(|(content,)| {
            let v: serde_json::Value =
                serde_json::from_str(&content).expect("content is canonical JSON");
            let action = v["canonical_action"].as_str().unwrap().to_string();
            let from = v["from"].as_str().unwrap().to_string();
            let to = v["to"].as_str().unwrap().to_string();
            (action, from, to)
        })
        .collect()
}

/// T-AC82-5 scenario: one-tool-call wake terminating via `sleep`.
/// Exercises the entry chain, one iteration of the tool loop, and
/// the Sleep early-return terminal arm. Asserts the canonical
/// transition chain matches AC-82's pipe-list and that
/// Inv_TerminalSuccession holds (exactly one TerminalEndsWake).
#[tokio::test]
async fn wake_loop_emits_canonical_lifecycle_chain_for_sleep_terminal() {
    let pool = common::test_pool().await;
    let a = make_agent(&pool, "lifecycle-sleep").await;

    event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("hello"),
        None,
    )
    .await
    .unwrap();

    // Drive the listener-side entry CAS so we have a wake_id.
    // run_wake_loop expects status = wake_acquiring and emits
    // WakeAcquireSucceeds + PromptAssemblyCompletes itself.
    let acquired = agent::attempt_wake_acquire(&pool, a.id)
        .await
        .unwrap()
        .unwrap();
    let wake_id = acquired.wake_id.unwrap();

    // LLM returns a single `sleep` tool_call.
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "test-1",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "sleep", "arguments": "{}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
        })))
        .mount(&mock)
        .await;

    let config = lifecycle_test_config();
    let llm = LlmClient::new(
        mock.uri(),
        "fake-key".into(),
        "test-model".into(),
        "test-model".into(),
    );
    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());
    let reason = wake_loop::run_wake_loop(&pool, &llm, &config, a.id, wake_id, &executor, &vault)
        .await
        .unwrap();
    assert_eq!(reason, "sleep");

    let chain = read_transitions(&pool, wake_id).await;

    // --- Invariant #1 + #4: exact canonical chain, one event per CAS.
    let expected: Vec<(&str, &str, &str)> = vec![
        // Entry chain (run_wake_loop top): wake_acquiring -> prompt_assembling -> awake
        ("WakeAcquireSucceeds", "wake_acquiring", "prompt_assembling"),
        ("PromptAssemblyCompletes", "prompt_assembling", "awake"),
        // Tool loop iteration #1: awake -> tool_dispatching -> tool_executing -> tool_result_processing
        ("ToolDispatches", "awake", "tool_dispatching"),
        ("AuthorizeExecution", "tool_dispatching", "tool_executing"),
        (
            "ReceiveToolResult",
            "tool_executing",
            "tool_result_processing",
        ),
        // Sleep early-return terminal: tool_result_processing -> wake_ending -> maintenance
        ("TerminalEndsWake", "tool_result_processing", "wake_ending"),
        (
            "WakeEndTransitionsToMaintenance",
            "wake_ending",
            "maintenance",
        ),
    ];
    assert_eq!(
        chain.len(),
        expected.len(),
        "AC-82 T-AC82-5 invariant #4: exactly one lifecycle_transition per CAS. Got chain: {chain:?}"
    );
    for (i, ((a_act, a_from, a_to), (e_act, e_from, e_to))) in
        chain.iter().zip(expected.iter()).enumerate()
    {
        assert_eq!(
            a_act, e_act,
            "step {i} canonical_action mismatch: chain={chain:?}"
        );
        assert_eq!(a_from, e_from, "step {i} from mismatch: chain={chain:?}");
        assert_eq!(a_to, e_to, "step {i} to mismatch: chain={chain:?}");
    }

    // --- Invariant #3: (prev.to == next.from) chain agreement.
    for i in 1..chain.len() {
        assert_eq!(
            chain[i - 1].2,
            chain[i].1,
            "AC-82 T-AC82-5 invariant #3 (prev.to == next.from) violated at step {i}: chain={chain:?}"
        );
    }

    // --- Invariant #5 (Inv_TerminalSuccession): exactly one TerminalEndsWake.
    let terminal_count = chain
        .iter()
        .filter(|(act, _, _)| act == "TerminalEndsWake")
        .count();
    assert_eq!(
        terminal_count, 1,
        "AC-82 Inv_TerminalSuccession violated: expected exactly one TerminalEndsWake, got {terminal_count}: chain={chain:?}"
    );
    let term_to_maint = chain
        .iter()
        .filter(|(act, _, _)| act == "WakeEndTransitionsToMaintenance")
        .count();
    assert_eq!(
        term_to_maint, 1,
        "AC-82 Inv_TerminalSuccession violated: expected exactly one WakeEndTransitionsToMaintenance, got {term_to_maint}"
    );
}

/// T-AC82-5 scenario: iteration-cap termination through the bottom
/// terminal block (NOT the Sleep early-return arm). This is the
/// scenario that surfaces Required #1 (false-`from` regression):
/// the bottom block must record the actual prior status (`awake`
/// for the iteration_cap path), not a hard-coded label. Combined
/// with invariant #3 above, any future regression of the bottom
/// block's `from` field will fail the chain-agreement assertion.
#[tokio::test]
async fn wake_loop_iteration_cap_terminal_records_actual_prior_status() {
    let pool = common::test_pool().await;
    let a = make_agent(&pool, "lifecycle-iter-cap").await;

    event::append_event(
        &pool,
        a.id,
        "message_received",
        "human",
        None,
        None,
        None,
        None,
        Some("hello"),
        None,
    )
    .await
    .unwrap();

    let acquired = agent::attempt_wake_acquire(&pool, a.id)
        .await
        .unwrap()
        .unwrap();
    let wake_id = acquired.wake_id.unwrap();

    // Pre-set the iteration count to the cap so the loop terminates
    // at the top of its first iteration (after entry chain) without
    // calling the LLM. The break path lands the agent in `awake`,
    // and the bottom-block `enter_wake_ending` must record `from =
    // 'awake'` (NOT a hard-coded label).
    for _ in 0..3 {
        agent::increment_iteration(&pool, a.id).await.unwrap();
    }

    let mock = MockServer::start().await;
    let config = Config {
        iteration_cap: 3,
        ..lifecycle_test_config()
    };
    let llm = LlmClient::new(
        mock.uri(),
        "fake-key".into(),
        "test-model".into(),
        "test-model".into(),
    );
    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap());
    let reason = wake_loop::run_wake_loop(&pool, &llm, &config, a.id, wake_id, &executor, &vault)
        .await
        .unwrap();
    assert_eq!(reason, "iteration_cap");

    let chain = read_transitions(&pool, wake_id).await;

    // Entry chain + bottom terminal chain. No tool-loop events
    // because iteration cap hits before the tool loop runs.
    let expected: Vec<(&str, &str, &str)> = vec![
        ("WakeAcquireSucceeds", "wake_acquiring", "prompt_assembling"),
        ("PromptAssemblyCompletes", "prompt_assembling", "awake"),
        ("TerminalEndsWake", "awake", "wake_ending"),
        (
            "WakeEndTransitionsToMaintenance",
            "wake_ending",
            "maintenance",
        ),
    ];
    assert_eq!(
        chain.len(),
        expected.len(),
        "iteration_cap chain length mismatch: chain={chain:?}"
    );
    for (i, ((a_act, a_from, a_to), (e_act, e_from, e_to))) in
        chain.iter().zip(expected.iter()).enumerate()
    {
        assert_eq!(a_act, e_act, "step {i} action: chain={chain:?}");
        assert_eq!(a_from, e_from, "step {i} from (Required #1 regression check — bottom block must record actual prior status): chain={chain:?}");
        assert_eq!(a_to, e_to, "step {i} to: chain={chain:?}");
    }

    // (prev.to == next.from) — would catch any false `from`
    // hard-code.
    for i in 1..chain.len() {
        assert_eq!(
            chain[i - 1].2,
            chain[i].1,
            "chain agreement violated at step {i}: chain={chain:?}"
        );
    }
}
