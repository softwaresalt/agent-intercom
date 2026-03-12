//! Integration tests for the @-mention thread-reply routing end-to-end flow.
//!
//! These tests cover the combined path:
//!   `register_thread_reply_fallback` (registers pending waiter)
//!   → `route_thread_reply` (resolves pending with stripped @-mention text)
//!
//! This complements the unit tests in `tests/unit/slack_thread_mention_routing.rs`
//! (which test `route_thread_reply` in isolation) and the basic flow test in
//! `tests/integration/thread_reply_integration.rs` (which covers S029–S031).
//! Here we add:
//!   - Concurrent pending entries in different channels
//!   - Unauthorized user guard end-to-end
//!   - Stripped text fidelity
//!   - No-pending-entry no-op
//!   - Channel isolation (two entries, one routed)
//!
//! All tests use the public API from `agent_intercom::slack::handlers::thread_reply`.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};

use agent_intercom::slack::handlers::thread_reply::{
    register_thread_reply_fallback, route_thread_reply,
};

/// Convenience alias mirroring the production type.
type PendingMap = Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;

fn empty_pending() -> PendingMap {
    Arc::new(Mutex::new(HashMap::new()))
}

// ─── AM-001: full register → route cycle with stripped @-mention text ────────

/// `register_thread_reply_fallback` followed by `route_thread_reply` with the
/// already-stripped @-mention text delivers the text through the oneshot.
///
/// This mirrors the production path: `push_events.rs` strips `<@BOTID>` before
/// calling `route_thread_reply`, so the waiter receives only the instruction text.
#[tokio::test]
async fn test_am001_register_then_route_delivers_stripped_text() {
    let pending = empty_pending();
    let channel_id = "C_AM001";
    let thread_ts = "1700000200.000001".to_owned();
    let authorized = "U_AM001_OP".to_owned();

    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-am001".to_owned(),
        authorized.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Simulate push_events.rs stripping '<@UBOTID> ' before routing.
    let stripped_text = "Use a more concise tone";
    let routed = route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized,
        stripped_text,
        Arc::clone(&pending),
    )
    .await
    .expect("AM-001: route_thread_reply must not fail");

    assert!(
        routed,
        "AM-001: route_thread_reply must return Ok(true) for a registered entry"
    );

    let received = rx
        .await
        .expect("AM-001: oneshot rx must resolve after routing");
    assert_eq!(
        received, stripped_text,
        "AM-001: received text must equal the stripped @-mention instruction"
    );

    // The pending map entry must be removed after successful routing.
    let map = pending.lock().await;
    assert!(
        map.is_empty(),
        "AM-001: pending map must be empty after successful route"
    );
}

// ─── AM-002: unauthorized sender is silently ignored; pending entry preserved ─

/// When an unauthorized user replies in the thread, `route_thread_reply` returns
/// `Ok(false)` and the pending entry is preserved so the authorized operator can
/// still respond.
#[tokio::test]
async fn test_am002_unauthorized_sender_ignored_entry_preserved() {
    let pending = empty_pending();
    let channel_id = "C_AM002";
    let thread_ts = "1700000200.000002".to_owned();
    let authorized = "U_AM002_OP".to_owned();
    let intruder = "U_AM002_INTRUDER".to_owned();

    let (tx, mut rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-am002".to_owned(),
        authorized.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    let routed = route_thread_reply(
        channel_id,
        &thread_ts,
        &intruder,
        "unauthorized text",
        Arc::clone(&pending),
    )
    .await
    .expect("AM-002: route_thread_reply must not fail on unauthorized sender");

    assert!(
        !routed,
        "AM-002: route_thread_reply must return Ok(false) for unauthorized sender"
    );

    // The oneshot must NOT have resolved.
    assert!(
        rx.try_recv().is_err(),
        "AM-002: rx must not have resolved after an unauthorized reply"
    );

    // The pending map entry must still be present.
    let map = pending.lock().await;
    assert!(
        !map.is_empty(),
        "AM-002: pending entry must remain after unauthorized reply"
    );
    drop(map);

    // Now the authorized operator can still reply successfully.
    let routed_after = route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized,
        "authorized reply",
        Arc::clone(&pending),
    )
    .await
    .expect("AM-002: second route attempt by authorized user must not fail");

    assert!(
        routed_after,
        "AM-002: authorized reply after intruder must succeed"
    );
}

