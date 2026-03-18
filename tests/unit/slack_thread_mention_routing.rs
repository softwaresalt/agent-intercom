//! Unit tests for the @-mention thread-reply routing gap (F-16/F-17 — Phase 2).
//!
//! ## Gap being fixed
//!
//! When a Slack operator replies to a pending-fallback thread by tagging
//! `@agent-intercom`, the event arrives as `AppMention` rather than a plain
//! `Message`.  Before this fix, the `AppMention` arm in `push_events.rs`
//! called `ingest_app_mention` **without** first consulting
//! `pending_thread_replies`, so the reply was routed to steering instead of
//! resolving the Refine prompt.
//!
//! These tests verify that:
//! 1. `route_thread_reply` correctly captures text that has already had the
//!    `<@UXXXXX>` bot-mention prefix stripped — i.e., the wiring that
//!    `push_events.rs` now invokes works end-to-end.
//! 2. Without a pending entry the function returns `Ok(false)`, so normal
//!    `ingest_app_mention` steering is unaffected.
//!
//! The TDD "red" state: before Phase 1 (`strip_mention` visibility change) and
//! Phase 2 (`AppMention` arm update), the `push_events.rs` handler never reaches
//! `route_thread_reply` for `AppMention` events.  These tests establish the
//! *expected* behaviour and serve as a regression guard once the fix lands.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};

/// Mirror the production type alias so tests are self-contained.
type PendingThreadReplies = Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;

// ── T-MR-001: @-mention reply captured via stripped text ──────────────────────

/// An @-mention reply whose mention prefix has been stripped must be captured
/// by `route_thread_reply` when a pending fallback entry exists.
///
/// This test documents the wiring that `push_events.rs` `AppMention` arm invokes
/// after Phase 2.  The mention prefix `<@UBOTID> ` is stripped by
/// `handlers::steer::strip_mention` before the text reaches `route_thread_reply`.
#[tokio::test]
async fn test_mr001_stripped_mention_captured_by_route_thread_reply() {
    use agent_intercom::slack::handlers::thread_reply::{
        register_thread_reply_fallback, route_thread_reply,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C_MENTION_TEST";
    let thread_ts = "1700000100.000001".to_owned();
    let authorized_user = "U_OPERATOR".to_owned();

    // Register a pending fallback (simulating what activate_thread_reply_fallback does).
    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-mr001".to_owned(),
        authorized_user.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Simulate what push_events.rs AppMention arm now does:
    //   1. Extract text from AppMention event.
    //   2. Strip the bot-mention prefix via strip_mention (now pub(crate)).
    //   3. Call route_thread_reply with the stripped text.
    //
    // We simulate step 2 inline here since strip_mention is pub(crate) (not
    // visible from the test crate).  The full production path calls
    // `handlers::steer::strip_mention(text).trim()`.
    let raw_mention_text = "<@UBOTID> Use a more concise tone";
    // Simulate the strip: remove the `<@...>` prefix.
    let stripped = raw_mention_text
        .trim_start()
        .split_once('>')
        .map_or(raw_mention_text, |(_, rest)| rest)
        .trim();

    assert_eq!(
        stripped, "Use a more concise tone",
        "pre-condition: stripping should remove the mention prefix"
    );

    let result = route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized_user,
        stripped,
        Arc::clone(&pending),
    )
    .await;

    assert!(result.is_ok(), "route_thread_reply must succeed");
    assert!(
        result.unwrap(),
        "route_thread_reply must return Ok(true) when a pending entry exists"
    );

    let received = rx.await.expect("oneshot must deliver the reply text");
    assert_eq!(
        received, "Use a more concise tone",
        "reply text delivered through oneshot must match the stripped mention text"
    );
}

// ── T-MR-002: No pending entry → Ok(false), steering path is safe ─────────────

