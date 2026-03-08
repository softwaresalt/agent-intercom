//! Thread-reply fallback handler (F-16, F-17 — US4).
//!
//! When Slack modals cannot be opened (e.g. due to `trigger_id` expiry in
//! Socket Mode), this module provides the fallback mechanism: a message is
//! posted in the interaction thread asking the operator to reply with text,
//! and a [`tokio::sync::oneshot`] sender is registered. When the operator
//! replies, [`route_thread_reply`] captures the first reply from the
//! authorized user and delivers it through the oneshot channel.
//!
//! ## Design
//!
//! * **Single-entry**: Only the first authorized reply is forwarded. The map
//!   entry is removed atomically to prevent double-delivery.
//! * **Unauthorized replies**: Silently ignored — the entry stays registered
//!   so the authorized operator can still respond.
//! * **Thread-safety**: All state lives in a `Mutex<HashMap>` behind an `Arc`
//!   so the map can be shared across async tasks without contention issues.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

/// Thread-safe map type for pending thread-reply oneshot senders.
///
/// Keyed by `thread_ts`. The value is a `(authorized_user_id, oneshot::Sender<String>)` pair
/// so that only the specific operator who triggered the fallback can complete it.
pub type PendingThreadReplies = Arc<Mutex<HashMap<String, (String, oneshot::Sender<String>)>>>;

/// Register a thread-reply fallback by storing the authorized user and oneshot sender keyed by
/// `thread_ts`.
///
/// Called immediately after a `views.open` failure. The companion
/// [`route_thread_reply`] function will deliver the operator's reply text
/// through this sender when `authorized_user_id` replies in the thread.
///
/// # Arguments
///
/// * `thread_ts` — Slack message timestamp identifying the fallback thread.
/// * `authorized_user_id` — The single Slack user ID allowed to complete this fallback.
/// * `tx` — The oneshot sender. The spawned fallback task holds the `rx` end.
/// * `pending` — Shared map of pending thread-reply senders.
pub async fn register_thread_reply_fallback(
    thread_ts: String,
    authorized_user_id: String,
    tx: oneshot::Sender<String>,
    pending: PendingThreadReplies,
) {
    let mut guard = pending.lock().await;
    guard.insert(thread_ts, (authorized_user_id, tx));
}

/// Route an incoming thread reply to the waiting oneshot sender.
///
/// Looks up `thread_ts` in the pending map and extracts the authorized user from
/// the stored pair. If `sender_user_id` matches the stored authorized user, removes
/// the entry and sends `text` through the oneshot. Unauthorized senders are silently
/// ignored — the entry remains so the authorized user can still reply.
///
/// # Returns
///
/// * `Ok(true)` — reply was captured and forwarded.
/// * `Ok(false)` — no pending entry found **or** unauthorized sender; no action taken.
/// * `Err(String)` — the oneshot channel was dropped before the send could complete.
///
/// # Arguments
///
/// * `thread_ts` — Slack thread timestamp to look up in the pending map.
/// * `sender_user_id` — Slack user ID of the person who replied.
/// * `text` — The raw text of the thread reply.
/// * `pending` — Shared map of pending thread-reply senders.
///
/// # Errors
///
/// Returns an error string if the oneshot receiver was dropped before delivery.
pub async fn route_thread_reply(
    thread_ts: &str,
    sender_user_id: &str,
    text: &str,
    pending: PendingThreadReplies,
) -> Result<bool, String> {
    let mut guard = pending.lock().await;

    if !guard.contains_key(thread_ts) {
        return Ok(false); // no pending fallback for this thread
    }

    let (ref authorized_user_id, _) = guard[thread_ts];
    if sender_user_id != authorized_user_id.as_str() {
        warn!(
            thread_ts,
            sender_user_id,
            authorized_user_id = authorized_user_id.as_str(),
            "unauthorized thread reply ignored (F-16/F-17 fallback)"
        );
        return Ok(false); // silently ignore unauthorized sender; entry remains
    }

    // Remove and send — only the first authorized reply is captured.
    let (_, tx) = guard
        .remove(thread_ts)
        .ok_or_else(|| "oneshot sender disappeared during lock".to_owned())?;
    drop(guard); // release lock before send to minimize contention

    if tx.send(text.to_owned()).is_err() {
        warn!(
            thread_ts,
            "thread-reply oneshot receiver already dropped — reply discarded"
        );
        return Err("oneshot receiver dropped before reply could be delivered".to_owned());
    }

    info!(
        thread_ts,
        sender_user_id, "thread reply captured and routed (F-16/F-17)"
    );
    Ok(true)
}
