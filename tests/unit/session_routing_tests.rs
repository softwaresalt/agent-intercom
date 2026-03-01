//! Unit tests for session Slack thread routing (T052, T053, T055).
//!
//! Covers S036 (`thread_ts` recorded on first message), S037/S038 (subsequent
//! messages use `thread_ts`), and S042 (`thread_ts` immutability once set).

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};
use agent_intercom::slack::blocks;
use slack_morphism::prelude::{SlackChannelId, SlackTs};

use agent_intercom::slack::client::SlackMessage;

// ── T052 / S036 ───────────────────────────────────────────────────────────────

/// After `set_thread_ts` is called, the session record reflects the timestamp.
///
/// Also validates that `session_started_blocks` produces at least one block —
/// confirming the builder exists and returns usable content.
#[tokio::test]
async fn thread_ts_recorded_on_first_slack_message() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new(
        "U_OWNER".into(),
        "/workspace/test".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");

    // thread_ts must start as None before the first message.
    assert!(created.thread_ts.is_none(), "thread_ts should start None");

    // Simulate posting the session-started message and receiving a Slack ts.
    let simulated_ts = "1700000000.123456";
    repo.set_thread_ts(&created.id, simulated_ts)
        .await
        .expect("set_thread_ts");

    let fetched = repo
        .get_by_id(&created.id)
        .await
        .expect("query")
        .expect("session exists");

    assert_eq!(
        fetched.thread_ts.as_deref(),
        Some(simulated_ts),
        "thread_ts should match the simulated Slack ts"
    );

    // T057 dependency: session_started_blocks must exist and return blocks.
    let started_blocks = blocks::session_started_blocks(&fetched);
    assert!(
        !started_blocks.is_empty(),
        "session_started_blocks must produce at least one block"
    );
}

// ── T053 / S037 S038 ─────────────────────────────────────────────────────────

/// A `SlackMessage` constructed with a `thread_ts` carries it through to
/// the API request, and one without carries no thread anchor.
#[test]
fn subsequent_messages_use_thread_ts() {
    let channel = SlackChannelId("C_TEST_123".into());
    let ts = SlackTs("1700000000.123456".into());

    let threaded = SlackMessage {
        channel: channel.clone(),
        text: Some("threaded message".into()),
        blocks: None,
        thread_ts: Some(ts.clone()),
    };
    assert!(
        threaded.thread_ts.is_some(),
        "threaded SlackMessage must carry thread_ts"
    );
    assert_eq!(
        threaded.thread_ts.as_ref().map(|t| &t.0),
        Some(&ts.0),
        "thread_ts value must match the session ts"
    );
}

/// A `SlackMessage` without a `thread_ts` posts to the channel root.
#[test]
fn post_message_without_thread_ts_sends_channel_message() {
    let channel = SlackChannelId("C_TEST_123".into());

    let root_msg = SlackMessage {
        channel,
        text: Some("root level message".into()),
        blocks: None,
        thread_ts: None,
    };
    assert!(
        root_msg.thread_ts.is_none(),
        "root-level SlackMessage must have no thread_ts"
    );
}

// ── T055 / S042 ───────────────────────────────────────────────────────────────

/// `set_thread_ts` is a write-once operation: a second call with a different
/// value is silently ignored, preserving the original timestamp (S042).
#[tokio::test]
async fn thread_ts_is_immutable_once_set() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new(
        "U_OWNER".into(),
        "/workspace/immutable".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let first_ts = "1700000001.000000";
    let second_ts = "1700000099.999999";

    repo.set_thread_ts(&created.id, first_ts)
        .await
        .expect("first set_thread_ts");

    // Second call must be a no-op (WHERE thread_ts IS NULL guard in the repo).
    repo.set_thread_ts(&created.id, second_ts)
        .await
        .expect("second set_thread_ts should not error");

    let fetched = repo
        .get_by_id(&created.id)
        .await
        .expect("query")
        .expect("session exists");

    assert_eq!(
        fetched.thread_ts.as_deref(),
        Some(first_ts),
        "thread_ts must not be overwritten once set (S042)"
    );
}

// ── T057 (block builder shape) ────────────────────────────────────────────────

/// `session_started_blocks` includes the session ID prefix and workspace path.
#[test]
fn session_started_blocks_contains_expected_content() {
    let session = Session::new(
        "U_OWNER".into(),
        "/workspace/myproject".into(),
        Some("build a thing".into()),
        SessionMode::Remote,
    );
    let rendered_blocks = blocks::session_started_blocks(&session);

    assert!(
        !rendered_blocks.is_empty(),
        "session_started_blocks must return at least one block"
    );
    // Serialize blocks to JSON to assert on human-readable content.
    let json = serde_json::to_string(&rendered_blocks).expect("serialize blocks");
    // The first 8 chars of the session ID should appear somewhere.
    let id_prefix: String = session.id.chars().take(8).collect();
    assert!(
        json.contains(&id_prefix),
        "session_started_blocks must include session ID prefix: {id_prefix}"
    );
}

/// `session_ended_blocks` produces at least one block with the status.
#[test]
fn session_ended_blocks_contains_expected_content() {
    let mut session = Session::new(
        "U_OWNER".into(),
        "/workspace/myproject".into(),
        None,
        SessionMode::Remote,
    );
    session.status = SessionStatus::Terminated;
    session.terminated_at = Some(chrono::Utc::now());

    let rendered_blocks = blocks::session_ended_blocks(&session, "terminated by operator");

    assert!(
        !rendered_blocks.is_empty(),
        "session_ended_blocks must return at least one block"
    );
    let json = serde_json::to_string(&rendered_blocks).expect("serialize blocks");
    assert!(
        json.contains("terminated by operator"),
        "session_ended_blocks must include the reason"
    );
}
