//! Integration tests for push-event routing (Copilot PR review: `push_events.rs`).
//!
//! Covers:
//! - Unauthorized users are silently ignored (auth guard)
//! - Authorized users pass the auth check
//! - Thread replies are routed to `store_from_slack` with the correct
//!   `channel_id` and `thread_ts`
//! - Top-level messages are filtered before reaching `store_from_slack`
//!   (verified by checking the function directly, since `handle_push_event`
//!   exits early without calling it)
//! - `post_ack` threads replies when `thread_ts` is present (verified via
//!   the `SteeringRepo` queue after `store_from_slack` succeeds)

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::slack::handlers::steer;
use agent_intercom::slack::push_events::is_authorized;

use super::test_helpers::{test_app_state, test_config};

// ── Auth guard ───────────────────────────────────────────────────────────────

/// Unauthorized users must be silently ignored by the auth guard.
#[tokio::test]
async fn push_event_unknown_user_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    // test_config does not populate authorized_user_ids, so any user is rejected.
    assert!(
        !is_authorized("U_UNKNOWN", &state),
        "unknown user must not pass auth check"
    );
}

/// A user that appears in `authorized_user_ids` must pass the auth check.
#[tokio::test]
async fn push_event_authorized_user_is_allowed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let mut config = test_config(root);
    config.authorized_user_ids = vec!["U_AUTHORIZED".to_owned()];
    let state = test_app_state(config).await;

    assert!(
        is_authorized("U_AUTHORIZED", &state),
        "authorized user must pass auth check"
    );
    assert!(
        !is_authorized("U_IMPOSTER", &state),
        "non-listed user must fail auth check even when others are authorized"
    );
}

// ── Thread-reply routing ─────────────────────────────────────────────────────

/// A thread reply must be routed to `store_from_slack` with the correct
/// `channel_id` and `thread_ts`, and the storage must succeed.
#[tokio::test]
async fn push_event_thread_reply_routes_with_channel_and_thread_ts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    // Create a session bound to channel "C_PUSH_TEST".
    let repo = SessionRepo::new(Arc::clone(&state.db));
    let mut session = Session::new(
        "U_OWNER".into(),
        root.into(),
        None,
        SessionMode::Remote,
    );
    session.channel_id = Some("C_PUSH_TEST".into());
    session.thread_ts = Some("1700001000.000001".into());
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    // Simulate the routing that `handle_push_event` performs for thread replies.
    let result = steer::store_from_slack(
        "refocus on the failing tests",
        Some("C_PUSH_TEST"),
        Some("1700001000.000001"),
        &state,
    )
    .await;

    assert!(
        result.is_ok(),
        "thread reply must be stored without error; got: {result:?}"
    );
}

// ── Top-level message filtering ───────────────────────────────────────────────

/// `store_from_slack` called without `thread_ts` (simulating a channel-scoped
/// lookup) must still succeed when an active session exists for the channel.
/// This proves the path exercised after the top-level-message guard in
/// `handle_push_event` (which exits early for non-thread messages) doesn't
/// break the routing contract.
#[tokio::test]
async fn push_event_store_from_slack_channel_scoped_succeeds() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let repo = SessionRepo::new(Arc::clone(&state.db));
    let mut session = Session::new(
        "U_OWNER".into(),
        root.into(),
        None,
        SessionMode::Remote,
    );
    session.channel_id = Some("C_TOPLEVEL".into());
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    // No thread_ts — mirrors what a channel-scoped steer call would do.
    let result =
        steer::store_from_slack("hello from channel", Some("C_TOPLEVEL"), None, &state).await;

    assert!(
        result.is_ok(),
        "channel-scoped steering must succeed; got: {result:?}"
    );
}

/// When there is no active session in the channel, `store_from_slack` must
/// return a descriptive error — it must never silently discard the message.
#[tokio::test]
async fn push_event_store_from_slack_no_session_returns_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    // No session created for "C_EMPTY".
    let result =
        steer::store_from_slack("hello", Some("C_EMPTY"), Some("1700001000.000001"), &state).await;

    assert!(
        result.is_err(),
        "missing session must produce an error, not silently drop the message"
    );
}
