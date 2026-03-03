//! Unit tests for Slack client WebSocket disconnect channel collection (T135, T136).
//!
//! Covers:
//! - T135 (S098, S099): active session with `channel_id` → channel returned from query
//! - T136 (S100): no active sessions → empty vec returned

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};
use agent_intercom::slack::client::collect_active_session_channels;

// ── T135 (S098/S099): Active session with channel → channel included ──────────

/// When at least one session is `active` and has a `channel_id`, that channel
/// must appear in the result of `collect_active_session_channels`.
#[tokio::test]
async fn active_session_with_channel_is_included() {
    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&pool));

    // Insert an active session bound to a Slack channel.
    let mut session = Session::new(
        "U-OPERATOR".to_owned(),
        "/workspace".to_owned(),
        Some("test".to_owned()),
        SessionMode::Remote,
    );
    session.channel_id = Some("C-WS-NOTIFY".to_owned());
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let channels = collect_active_session_channels(&pool)
        .await
        .expect("query must succeed");

    let raw: Vec<&str> = channels.iter().map(|c| c.0.as_str()).collect();
    assert!(
        raw.contains(&"C-WS-NOTIFY"),
        "active session channel must appear in result, got: {raw:?}"
    );
}

// ── T135 cont.: Session without channel_id is excluded ───────────────────────

/// Active sessions that have `channel_id = NULL` must not produce a result entry.
#[tokio::test]
async fn active_session_without_channel_is_excluded() {
    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&pool));

    // Active session with no channel_id attached (MCP direct-connect style).
    let session = Session::new(
        "U-LOCAL".to_owned(),
        "/workspace".to_owned(),
        None,
        SessionMode::Local,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let channels = collect_active_session_channels(&pool)
        .await
        .expect("query must succeed");

    assert!(
        channels.is_empty(),
        "session without channel_id must be excluded, got: {channels:?}"
    );
}

// ── T136 (S100): No active sessions → empty vec ───────────────────────────────

/// When the DB contains no active sessions, `collect_active_session_channels`
/// must return an empty `Vec` without error.
#[tokio::test]
async fn no_active_sessions_returns_empty_vec() {
    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));

    let channels = collect_active_session_channels(&pool)
        .await
        .expect("query must succeed on empty db");

    assert!(
        channels.is_empty(),
        "empty db must produce empty channel list, got: {channels:?}"
    );
}
