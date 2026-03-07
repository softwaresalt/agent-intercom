//! Integration tests for session Slack thread routing (T054).
//!
//! Verifies that two concurrent sessions maintain separate Slack threads
//! with no cross-contamination, and that `thread_ts` is scoped to its
//! originating channel (S041).

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};

// ── T054 / S041 ───────────────────────────────────────────────────────────────

/// Two concurrent sessions in different channels each record an independent
/// `thread_ts` with no interference between them.
#[tokio::test]
async fn two_concurrent_sessions_get_separate_threads() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    // Session A — channel X.
    let mut session_a = Session::new(
        "U_OWNER_A".into(),
        "/workspace/alpha".into(),
        None,
        SessionMode::Remote,
    );
    session_a.channel_id = Some("C_CHANNEL_X".into());
    let created_a = repo.create(&session_a).await.expect("create A");
    repo.update_status(&created_a.id, SessionStatus::Active)
        .await
        .expect("activate A");

    // Session B — channel Y.
    let mut session_b = Session::new(
        "U_OWNER_B".into(),
        "/workspace/beta".into(),
        None,
        SessionMode::Remote,
    );
    session_b.channel_id = Some("C_CHANNEL_Y".into());
    let created_b = repo.create(&session_b).await.expect("create B");
    repo.update_status(&created_b.id, SessionStatus::Active)
        .await
        .expect("activate B");

    // Simulate first Slack post for each session capturing its own ts.
    let ts_a = "1700001000.000001";
    let ts_b = "1700002000.000002";

    repo.set_thread_ts(&created_a.id, ts_a)
        .await
        .expect("set A thread_ts");
    repo.set_thread_ts(&created_b.id, ts_b)
        .await
        .expect("set B thread_ts");

    let fetched_a = repo
        .get_by_id(&created_a.id)
        .await
        .expect("query A")
        .expect("A exists");
    let fetched_b = repo
        .get_by_id(&created_b.id)
        .await
        .expect("query B")
        .expect("B exists");

    assert_eq!(
        fetched_a.thread_ts.as_deref(),
        Some(ts_a),
        "session A must keep its own thread_ts"
    );
    assert_eq!(
        fetched_b.thread_ts.as_deref(),
        Some(ts_b),
        "session B must keep its own thread_ts"
    );
    assert_ne!(
        fetched_a.thread_ts, fetched_b.thread_ts,
        "sessions A and B must have different thread_ts values"
    );
}

/// `find_by_channel_and_thread` is scoped to the (channel, `thread_ts`) pair —
/// the same `thread_ts` in a different channel returns no match.
#[tokio::test]
async fn session_thread_is_channel_scoped() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let shared_ts = "1700003000.000003";

    // Session in channel X with shared_ts.
    let mut session_x = Session::new(
        "U_OWNER".into(),
        "/workspace/x".into(),
        None,
        SessionMode::Remote,
    );
    session_x.channel_id = Some("C_CHANNEL_X".into());
    let created_x = repo.create(&session_x).await.expect("create X");
    repo.update_status(&created_x.id, SessionStatus::Active)
        .await
        .expect("activate X");
    repo.set_thread_ts(&created_x.id, shared_ts)
        .await
        .expect("set X thread_ts");

    // Look up by channel X + shared_ts → should find session_x.
    let found = repo
        .find_by_channel_and_thread("C_CHANNEL_X", shared_ts)
        .await
        .expect("find by channel+thread")
        .expect("should find session in channel X");
    assert_eq!(found.id, created_x.id, "should find session X by thread");

    // Look up by channel Y (no session there) + shared_ts → None.
    let not_found = repo
        .find_by_channel_and_thread("C_CHANNEL_Y", shared_ts)
        .await
        .expect("find by unknown channel+thread");
    assert!(
        not_found.is_none(),
        "thread_ts must be channel-scoped: same ts in different channel must not match"
    );
}
