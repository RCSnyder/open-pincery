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
