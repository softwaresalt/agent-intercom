//! Unit tests for thread-reply fallback (F-16, F-17) — S029–S033,
//! and `parse_thread_decision` (US17 — T081).
//!
//! These tests verify the core `register_thread_reply_fallback` and
//! `route_thread_reply` functions in isolation, without requiring a full
//! `AppState` construction.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};

/// Convenience alias matching the production type after Fix B (3-tuple value).
type PendingThreadReplies = Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;

// ── T039 / S029 ───────────────────────────────────────────────────────────────

/// Modal failure triggers fallback message registration.
///
/// When `open_modal` fails, a oneshot sender should be registered in
/// `pending_thread_replies` keyed by the composite `"{channel_id}\x1f{thread_ts}"`.
#[tokio::test]
async fn test_s029_fallback_message_registration() {
    use agent_intercom::slack::handlers::thread_reply::{
        fallback_map_key, register_thread_reply_fallback,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C12345";
    let thread_ts = "1234567890.000100".to_owned();

    let (tx, _rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-001".to_owned(),
        "U12345".to_owned(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    let guard = pending.lock().await;
    let key = fallback_map_key(channel_id, &thread_ts);
    assert!(
        guard.contains_key(&key),
        "sender should be registered for composite (channel_id, thread_ts) key"
    );
}

// ── T040 / S030 ───────────────────────────────────────────────────────────────

/// Thread reply captured and routed to waiting oneshot.
///
/// When an authorized user replies in the fallback thread, the reply text
/// must be delivered through the registered oneshot channel.
#[tokio::test]
async fn test_s030_reply_routes_to_oneshot() {
    use agent_intercom::slack::handlers::thread_reply::{fallback_map_key, route_thread_reply};

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C12345";
    let thread_ts = "1234567890.000200".to_owned();
    let authorized_user = "U12345".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    let key = fallback_map_key(channel_id, &thread_ts);
    pending
        .lock()
        .await
        .insert(key, ("session-001".to_owned(), authorized_user.clone(), tx));

    let result = route_thread_reply(
        channel_id,
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
    use agent_intercom::slack::handlers::thread_reply::{fallback_map_key, route_thread_reply};

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C12345";
    let thread_ts = "1234567890.000300".to_owned();
    let authorized_user = "U12345".to_owned();

    let (tx, _rx) = oneshot::channel::<String>();
    let key = fallback_map_key(channel_id, &thread_ts);
    pending.lock().await.insert(
        key.clone(),
        ("session-001".to_owned(), authorized_user.clone(), tx),
    );

    let _ = route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized_user,
        "some text",
        Arc::clone(&pending),
    )
    .await;

    let guard = pending.lock().await;
    assert!(
        !guard.contains_key(&key),
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
    use agent_intercom::slack::handlers::thread_reply::{fallback_map_key, route_thread_reply};

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C12345";
    let thread_ts = "1234567890.000400".to_owned();
    let authorized_user = "U12345".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    let key = fallback_map_key(channel_id, &thread_ts);
    pending
        .lock()
        .await
        .insert(key, ("session-001".to_owned(), authorized_user.clone(), tx));

    // First reply — should succeed.
    let first = route_thread_reply(
        channel_id,
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
        channel_id,
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
    use agent_intercom::slack::handlers::thread_reply::{fallback_map_key, route_thread_reply};

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C12345";
    let thread_ts = "1234567890.000500".to_owned();
    let authorized_user = "U_AUTHORIZED".to_owned();
    let unauthorized_user = "U_BADACTOR".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    let key = fallback_map_key(channel_id, &thread_ts);
    pending.lock().await.insert(
        key.clone(),
        ("session-001".to_owned(), authorized_user.clone(), tx),
    );

    // Unauthorized user sends reply — should be silently ignored.
    let result = route_thread_reply(
        channel_id,
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
            guard.contains_key(&key),
            "entry should remain after unauthorized reply"
        );
    }

    // Drop the sender (simulate cleanup) and verify rx was never sent.
    pending.lock().await.remove(&key);
    let recv_result = rx.await;
    assert!(
        recv_result.is_err(),
        "unauthorized reply should not have been forwarded through oneshot"
    );
}

// ── T044b / S034 — composite key isolation ────────────────────────────────────

/// Two entries with the same `thread_ts` but different channels must be
/// independent (Fix E — CS-02 / LC-05: composite key prevents cross-channel
/// collision).
#[tokio::test]
async fn test_s034_composite_key_prevents_cross_channel_collision() {
    use agent_intercom::slack::handlers::thread_reply::{
        fallback_map_key, register_thread_reply_fallback, route_thread_reply,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let thread_ts = "1700000000.000001".to_owned(); // same timestamp in both channels

    // Register in channel A.
    let (tx_a, rx_a) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        "C_CHANNEL_A",
        thread_ts.clone(),
        "session-a".to_owned(),
        "U_OP_A".to_owned(),
        tx_a,
        Arc::clone(&pending),
    )
    .await;

    // Register in channel B.
    let (tx_b, rx_b) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        "C_CHANNEL_B",
        thread_ts.clone(),
        "session-b".to_owned(),
        "U_OP_B".to_owned(),
        tx_b,
        Arc::clone(&pending),
    )
    .await;

    // Both keys should be present.
    {
        let guard = pending.lock().await;
        assert_eq!(guard.len(), 2, "both channel entries should be distinct");
        assert!(guard.contains_key(&fallback_map_key("C_CHANNEL_A", &thread_ts)));
        assert!(guard.contains_key(&fallback_map_key("C_CHANNEL_B", &thread_ts)));
    }

    // Reply in channel A — must only resolve channel A.
    let res_a = route_thread_reply(
        "C_CHANNEL_A",
        &thread_ts,
        "U_OP_A",
        "reply for A",
        Arc::clone(&pending),
    )
    .await;
    assert!(res_a.is_ok_and(|v| v), "channel A reply should be captured");
    assert_eq!(rx_a.await.unwrap(), "reply for A");

    // Channel B entry must still be present.
    assert!(
        pending
            .lock()
            .await
            .contains_key(&fallback_map_key("C_CHANNEL_B", &thread_ts)),
        "channel B entry must survive channel A reply"
    );

    // Clean up channel B.
    let res_b = route_thread_reply(
        "C_CHANNEL_B",
        &thread_ts,
        "U_OP_B",
        "reply for B",
        Arc::clone(&pending),
    )
    .await;
    assert!(res_b.is_ok_and(|v| v), "channel B reply should be captured");
    assert_eq!(rx_b.await.unwrap(), "reply for B");
}

// ── T060 / S036 — duplicate registration preserves original ───────────────────

/// S036 — Registering a fallback for a key that already exists must drop the
/// second sender and preserve the original entry (LC-04).
///
/// Verification:
/// 1. `rx_second` resolves to `Err` (second `tx` was dropped).
/// 2. The map still has exactly 1 entry.
/// 3. The original `tx`/`rx` pair is still usable (routable via `route_thread_reply`).
#[tokio::test]
async fn test_s036_duplicate_registration_preserves_original() {
    use agent_intercom::slack::handlers::thread_reply::{
        register_thread_reply_fallback, route_thread_reply,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C99";
    let thread_ts = "9999.0001".to_owned();

    // First registration.
    let (tx_first, rx_first) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-s036".to_owned(),
        "U_FIRST".to_owned(),
        tx_first,
        Arc::clone(&pending),
    )
    .await;

    // Second registration for same key — must be rejected (LC-04).
    let (tx_second, rx_second) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-s036".to_owned(),
        "U_SECOND".to_owned(),
        tx_second,
        Arc::clone(&pending),
    )
    .await;

    // Map must still have exactly 1 entry.
    assert_eq!(
        pending.lock().await.len(),
        1,
        "map must have exactly 1 entry after duplicate registration"
    );

    // Second tx must have been dropped → rx_second resolves to Err.
    assert!(
        rx_second.await.is_err(),
        "second rx must resolve to Err because second tx was dropped (LC-04)"
    );

    // Original entry must still be routable — route_thread_reply finds U_FIRST.
    let route_result = route_thread_reply(
        channel_id,
        &thread_ts,
        "U_FIRST",
        "original reply",
        Arc::clone(&pending),
    )
    .await;
    assert!(
        route_result.is_ok_and(|v| v),
        "original entry must still be routable after duplicate was rejected"
    );
    assert_eq!(
        rx_first.await.ok().as_deref(),
        Some("original reply"),
        "original rx must receive the reply"
    );
}

// ── T058 / S037 — sender drop allows rx to detect cleanup ─────────────────────

/// S037 — When the registered sender is dropped (e.g., by
/// `cleanup_session_fallbacks`), the `rx` end must resolve to `Err`.
///
/// This verifies the "sender dropped → task exits" observer path.
#[tokio::test]
async fn test_s037_sender_drop_allows_rx_to_detect_cleanup() {
    use agent_intercom::slack::handlers::thread_reply::register_thread_reply_fallback;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));
    let channel_id = "C_DROP";
    let thread_ts = "1111.0001".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-s037".to_owned(),
        "U_OP".to_owned(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Simulate cleanup by removing the entry (which drops the stored tx).
    pending.lock().await.clear();

    // rx must now resolve to Err because the sender was dropped.
    assert!(
        rx.await.is_err(),
        "rx must receive Err when sender is dropped via cleanup"
    );
}

// ── T058 / S038 — route_thread_reply no match returns false ───────────────────

/// S038 — Calling `route_thread_reply` for a `channel/thread_ts` with no pending
/// entry must return `Ok(false)` — not an error, and not `true`.
#[tokio::test]
async fn test_s038_route_thread_reply_no_match_returns_false() {
    use agent_intercom::slack::handlers::thread_reply::route_thread_reply;

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));

    let result = route_thread_reply(
        "C_NONEXISTENT",
        "0000.0000",
        "U_ANY",
        "some reply",
        Arc::clone(&pending),
    )
    .await;

    assert!(result.is_ok(), "missing entry must not return Err");
    assert!(
        !result.unwrap(),
        "missing entry must return Ok(false), not Ok(true)"
    );
}

