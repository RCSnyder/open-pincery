//! AC-34 (v6): round-trip tests for `AgentStatus` <-> DB string conversion.
//!
//! These are pure unit tests — no DB required. They pin the invariant that
//! `from_db_str` / `as_db_str` form a bijection for the five valid values
//! and that the DB strings match the TLA+ specification names exactly.

use open_pincery::models::agent::AgentStatus;

#[test]
fn round_trip_all_variants() {
    for s in [
        AgentStatus::Resting,
        AgentStatus::WakeAcquiring,
        AgentStatus::Awake,
        AgentStatus::WakeEnding,
        AgentStatus::Maintenance,
    ] {
        let db = s.as_db_str();
        let back = AgentStatus::from_db_str(db)
            .unwrap_or_else(|| panic!("from_db_str({db:?}) must round-trip"));
        assert_eq!(s, back, "round-trip mismatch for {s:?} via {db:?}");
    }
}

#[test]
fn db_strings_match_tla_spec_names() {
    // If these strings change, migration 20260420000001 and the agents
    // CHECK constraint must change with them. Pin them here so drift is
    // a compile/test failure, not a runtime corruption.
    assert_eq!(AgentStatus::DB_ASLEEP, "asleep");
    assert_eq!(AgentStatus::DB_WAKE_ACQUIRING, "wake_acquiring");
    assert_eq!(AgentStatus::DB_AWAKE, "awake");
    assert_eq!(AgentStatus::DB_WAKE_ENDING, "wake_ending");
    assert_eq!(AgentStatus::DB_MAINTENANCE, "maintenance");
}

#[test]
fn unknown_db_string_returns_none() {
    assert!(AgentStatus::from_db_str("").is_none());
    assert!(AgentStatus::from_db_str("ASLEEP").is_none());
    assert!(AgentStatus::from_db_str("sleeping").is_none());
    assert!(AgentStatus::from_db_str("wake").is_none());
}

#[test]
fn as_db_str_is_const() {
    // Compile-time proof that as_db_str is a const fn.
    const _AWAKE: &str = AgentStatus::Awake.as_db_str();
    const _RESTING: &str = AgentStatus::Resting.as_db_str();
    assert_eq!(_AWAKE, "awake");
    assert_eq!(_RESTING, "asleep");
}
