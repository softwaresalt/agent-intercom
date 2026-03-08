//! Unit tests for `SessionRepo::count_active_acp` (T009–T011, T014, T015).
//!
//! Covers:
//! - T009 (S010): `count_active_acp` counts both `active` and `created` ACP sessions
//! - T010 (S011): `count_active_acp` excludes MCP sessions from count
//! - T011 (S015): `count_active_acp` excludes `terminated` and `interrupted` ACP sessions
//! - T014 (S014): with `max_sessions = 0`, count `>= max` always rejects all starts
//! - T015 (LC-06): `count_active_acp` counts `paused` ACP sessions (live child process)

use std::sync::Arc;

use agent_intercom::models::session::{ProtocolMode, Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Insert an ACP session with the given `status` into `repo`.
///
/// For statuses beyond `Created`, the function drives the session through the
/// required transition chain (`Created → Active [→ Paused | Terminated]`).
async fn insert_acp_session(repo: &SessionRepo, status: SessionStatus) -> String {
    let mut session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    session.protocol_mode = ProtocolMode::Acp;
    let created = repo.create(&session).await.expect("create ACP session");

    match status {
        SessionStatus::Created => {}
        SessionStatus::Active => {
            repo.update_status(&created.id, SessionStatus::Active)
                .await
                .expect("activate ACP session");
        }
        SessionStatus::Paused => {
            repo.update_status(&created.id, SessionStatus::Active)
                .await
                .expect("activate ACP session");
            repo.update_status(&created.id, SessionStatus::Paused)
                .await
                .expect("pause ACP session");
        }
        SessionStatus::Terminated => {
            repo.update_status(&created.id, SessionStatus::Active)
                .await
                .expect("activate ACP session");
            repo.set_terminated(&created.id, SessionStatus::Terminated)
                .await
                .expect("terminate ACP session");
        }
        SessionStatus::Interrupted => {
            repo.update_status(&created.id, SessionStatus::Active)
                .await
                .expect("activate ACP session");
            repo.update_status(&created.id, SessionStatus::Interrupted)
                .await
                .expect("interrupt ACP session");
        }
    }

    created.id
}

/// Insert an MCP session with `Active` status into `repo`.
async fn insert_mcp_session(repo: &SessionRepo) -> String {
    // `Session::new` defaults `protocol_mode` to `ProtocolMode::Mcp`.
    let session = Session::new("U2".into(), "/ws".into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create MCP session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate MCP session");
    created.id
}

// ── T009 / S010 ───────────────────────────────────────────────────────────────

/// `count_active_acp` must count both `active` and `created` ACP sessions.
///
/// A `created` session is initialising — it occupies an ACP capacity slot and
/// must be counted to prevent double-booking (F-07).
#[tokio::test]
async fn count_active_acp_includes_active_and_created_acp_sessions() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    // One ACP session still in `created` status (handshake not yet complete).
    insert_acp_session(&repo, SessionStatus::Created).await;
    // One ACP session that has progressed to `active`.
    insert_acp_session(&repo, SessionStatus::Active).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    assert_eq!(
        count, 2,
        "both `created` and `active` ACP sessions must be counted toward capacity"
    );
}

// ── T010 / S011 ───────────────────────────────────────────────────────────────

/// `count_active_acp` must exclude MCP sessions from the ACP capacity count.
///
/// MCP sessions run on a separate protocol and must not consume ACP capacity
/// slots (F-07: cross-protocol pollution).
#[tokio::test]
async fn count_active_acp_excludes_mcp_sessions() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    // Two active MCP sessions — must NOT count toward ACP capacity.
    insert_mcp_session(&repo).await;
    insert_mcp_session(&repo).await;

    // One active ACP session — must be counted.
    insert_acp_session(&repo, SessionStatus::Active).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    assert_eq!(count, 1, "MCP sessions must not consume ACP capacity slots");
}

// ── T011 / S015 ───────────────────────────────────────────────────────────────

/// `count_active_acp` must exclude `terminated` and `interrupted` ACP sessions.
///
/// Only sessions that actively occupy server resources contribute to the
/// capacity limit. Terminated and interrupted sessions are done — their child
/// processes have exited — and must not block new ACP starts.
#[tokio::test]
async fn count_active_acp_excludes_terminated_and_interrupted_sessions() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    // A terminated ACP session — done, must not count.
    insert_acp_session(&repo, SessionStatus::Terminated).await;
    // An interrupted ACP session — done, must not count.
    insert_acp_session(&repo, SessionStatus::Interrupted).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    assert_eq!(
        count, 0,
        "terminated and interrupted ACP sessions must not consume ACP capacity slots"
    );
}

// ── T015 / LC-06 ──────────────────────────────────────────────────────────────

/// `count_active_acp` must count `paused` ACP sessions toward the capacity limit.
///
/// A paused session still holds a live child process. Without counting it, an
/// operator could pause N sessions and then immediately start N more, effectively
/// doubling the configured limit (LC-06: capacity enforcement must include paused).
#[tokio::test]
async fn count_active_acp_includes_paused_sessions() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    // One paused ACP session — must count (live child process still running).
    insert_acp_session(&repo, SessionStatus::Paused).await;

    let count = repo.count_active_acp().await.expect("count_active_acp");
    assert_eq!(
        count, 1,
        "a paused ACP session must consume an ACP capacity slot (LC-06)"
    );
}

// ── T014 / S014 ───────────────────────────────────────────────────────────────

/// With `max_sessions = 0`, the capacity check `count >= max` evaluates to
/// `0 >= 0 == true` even with zero active sessions, rejecting every start.
///
/// `count_active_acp` must return `0` for an empty database; the handler
/// enforces rejection because `0 >= 0`.
#[tokio::test]
async fn count_active_acp_zero_active_still_rejected_when_max_is_zero() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    let count = repo.count_active_acp().await.expect("count_active_acp");

    // With max_sessions = 0 the capacity check becomes `count >= 0`.
    let max_sessions: i64 = 0;
    assert_eq!(count, 0, "empty repo must return count of 0");
    assert!(
        count >= max_sessions,
        "0 >= 0 must be true — all ACP starts are rejected when max_sessions = 0"
    );
}