// ── T045 / S035 — cleanup_session_fallbacks ───────────────────────────────────

/// `cleanup_session_fallbacks` removes all entries for the terminated session
/// and leaves entries for other sessions intact (Fix B — F-20).
#[tokio::test]
async fn test_s035_cleanup_session_fallbacks_removes_correct_entries() {
    use agent_intercom::slack::handlers::thread_reply::{
        cleanup_session_fallbacks, fallback_map_key,
    };

    let pending: PendingThreadReplies = Arc::new(Mutex::new(HashMap::new()));

    let (tx_a1, _rx_a1) = oneshot::channel::<String>();
    let (tx_a2, _rx_a2) = oneshot::channel::<String>();
    let (tx_b1, _rx_b1) = oneshot::channel::<String>();

    {
        let mut guard = pending.lock().await;
        // Two entries for session-A in different channels.
        guard.insert(
            fallback_map_key("C1", "1700000001.000001"),
            ("session-A".to_owned(), "U1".to_owned(), tx_a1),
        );
        guard.insert(
            fallback_map_key("C2", "1700000002.000001"),
            ("session-A".to_owned(), "U2".to_owned(), tx_a2),
        );
        // One entry for session-B.
        guard.insert(
            fallback_map_key("C1", "1700000003.000001"),
            ("session-B".to_owned(), "U3".to_owned(), tx_b1),
        );
    }

    // Terminate session-A — both its entries should be removed.
    cleanup_session_fallbacks("session-A", &pending).await;

    let guard = pending.lock().await;
    assert!(
        !guard.contains_key(&fallback_map_key("C1", "1700000001.000001")),
        "session-A entry 1 must be removed"
    );
    assert!(
        !guard.contains_key(&fallback_map_key("C2", "1700000002.000001")),
        "session-A entry 2 must be removed"
    );
    assert!(
        guard.contains_key(&fallback_map_key("C1", "1700000003.000001")),
        "session-B entry must remain after session-A cleanup"
    );
}

