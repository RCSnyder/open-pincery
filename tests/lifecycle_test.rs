mod common;

use open_pincery::models::agent;
use open_pincery::models::user;
use open_pincery::models::workspace;

/// AC-1: CAS lifecycle — asleep → awake → maintenance → asleep
#[tokio::test]
async fn test_cas_lifecycle_happy_path() {
    let pool = common::test_pool().await;

    // Setup: create user, org, workspace, agent
    let u = user::create_local_admin(&pool, "test@test.com", "Test").await.unwrap();
    let org = workspace::create_organization(&pool, "test", "test", u.id).await.unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "test", "test", u.id).await.unwrap();
    let a = agent::create_agent(&pool, "test-agent", ws.id, u.id).await.unwrap();

    assert_eq!(a.status, "asleep");

    // Acquire wake: asleep → awake
    let acquired = agent::acquire_wake(&pool, a.id).await.unwrap();
    assert!(acquired.is_some());
    let awake = acquired.unwrap();
    assert_eq!(awake.status, "awake");
    assert!(awake.wake_id.is_some());

    // Double acquire should fail (CAS)
    let double = agent::acquire_wake(&pool, a.id).await.unwrap();
    assert!(double.is_none());

    // Transition to maintenance: awake → maintenance
    let maint = agent::transition_to_maintenance(&pool, a.id).await.unwrap();
    assert!(maint.is_some());
    assert_eq!(maint.unwrap().status, "maintenance");

    // Release to asleep: maintenance → asleep
    let released = agent::release_to_asleep(&pool, a.id).await.unwrap();
    assert!(released.is_some());
    let asleep = released.unwrap();
    assert_eq!(asleep.status, "asleep");
    assert!(asleep.wake_id.is_none());
}

/// AC-1: Invalid transitions should fail
#[tokio::test]
async fn test_cas_invalid_transitions() {
    let pool = common::test_pool().await;

    let u = user::create_local_admin(&pool, "test2@test.com", "Test2").await.unwrap();
    let org = workspace::create_organization(&pool, "test2", "test2", u.id).await.unwrap();
    let ws = workspace::create_workspace(&pool, org.id, "test2", "test2", u.id).await.unwrap();
    let a = agent::create_agent(&pool, "test-agent-2", ws.id, u.id).await.unwrap();

    // Can't transition to maintenance from asleep
    let bad = agent::transition_to_maintenance(&pool, a.id).await.unwrap();
    assert!(bad.is_none());

    // Can't release from asleep
    let bad = agent::release_to_asleep(&pool, a.id).await.unwrap();
    assert!(bad.is_none());
}