// ─── AM-003: no pending entry returns Ok(false) — no panic, no side effects ──

/// Calling `route_thread_reply` when no pending entry exists for the
/// `(channel, thread_ts)` pair returns `Ok(false)` without panicking or
/// modifying any state.
#[tokio::test]
async fn test_am003_no_pending_entry_returns_false() {
    let pending = empty_pending();

    let result = route_thread_reply(
        "C_ABSENT",
        "1700000000.000000",
        "U_ANYONE",
        "some text",
        Arc::clone(&pending),
    )
    .await
    .expect("AM-003: route_thread_reply must not error when no pending entry exists");

    assert!(
        !result,
        "AM-003: route_thread_reply must return Ok(false) with no pending entry"
    );

    let map = pending.lock().await;
    assert!(map.is_empty(), "AM-003: pending map must still be empty");
}

// ─── AM-004: channel isolation — two pending entries, only one routed ─────────

/// When two different channels each have a pending fallback for the same
/// `thread_ts` value, routing a reply in channel B only resolves channel B's
/// oneshot; channel A's entry is untouched.
///
/// This validates the composite key (`"{channel_id}\x1f{thread_ts}"`) prevents
/// cross-channel collisions (CS-02 / LC-05 from the production design notes).
#[tokio::test]
async fn test_am004_channel_isolation_only_correct_channel_resolves() {
    let pending = empty_pending();
    let shared_thread_ts = "1700000200.000003".to_owned();

    let channel_a = "C_AM004_A";
    let channel_b = "C_AM004_B";
    let operator_a = "U_AM004_OP_A".to_owned();
    let operator_b = "U_AM004_OP_B".to_owned();

    let (tx_a, mut rx_a) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_a,
        shared_thread_ts.clone(),
        "session-am004-a".to_owned(),
        operator_a.clone(),
        tx_a,
        Arc::clone(&pending),
    )
    .await;

    let (tx_b, rx_b) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_b,
        shared_thread_ts.clone(),
        "session-am004-b".to_owned(),
        operator_b.clone(),
        tx_b,
        Arc::clone(&pending),
    )
    .await;

    // Route a reply in channel B only.
    let routed = route_thread_reply(
        channel_b,
        &shared_thread_ts,
        &operator_b,
        "channel B reply",
        Arc::clone(&pending),
    )
    .await
    .expect("AM-004: routing in channel B must not fail");

    assert!(
        routed,
        "AM-004: route_thread_reply for channel B must succeed"
    );

    // Channel B oneshot resolves.
    let b_received = rx_b.await.expect("AM-004: channel B rx must resolve");
    assert_eq!(
        b_received, "channel B reply",
        "AM-004: channel B text mismatch"
    );

    // Channel A oneshot must NOT have resolved.
    assert!(
        rx_a.try_recv().is_err(),
        "AM-004: channel A rx must not resolve when channel B was routed"
    );

    // Channel A entry remains in the map.
    let map = pending.lock().await;
    assert_eq!(
        map.len(),
        1,
        "AM-004: only channel B entry should have been removed"
    );
}

// ─── AM-005: stripped text fidelity ──────────────────────────────────────────

/// The text delivered through the oneshot must exactly equal the stripped
/// @-mention instruction, including any multi-word input with punctuation.
///
/// Validates that `route_thread_reply` does not further modify the text it
/// receives (e.g., no additional trimming, escaping, or truncation).
#[tokio::test]
async fn test_am005_stripped_text_fidelity() {
    let pending = empty_pending();
    let channel_id = "C_AM005";
    let thread_ts = "1700000200.000005".to_owned();
    let authorized = "U_AM005_OP".to_owned();

    // Instruction text with varied whitespace, punctuation, and Unicode.
    let instruction = "Use a more concise tone — focus on the key points. Keep it under 200 words.";

    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts.clone(),
        "session-am005".to_owned(),
        authorized.clone(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    route_thread_reply(
        channel_id,
        &thread_ts,
        &authorized,
        instruction,
        Arc::clone(&pending),
    )
    .await
    .expect("AM-005: route_thread_reply must not fail");

    let received = rx.await.expect("AM-005: oneshot rx must resolve");
    assert_eq!(
        received, instruction,
        "AM-005: delivered text must exactly equal the stripped instruction"
    );
}