// ── T081 / US17 — parse_thread_decision ───────────────────────────────────────

/// `parse_thread_decision` extracts `continue` from bare keyword.
#[test]
fn test_parse_thread_decision_continue() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("continue");
    assert_eq!(d.keyword, "continue");
    assert!(d.instruction.is_empty());
}

/// `parse_thread_decision` extracts `refine` with trailing instruction text.
#[test]
fn test_parse_thread_decision_refine_with_instruction() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("refine fix the error handling");
    assert_eq!(d.keyword, "refine");
    assert_eq!(d.instruction, "fix the error handling");
}

/// `parse_thread_decision` extracts `stop` keyword.
#[test]
fn test_parse_thread_decision_stop() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("stop");
    assert_eq!(d.keyword, "stop");
    assert!(d.instruction.is_empty());
}

/// `parse_thread_decision` extracts `approve` keyword.
#[test]
fn test_parse_thread_decision_approve() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("approve");
    assert_eq!(d.keyword, "approve");
    assert!(d.instruction.is_empty());
}

/// `parse_thread_decision` extracts `reject` with reason text.
#[test]
fn test_parse_thread_decision_reject_with_reason() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("reject the path is wrong");
    assert_eq!(d.keyword, "reject");
    assert_eq!(d.instruction, "the path is wrong");
}

