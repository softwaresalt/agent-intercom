//! Contract tests for ACP session capacity enforcement (T012–T013).
//!
//! Covers:
//! - T012 (S008, S010): ACP capacity is enforced against both `created` and
//!   `active` sessions — a `created` (initialising) session is not double-bookable.
//! - T013 (S011): ACP capacity is unaffected by active MCP sessions — an ACP
//!   start succeeds when only MCP sessions are active.
//!
//! These tests verify the *capacity enforcement contract*: the count returned
//! by `count_active_acp` drives the `>= max` gate in `handle_acp_session_start`.

use std::sync::Arc;

use agent_intercom::models::session::{ProtocolMode, Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Insert one ACP session at `Created` status (handshake in progress).
async fn insert_acp_created(repo: &SessionRepo) {
    let mut session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    session.protocol_mode = ProtocolMode::Acp;
    repo.create(&session).await.expect("create ACP session");
}

/// Insert one ACP session at `Active` status (handshake complete).
async fn insert_acp_active(repo: &SessionRepo) {
    let mut session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    session.protocol_mode = ProtocolMode::Acp;
    let created = repo.create(&session).await.expect("create ACP session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate ACP session");
}

/// Insert one MCP session at `Active` status.
async fn insert_mcp_active(repo: &SessionRepo) {
    // `Session::new` defaults `protocol_mode` to `ProtocolMode::Mcp`.
    let session = Session::new("U2".into(), "/ws".into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create MCP session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate MCP session");
}

// ── T012 / S008 S010 ─────────────────────────────────────────────────────────

/// ACP capacity enforcement counts `created` sessions as occupying a slot.
///
/// When `max_sessions = 1` and one ACP session is in the `created` state
/// (handshake still in progress), `count_active_acp()` must return 1 so that
/// the `count >= max` check blocks a second start.  This prevents the
/// double-booking race where two starts interleave during the handshake window.
#[tokio::test]
async fn acp_capacity_rejects_start_when_created_session_occupies_slot() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    // Simulate an ACP session that has been created but whose handshake is
    // still in progress (status = `created`).
    insert_acp_created(&repo).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    let max_sessions: i64 = 1;

    // The `>= max` gate in `handle_acp_session_start` must fire.
    assert_eq!(
        count, 1,
        "a `created` ACP session must occupy a capacity slot"
    );
    assert!(
        count >= max_sessions,
        "count ({count}) >= max_sessions ({max_sessions}) must be true — start must be rejected"
    );
}

// ── T013 / S011 ───────────────────────────────────────────────────────────────

/// ACP capacity is independent of active MCP sessions.
///
/// When `max_sessions = 1` and only MCP sessions are running, an ACP start
/// must be permitted.  `count_active_acp()` must return 0, so `count < max`
/// and the gate allows the new session to proceed.
#[tokio::test]
async fn acp_capacity_allows_start_when_only_mcp_sessions_are_active() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    // Simulate the scenario where an MCP session is fully active.
    insert_mcp_active(&repo).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    let max_sessions: i64 = 1;

    // The `>= max` gate must NOT fire — ACP capacity is unaffected.
    assert_eq!(
        count, 0,
        "active MCP sessions must not consume ACP capacity slots"
    );
    assert!(
        count < max_sessions,
        "count ({count}) < max_sessions ({max_sessions}) must be true — start must be allowed"
    );
}

// ── Regression: both created and active together ──────────────────────────────

/// When both a `created` and an `active` ACP session exist, the combined count
/// must equal 2, saturating a `max_sessions = 2` limit.
#[tokio::test]
async fn acp_capacity_counts_created_and_active_together() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    insert_acp_created(&repo).await;
    insert_acp_active(&repo).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    let max_sessions: i64 = 2;

    assert_eq!(
        count, 2,
        "one `created` + one `active` must yield count of 2"
    );
    assert!(
        count >= max_sessions,
        "count ({count}) >= max_sessions ({max_sessions}) — start must be rejected"
    );
}
