//! Unit tests for heartbeat ping fallback (T069, scenarios S080-S082).
//!
//! Verifies that `pick_primary_session` selects the most recently updated
//! session when multiple active sessions are present (US14 resilience).

use agent_intercom::mcp::tools::heartbeat::pick_primary_session;
use agent_intercom::models::session::{Session, SessionMode};
use chrono::Utc;

/// Build a minimal test session whose `updated_at` is `age_secs` seconds in the past.
fn session_aged(age_secs: i64) -> Session {
    let mut s = Session::new(
        "test-owner".to_owned(),
        std::env::temp_dir().to_string_lossy().into_owned(),
        None,
        SessionMode::Local,
    );
    s.updated_at = Utc::now() - chrono::Duration::seconds(age_secs);
    s
}

/// S080 — When multiple active sessions exist, the most recently updated is selected.
#[test]
fn pick_primary_session_selects_most_recent() {
    let older = session_aged(120); // two minutes old
    let newer = session_aged(5); // five seconds old
    let newer_id = newer.id.clone();

    let picked = pick_primary_session(vec![older, newer]);
    assert_eq!(
        picked.map(|s| s.id),
        Some(newer_id),
        "most recently updated session should be selected"
    );
}

/// S081 — When exactly one active session exists, that session is selected.
#[test]
fn pick_primary_session_returns_single_session() {
    let session = session_aged(0);
    let id = session.id.clone();

    let picked = pick_primary_session(vec![session]);
    assert_eq!(picked.map(|s| s.id), Some(id));
}

/// S082 — When no active sessions exist, `None` is returned.
#[test]
fn pick_primary_session_returns_none_for_empty() {
    let picked = pick_primary_session(vec![]);
    assert!(picked.is_none(), "empty list should return None");
}