/// `parse_thread_decision` extracts `resume` with optional instruction.
#[test]
fn test_parse_thread_decision_resume_with_instruction() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("resume check the database schema");
    assert_eq!(d.keyword, "resume");
    assert_eq!(d.instruction, "check the database schema");
}

/// `parse_thread_decision` extracts `resume` without instruction.
#[test]
fn test_parse_thread_decision_resume_bare() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("resume");
    assert_eq!(d.keyword, "resume");
    assert!(d.instruction.is_empty());
}

/// Empty input defaults to `continue` (matches FR-008 auto-continue).
#[test]
fn test_parse_thread_decision_empty_defaults_continue() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("");
    assert_eq!(d.keyword, "continue");
    assert!(d.instruction.is_empty());
}

/// Whitespace-only input defaults to `continue`.
#[test]
fn test_parse_thread_decision_whitespace_defaults_continue() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("   ");
    assert_eq!(d.keyword, "continue");
    assert!(d.instruction.is_empty());
}

/// Unknown keyword is passed through as-is.
#[test]
fn test_parse_thread_decision_unknown_keyword() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("unknown stuff here");
    assert_eq!(d.keyword, "unknown");
    assert_eq!(d.instruction, "stuff here");
}

/// Keywords are case-insensitive.
#[test]
fn test_parse_thread_decision_case_insensitive() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("CONTINUE");
    assert_eq!(d.keyword, "continue");

    let d2 = parse_thread_decision("Refine do this thing");
    assert_eq!(d2.keyword, "refine");
    assert_eq!(d2.instruction, "do this thing");

    let d3 = parse_thread_decision("APPROVE");
    assert_eq!(d3.keyword, "approve");
}

/// Leading/trailing whitespace is trimmed.
#[test]
fn test_parse_thread_decision_trims_whitespace() {
    use agent_intercom::slack::handlers::thread_reply::parse_thread_decision;

    let d = parse_thread_decision("  continue  ");
    assert_eq!(d.keyword, "continue");
    assert!(d.instruction.is_empty());

    let d2 = parse_thread_decision("  refine   fix this  ");
    assert_eq!(d2.keyword, "refine");
    assert_eq!(d2.instruction, "fix this");
}
