//! Integration tests for thread-reply fallback edge cases (Phase 3, Task 3.3).
//!
//! Verifies the fallback handler's behavior when an incoming thread reply
//! arrives for a registered pending entry (S-T1-017) and when no entry is
//! registered (S-T1-018).
//!
//! Uses [`register_thread_reply_fallback`] and [`route_thread_reply`] from
//! `slack::handlers::thread_reply` directly, with no live Slack connection.
//!
//! Scenarios covered:
//! - S-T1-017: Registered fallback → reply routes to oneshot
//! - S-T1-018: Orphaned thread reply → gracefully ignored (Ok(false))
//! - Duplicate registration guard (LC-04): second register for same key is dropped

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{oneshot, Mutex};

use agent_intercom::slack::handlers::thread_reply::{
    fallback_map_key, register_thread_reply_fallback, route_thread_reply,
};

// ── Shared helpers ────────────────────────────────────────────────────────────

type PendingReplies = Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;

fn make_pending() -> PendingReplies {
    Arc::new(Mutex::new(HashMap::new()))
}

// ── S-T1-017: Registered fallback routes to oneshot ──────────────────────────

/// S-T1-017 — A registered thread-reply fallback forwards the operator's reply
/// to the waiting oneshot channel.
///
/// Sequence:
/// 1. Register fallback for `thread_ts = "1234567890.123456"`.
/// 2. Route a reply from the authorized user with text `"Use retry logic"`.
/// 3. Verify `route_thread_reply` returns `Ok(true)`.
/// 4. Verify the oneshot channel delivers `"Use retry logic"`.
/// 5. Verify the pending map entry is removed after delivery (no double-send).
#[tokio::test]
async fn registered_fallback_routes_reply_to_oneshot() {
    let pending = make_pending();
    let channel_id = "C_TEST_FALLBACK";
    let thread_ts = "1234567890.123456".to_owned();
    let session_id = "session-fallback-001".to_owned();
    let authorized_user = "U_OPERATOR".to_owned();

    // Step 1: register the fallback entry.
    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        session_id.clone(),
        authorized_user.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Confirm the entry is registered.
    let key = fallback_map_key(channel_id, &thread_ts);
    assert!(
        pending.lock().await.contains_key(&key),
        "S-T1-017: fallback entry must be registered"
    );

    // Step 2: route the reply from the authorized user.
    let routed = route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized_user,
        "Use retry logic",
        Arc::clone(&pending),
    )
    .await;

    assert_eq!(
        routed,
        Ok(true),
        "S-T1-017: route_thread_reply must return Ok(true) for a registered pending entry"
    );

    // Step 3: oneshot must deliver the reply text.
    let delivered = tokio::time::timeout(Duration::from_millis(500), rx)
        .await
        .expect("oneshot must resolve within timeout")
        .expect("oneshot channel must not be dropped");

    assert_eq!(
        delivered, "Use retry logic",
        "S-T1-017: delivered text must match the operator's reply"
    );

    // Step 4: pending map entry must be removed after successful delivery.
    assert!(
        !pending.lock().await.contains_key(&key),
        "S-T1-017: pending entry must be removed after reply is captured"
    );
}

// ── S-T1-018: Orphaned thread reply ignored gracefully ────────────────────────

/// S-T1-018 — A thread reply arriving with no registered pending entry must be
/// silently ignored without error or panic.
///
/// `route_thread_reply` must return `Ok(false)` when there is no entry for
/// the given `(channel_id, thread_ts)` composite key.
#[tokio::test]
async fn orphaned_thread_reply_returns_false_without_error() {
    let pending = make_pending();
    let channel_id = "C_TEST_ORPHAN";
    let thread_ts = "9999999999.000001";
    let user_id = "U_RANDOM_USER";

    // No fallback has been registered for this thread.
    let result = route_thread_reply(
        channel_id,
        thread_ts,
        user_id,
        "This reply has no recipient",
        Arc::clone(&pending),
    )
    .await;

    assert_eq!(
        result,
        Ok(false),
        "S-T1-018: orphaned reply must return Ok(false) without error"
    );

    // Pending map must remain empty — no side effects.
    assert!(
        pending.lock().await.is_empty(),
        "S-T1-018: orphaned reply must leave pending map unchanged"
    );
}

// ── S-T1-018 variant: unauthorized reply is silently ignored ─────────────────

/// Unauthorized user's reply to a registered thread must be silently ignored.
///
/// The pending entry must remain so the authorized operator can still respond.
/// `route_thread_reply` returns `Ok(false)` — not an error — for unauthorized
/// senders.
#[tokio::test]
async fn unauthorized_reply_to_registered_fallback_is_ignored() {
    let pending = make_pending();
    let channel_id = "C_TEST_AUTHGUARD";
    let thread_ts = "1111111111.000001".to_owned();
    let session_id = "session-authguard".to_owned();
    let authorized_user = "U_AUTHORIZED".to_owned();
    let intruder = "U_INTRUDER";

    // Register with the authorized user.
    let (tx, _rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        session_id,
        authorized_user,
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Unauthorized user sends a reply.
    let result = route_thread_reply(
        channel_id,
        &thread_ts,
        intruder,
        "I am not the operator",
        Arc::clone(&pending),
    )
    .await;

    assert_eq!(
        result,
        Ok(false),
        "unauthorized reply must return Ok(false) — entry must remain for authorized user"
    );

    // Entry must still be present — the authorized operator has not yet replied.
    let key = fallback_map_key(channel_id, &thread_ts);
    assert!(
        pending.lock().await.contains_key(&key),
        "pending entry must persist after unauthorized reply (authorized user can still respond)"
    );
}

// ── LC-04: Duplicate registration guard ──────────────────────────────────────

/// LC-04 — Registering a second fallback for an already-registered key drops
/// the new sender, leaving the original entry intact.
///
/// This prevents a rapid double-tap from replacing a valid registration.
/// The original entry's `tx` remains live; the new `tx` is dropped (causing
/// its `rx` to immediately error).
#[tokio::test]
async fn duplicate_fallback_registration_drops_new_sender() {
    let pending = make_pending();
    let channel_id = "C_TEST_DUPLICATE";
    let thread_ts = "2222222222.000001".to_owned();
    let authorized_user = "U_OPERATOR".to_owned();

    // First registration — valid entry.
    let (tx1, rx1) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-dup-001".to_owned(),
        authorized_user.clone(),
        tx1,
        Arc::clone(&pending),
    )
    .await;

    // Second registration for the same key — must be dropped (LC-04).
    let (tx2, mut rx2) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-dup-002".to_owned(),
        authorized_user.clone(),
        tx2,
        Arc::clone(&pending),
    )
    .await;

    // The second receiver must immediately error because tx2 was dropped.
    assert!(
        rx2.try_recv().is_err(),
        "LC-04: duplicate sender must be dropped; its receiver must error immediately"
    );

    // The original entry is still valid — route a reply via the first entry.
    let routed = route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized_user,
        "original entry intact",
        Arc::clone(&pending),
    )
    .await;

    assert_eq!(
        routed,
        Ok(true),
        "LC-04: original entry must still be reachable after duplicate registration is dropped"
    );

    let delivered = tokio::time::timeout(Duration::from_millis(200), rx1)
        .await
        .expect("original oneshot must still resolve")
        .expect("original oneshot must not be dropped");

    assert_eq!(
        delivered, "original entry intact",
        "LC-04: original oneshot must receive the reply text"
    );
}