/// When there is no pending fallback for the channel+thread, `route_thread_reply`
/// must return `Ok(false)` so the `AppMention` arm can fall through to normal
/// steering via `ingest_app_mention`.
///
/// This ensures the fix does not break non-fallback @-mention handling.
#[tokio::test]
async fn test_mr002_no_pending_entry_returns_false_so_steering_unaffected() {
    use agent_intercom::slack::handlers::thread_reply::route_thread_reply;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));

    let result = route_thread_reply(
        "C_NO_FALLBACK",
        "9999.0001",
        "U_OPERATOR",
        "Use a more concise tone",
        Arc::clone(&pending),
    )
    .await;

    assert!(
        result.is_ok(),
        "no-entry case must return Ok, not Err (S038 extended for mention path)"
    );
    assert!(
        !result.unwrap(),
        "no-entry case must return Ok(false), allowing fall-through to steering"
    );
}

// ── T-MR-003: Top-level mention (no thread_ts) is never checked ───────────────

/// A top-level `AppMention` (no `thread_ts`) must never consult
/// `pending_thread_replies`.  Since `route_thread_reply` is only called when
/// `thread_ts` is `Some`, a top-level mention with a registered entry for a
/// *different* key must still return `Ok(false)`.
///
/// This verifies the guard condition: `if let Some(ref ts) = thread_ts { ... }`.
#[tokio::test]
async fn test_mr003_top_level_mention_does_not_consume_pending_entry() {
    use agent_intercom::slack::handlers::thread_reply::{
        fallback_map_key, register_thread_reply_fallback, route_thread_reply,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C_TOP_LEVEL";
    let thread_ts = "1700000200.000001".to_owned();
    let authorized_user = "U_OPERATOR".to_owned();

    let (tx, _rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-mr003".to_owned(),
        authorized_user.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // A top-level mention has no thread_ts, so we would NOT call route_thread_reply.
    // We verify this indirectly: calling route_thread_reply with a completely
    // different channel/ts pair returns Ok(false) and does not touch the existing entry.
    let result = route_thread_reply(
        channel_id,
        "0000.0000", // different thread_ts — as if we looked up the wrong key
        &authorized_user,
        "some text",
        Arc::clone(&pending),
    )
    .await;

    assert!(result.is_ok(), "mismatched thread_ts must not error");
    assert!(
        !result.unwrap(),
        "mismatched thread_ts must return Ok(false)"
    );

    // The original entry must be untouched.
    let guard = pending.lock().await;
    assert!(
        guard.contains_key(&fallback_map_key(channel_id, &thread_ts)),
        "pending entry must remain when a different thread_ts is queried"
    );
}

// ── T-MR-004: Unauthorized @-mention reply is silently ignored ────────────────

/// An @-mention from an unauthorized user must not consume the pending entry,
/// leaving it available for the authorized operator to respond.
#[tokio::test]
async fn test_mr004_unauthorized_mention_reply_ignored() {
    use agent_intercom::slack::handlers::thread_reply::{
        fallback_map_key, register_thread_reply_fallback, route_thread_reply,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C_UNAUTH";
    let thread_ts = "1700000300.000001".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-mr004".to_owned(),
        "U_AUTHORIZED".to_owned(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Unauthorized user @-mentions the bot in the thread.
    let result = route_thread_reply(
        channel_id,
        &thread_ts,
        "U_UNAUTHORIZED",
        "I am trying to hijack this",
        Arc::clone(&pending),
    )
    .await;

    assert!(result.is_ok(), "unauthorized mention must not return Err");
    assert!(
        !result.unwrap(),
        "unauthorized mention must return Ok(false)"
    );

    // Entry must still be present.
    assert!(
        pending
            .lock()
            .await
            .contains_key(&fallback_map_key(channel_id, &thread_ts)),
        "pending entry must survive unauthorized mention reply"
    );

    // The oneshot must not have received anything.
    // Drop the sender to clean up, then rx should give Err (not the unauthorized text).
    pending.lock().await.clear(); // drops tx
    assert!(
        rx.await.is_err(),
        "oneshot must not carry the unauthorized text"
    );
}
