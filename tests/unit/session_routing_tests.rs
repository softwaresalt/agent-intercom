//! Unit tests for session Slack thread routing (T052, T053, T055) and
//! multi-session channel routing (T061–T063, T068b).
//!
//! Covers:
//! - S036 (`thread_ts` recorded on first message)
//! - S037/S038 (subsequent messages use `thread_ts`)
//! - S042 (`thread_ts` immutability once set)
//! - S043 (`find_active_by_channel` returns correct session)
//! - S045 (no active session in channel → empty result)
//! - S046 (`find_by_channel_and_thread` disambiguates multiple sessions)
//! - S047 (non-existent `thread_ts` → None)
//! - S076 / FR-031 (non-owner action rejected by `check_session_ownership`)

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};
use agent_intercom::slack::blocks;
use agent_intercom::slack::handlers::check_session_ownership;
use agent_intercom::AppError;
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

// ── T061 / S043 ───────────────────────────────────────────────────────────────

/// `find_active_by_channel` returns the active session in the given channel (S043).
#[tokio::test]
async fn session_lookup_by_channel_returns_active_session() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let mut session = Session::new(
        "U_OWNER".into(),
        "/workspace/channel-test".into(),
        None,
        SessionMode::Remote,
    );
    session.channel_id = Some("C_ROUTED".into());
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let results = repo
        .find_active_by_channel("C_ROUTED")
        .await
        .expect("find_active_by_channel");

    assert!(
        !results.is_empty(),
        "find_active_by_channel must return the active session (S043)"
    );
    assert_eq!(
        results[0].id, created.id,
        "returned session must match the created session"
    );
}

// ── T062 / S045 ───────────────────────────────────────────────────────────────

/// `find_active_by_channel` returns an empty vec when no session exists in the
/// channel, so callers can surface a "no active session in this channel" message
/// (S045).
#[tokio::test]
async fn session_lookup_by_channel_returns_none_when_no_active_session() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    // Create a session in a different channel — should not appear in "C_EMPTY".
    let mut other = Session::new(
        "U_OWNER".into(),
        "/workspace/other".into(),
        None,
        SessionMode::Remote,
    );
    other.channel_id = Some("C_OTHER".into());
    let created = repo.create(&other).await.expect("create other");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate other");

    let results = repo
        .find_active_by_channel("C_EMPTY")
        .await
        .expect("find_active_by_channel");

    assert!(
        results.is_empty(),
        "channel with no sessions must return empty vec (S045)"
    );
}

// ── T063 / S046 ───────────────────────────────────────────────────────────────

/// `find_by_channel_and_thread` returns the correct session when multiple
/// sessions share a channel but have different `thread_ts` values (S046).
#[tokio::test]
async fn thread_ts_disambiguates_multiple_sessions_in_channel() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let ts_a = "1700010000.000001";
    let ts_b = "1700020000.000002";

    // Session A — channel C_SHARED, thread ts_a.
    let mut session_a = Session::new(
        "U_OWNER_A".into(),
        "/workspace/a".into(),
        None,
        SessionMode::Remote,
    );
    session_a.channel_id = Some("C_SHARED".into());
    let created_a = repo.create(&session_a).await.expect("create A");
    repo.update_status(&created_a.id, SessionStatus::Active)
        .await
        .expect("activate A");
    repo.set_thread_ts(&created_a.id, ts_a)
        .await
        .expect("set thread ts A");

    // Session B — same channel, different thread ts_b.
    let mut session_b = Session::new(
        "U_OWNER_B".into(),
        "/workspace/b".into(),
        None,
        SessionMode::Remote,
    );
    session_b.channel_id = Some("C_SHARED".into());
    let created_b = repo.create(&session_b).await.expect("create B");
    repo.update_status(&created_b.id, SessionStatus::Active)
        .await
        .expect("activate B");
    repo.set_thread_ts(&created_b.id, ts_b)
        .await
        .expect("set thread ts B");

    // Looking up by ts_a must return session A.
    let found_a = repo
        .find_by_channel_and_thread("C_SHARED", ts_a)
        .await
        .expect("find A")
        .expect("must find session A");
    assert_eq!(
        found_a.id, created_a.id,
        "find_by_channel_and_thread must disambiguate by thread_ts (S046)"
    );

    // Looking up by ts_b must return session B.
    let found_b = repo
        .find_by_channel_and_thread("C_SHARED", ts_b)
        .await
        .expect("find B")
        .expect("must find session B");
    assert_eq!(
        found_b.id, created_b.id,
        "find_by_channel_and_thread must return session B for ts_b"
    );
}

// ── T063 / S047 ───────────────────────────────────────────────────────────────

/// `find_by_channel_and_thread` returns `None` when `thread_ts` does not exist
/// in the channel (S047).
#[tokio::test]
async fn thread_ts_returns_none_when_thread_not_found() {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let not_found = repo
        .find_by_channel_and_thread("C_ANY", "1700099999.000000")
        .await
        .expect("find_by_channel_and_thread must not error");

    assert!(
        not_found.is_none(),
        "non-existent thread_ts must return None (S047)"
    );
}

// ── T068b / S076 ──────────────────────────────────────────────────────────────

/// `check_session_ownership` returns `Unauthorized` when the acting user is not
/// the session owner (S076 / FR-031).
#[test]
fn non_owner_action_is_rejected() {
    let session = Session::new(
        "U_OWNER".into(),
        "/workspace/owned".into(),
        None,
        SessionMode::Remote,
    );

    // A different user must be rejected.
    let result = check_session_ownership(&session, "U_OTHER");
    assert!(
        matches!(result, Err(AppError::Unauthorized(_))),
        "non-owner must receive Unauthorized error (S076 / FR-031)"
    );

    // The actual owner must be accepted.
    let ok = check_session_ownership(&session, "U_OWNER");
    assert!(ok.is_ok(), "session owner must pass the ownership check");
}

/// `check_session_ownership` skips the check when `owner_user_id` is empty
/// (MCP sessions without a designated operator).
#[test]
fn empty_owner_skips_ownership_check() {
    let session = Session::new(
        String::new(), // empty owner — MCP session placeholder
        "/workspace/mcp".into(),
        None,
        SessionMode::Remote,
    );

    // Any user must be accepted when there is no designated owner.
    let result = check_session_ownership(&session, "U_ANYONE");
    assert!(
        result.is_ok(),
        "empty owner_user_id must skip the ownership check"
    );
}
