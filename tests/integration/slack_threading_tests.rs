//! Integration tests for multi-session thread routing isolation (Phase 3, Task 3.5).
//!
//! Verifies that thread-reply fallbacks registered for different threads within
//! the same channel are isolated: a reply to Session A's thread does not resolve
//! Session B's pending entry (S-T1-024).
//!
//! Scenarios covered:
//! - S-T1-024: Two sessions, same channel, different `thread_ts` → action in
//!   Session A only affects Session A's state.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{oneshot, Mutex};

use agent_intercom::slack::handlers::thread_reply::{
    fallback_map_key, register_thread_reply_fallback, route_thread_reply,
};

// ── Shared helpers ────────────────────────────────────────────────────────────

type PendingReplies =
    Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;

fn make_pending() -> PendingReplies {
    Arc::new(Mutex::new(HashMap::new()))
}

// ── S-T1-024: Multi-session thread routing — correct session resolved ─────────

/// S-T1-024 — When two sessions are registered in the same channel with
/// different `thread_ts` values, a reply sent to Session A's thread resolves
/// only Session A's oneshot and leaves Session B's entry untouched.
///
/// Verifies the composite-key design (`"{channel_id}\x1f{thread_ts}"`) prevents
/// cross-session interference (FR-006, CS-02 / LC-05).
#[tokio::test]
async fn button_action_in_session_a_thread_only_affects_session_a() {
    let pending = make_pending();
    let channel_id = "C_SHARED_CHANNEL";

    // Session A: thread_ts "111.000"
    let thread_ts_a = "1700000000.000111".to_owned();
    let session_id_a = "session-thread-A".to_owned();
    let user_a = "U_OPERATOR_A".to_owned();

    // Session B: thread_ts "222.000" — same channel, different thread
    let thread_ts_b = "1700000000.000222".to_owned();
    let session_id_b = "session-thread-B".to_owned();
    let user_b = "U_OPERATOR_B".to_owned();

    // Register Session A's fallback.
    let (tx_a, rx_a) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts_a.clone(),
        session_id_a.clone(),
        user_a.clone(),
        tx_a,
        Arc::clone(&pending),
    )
    .await;

    // Register Session B's fallback in the same channel.
    let (tx_b, mut rx_b) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        channel_id,
        thread_ts_b.clone(),
        session_id_b.clone(),
        user_b.clone(),
        tx_b,
        Arc::clone(&pending),
    )
    .await;

    // Confirm both entries are registered under distinct keys.
    let key_a = fallback_map_key(channel_id, &thread_ts_a);
    let key_b = fallback_map_key(channel_id, &thread_ts_b);
    assert_ne!(key_a, key_b, "Session A and B must have distinct composite keys");
    {
        let guard = pending.lock().await;
        assert!(guard.contains_key(&key_a), "Session A must be registered");
        assert!(guard.contains_key(&key_b), "Session B must be registered");
    }

    // A reply arrives in Session A's thread from user_a.
    let routed = route_thread_reply(
        channel_id,
        &thread_ts_a,
        &user_a,
        "continue working on the parser",
        Arc::clone(&pending),
    )
    .await;

    assert_eq!(
        routed,
        Ok(true),
        "S-T1-024: route_thread_reply for Session A must return Ok(true)"
    );

    // Session A's oneshot must deliver the reply.
    let delivered_a = tokio::time::timeout(Duration::from_millis(500), rx_a)
        .await
        .expect("Session A oneshot must resolve within timeout")
        .expect("Session A oneshot channel must not be dropped");

    assert_eq!(
        delivered_a, "continue working on the parser",
        "S-T1-024: Session A must receive the correct reply text"
    );

    // Session A's entry must be removed from the pending map.
    assert!(
        !pending.lock().await.contains_key(&key_a),
        "S-T1-024: Session A's pending entry must be removed after reply is routed"
    );

    // Session B's entry must remain — it was not affected by Session A's reply.
    assert!(
        pending.lock().await.contains_key(&key_b),
        "S-T1-024: Session B's pending entry must be untouched by Session A's reply"
    );

    // Session B's oneshot must not have resolved.
    let not_resolved = rx_b.try_recv();
    assert!(
        not_resolved.is_err(),
        "S-T1-024: Session B's oneshot must not be resolved by Session A's reply"
    );
}

// ── Cross-channel isolation ───────────────────────────────────────────────────

/// A reply in channel C1 must not affect a pending entry in channel C2,
/// even when both entries share the same `thread_ts`.
///
/// Reinforces the composite-key design: channel differences produce distinct
/// keys, preventing cross-channel contamination.
#[tokio::test]
async fn reply_in_channel_c1_does_not_affect_entry_in_channel_c2() {
    let pending = make_pending();
    let thread_ts = "1700000000.000999".to_owned();
    let user = "U_OPERATOR".to_owned();

    // Register the same thread_ts in two different channels.
    let (tx_c1, rx_c1) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        "C_CHANNEL_ONE",
        thread_ts.clone(),
        "session-c1".to_owned(),
        user.clone(),
        tx_c1,
        Arc::clone(&pending),
    )
    .await;

    let (tx_c2, mut rx_c2) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        "C_CHANNEL_TWO",
        thread_ts.clone(),
        "session-c2".to_owned(),
        user.clone(),
        tx_c2,
        Arc::clone(&pending),
    )
    .await;

    // Reply arrives in channel C1 only.
    let routed = route_thread_reply(
        "C_CHANNEL_ONE",
        &thread_ts,
        &user,
        "reply for c1",
        Arc::clone(&pending),
    )
    .await;

    assert_eq!(routed, Ok(true), "C1 reply must route to C1's entry");

    // C1 oneshot resolves.
    let delivered_c1 = tokio::time::timeout(Duration::from_millis(200), rx_c1)
        .await
        .expect("C1 oneshot must resolve")
        .expect("C1 oneshot must not be dropped");
    assert_eq!(delivered_c1, "reply for c1");

    // C2 entry must be unaffected.
    assert!(
        rx_c2.try_recv().is_err(),
        "C2 oneshot must not be resolved by a reply in C1"
    );
    let key_c2 = fallback_map_key("C_CHANNEL_TWO", &thread_ts);
    assert!(
        pending.lock().await.contains_key(&key_c2),
        "C2 pending entry must survive a reply in a different channel"
    );
}
