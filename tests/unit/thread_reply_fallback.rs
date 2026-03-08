//! Unit tests for thread-reply fallback (F-16, F-17) — S029–S033.
//!
//! These tests verify the core `register_thread_reply_fallback` and
//! `route_thread_reply` functions in isolation, without requiring a full
//! `AppState` construction.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};

/// Convenience alias matching the production type.
type PendingThreadReplies = Arc<Mutex<HashMap<String, (String, oneshot::Sender<String>)>>>;

// ── T039 / S029 ───────────────────────────────────────────────────────────────

/// Modal failure triggers fallback message registration.
///
/// When `open_modal` fails, a oneshot sender should be registered in
/// `pending_thread_replies` keyed by `thread_ts`.
#[tokio::test]
async fn test_s029_fallback_message_registration() {
    use agent_intercom::slack::handlers::thread_reply::register_thread_reply_fallback;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let thread_ts = "1234567890.000100".to_owned();

    let (tx, _rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        thread_ts.clone(),
        "U12345".to_owned(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    let guard = pending.lock().await;
    assert!(
        guard.contains_key(&thread_ts),
        "sender should be registered for thread_ts"
    );
}

// ── T040 / S030 ───────────────────────────────────────────────────────────────

/// Thread reply captured and routed to waiting oneshot.
///
/// When an authorized user replies in the fallback thread, the reply text
/// must be delivered through the registered oneshot channel.
#[tokio::test]
async fn test_s030_reply_routes_to_oneshot() {
    use agent_intercom::slack::handlers::thread_reply::route_thread_reply;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let thread_ts = "1234567890.000200".to_owned();
    let authorized_user = "U12345".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    pending
        .lock()
        .await
        .insert(thread_ts.clone(), (authorized_user.clone(), tx));

    let result = route_thread_reply(
        &thread_ts,
        &authorized_user,
        "approve",
        Arc::clone(&pending),
    )
    .await;

    assert!(result.is_ok(), "route_thread_reply should return Ok");
    let received = rx.await.expect("should receive value through oneshot");
    assert_eq!(received, "approve", "received text should match the reply");
}

// ── T041 / S031 ───────────────────────────────────────────────────────────────

/// Entry removed after reply capture (acknowledgment step).
///
/// After the first reply is captured the map entry must be removed so
/// subsequent replies are not forwarded again (only first captured).
#[tokio::test]
async fn test_s031_entry_removed_after_capture() {
    use agent_intercom::slack::handlers::thread_reply::route_thread_reply;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let thread_ts = "1234567890.000300".to_owned();
    let authorized_user = "U12345".to_owned();

    let (tx, _rx) = oneshot::channel::<String>();
    pending
        .lock()
        .await
        .insert(thread_ts.clone(), (authorized_user.clone(), tx));

    let _ = route_thread_reply(
        &thread_ts,
        &authorized_user,
        "some text",
        Arc::clone(&pending),
    )
    .await;

    let guard = pending.lock().await;
    assert!(
        !guard.contains_key(&thread_ts),
        "entry should be removed after capture (S031)"
    );
}

// ── T042 / S032 ───────────────────────────────────────────────────────────────

/// Multiple replies — only first captured.
///
/// A second reply after the entry has been consumed must return `Ok(false)`
/// rather than panicking or erroring.
#[tokio::test]
async fn test_s032_only_first_reply_captured() {
    use agent_intercom::slack::handlers::thread_reply::route_thread_reply;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let thread_ts = "1234567890.000400".to_owned();
    let authorized_user = "U12345".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    pending
        .lock()
        .await
        .insert(thread_ts.clone(), (authorized_user.clone(), tx));

    // First reply — should succeed.
    let first = route_thread_reply(
        &thread_ts,
        &authorized_user,
        "first reply",
        Arc::clone(&pending),
    )
    .await;
    assert!(first.is_ok(), "first reply should succeed");
    assert_eq!(rx.await.unwrap(), "first reply");

    // Second reply — no pending entry, should return Ok(false) not error.
    let second = route_thread_reply(
        &thread_ts,
        &authorized_user,
        "second reply",
        Arc::clone(&pending),
    )
    .await;
    assert!(
        second.is_ok(),
        "second reply should be silently ignored (no panic, no error)"
    );
    assert!(
        !second.unwrap(),
        "second reply should return Ok(false) — entry already consumed"
    );
}

// ── T043 / S033 ───────────────────────────────────────────────────────────────

/// Unauthorized user reply rejected (not forwarded).
///
/// A reply from a user who is NOT the authorized operator must be silently
/// ignored and the pending entry must remain so the authorized user can still
/// reply.
#[tokio::test]
async fn test_s033_unauthorized_user_rejected() {
    use agent_intercom::slack::handlers::thread_reply::route_thread_reply;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let thread_ts = "1234567890.000500".to_owned();
    let authorized_user = "U_AUTHORIZED".to_owned();
    let unauthorized_user = "U_BADACTOR".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    pending
        .lock()
        .await
        .insert(thread_ts.clone(), (authorized_user.clone(), tx));

    // Unauthorized user sends reply — should be silently ignored.
    let result = route_thread_reply(
        &thread_ts,
        &unauthorized_user, // sender
        "malicious reply",
        Arc::clone(&pending),
    )
    .await;

    assert!(result.is_ok(), "unauthorized reply should not return Err");
    assert!(
        !result.unwrap(),
        "unauthorized reply should return Ok(false)"
    );

    // Entry should still be present — not consumed by unauthorized reply.
    {
        let guard = pending.lock().await;
        assert!(
            guard.contains_key(&thread_ts),
            "entry should remain after unauthorized reply"
        );
    }

    // Drop the sender (simulate cleanup) and verify rx was never sent.
    pending.lock().await.remove(&thread_ts);
    let recv_result = rx.await;
    assert!(
        recv_result.is_err(),
        "unauthorized reply should not have been forwarded through oneshot"
    );
}
