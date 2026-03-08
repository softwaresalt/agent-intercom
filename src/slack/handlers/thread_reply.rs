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
//! * **Composite key**: The map is keyed by `"{channel_id}\x1f{thread_ts}"` to
//!   prevent cross-channel collisions when two channels happen to share a
//!   timestamp (CS-02 / LC-05).
//! * **Session cleanup**: The stored `session_id` allows [`cleanup_session_fallbacks`]
//!   to drop all entries for a terminated session (F-20).

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

/// Default time limit for a pending thread-reply fallback.
///
/// If the operator does not reply within this window the spawned task exits
/// and the pending map entry is removed.
pub(crate) const FALLBACK_REPLY_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(300);

/// Thread-safe map type for pending thread-reply oneshot senders.
///
/// Keyed by `"{channel_id}\x1f{thread_ts}"` (composite key — ASCII Unit
/// Separator `\x1f` cannot appear in either field). Value is
/// `(session_id, authorized_user_id, Sender)`.
/// Storing `session_id` allows cleanup when the owning session terminates
/// (F-20: [`cleanup_session_fallbacks`]).
pub type PendingThreadReplies =
    Arc<Mutex<HashMap<String, (String, String, oneshot::Sender<String>)>>>;

/// Build the composite map key from channel and thread timestamp.
///
/// Uses ASCII Unit Separator (`\x1f`) as delimiter — cannot appear in
/// either field — to prevent cross-channel collisions (CS-02 / LC-05).
///
/// Exposed as `pub` so integration and unit tests can construct expected keys
/// for assertions without duplicating the format string.
#[must_use]
pub fn fallback_map_key(channel_id: &str, thread_ts: &str) -> String {
    format!("{channel_id}\x1f{thread_ts}")
}

/// Register a thread-reply fallback by storing the session, authorized user,
/// and oneshot sender, keyed by the composite `"{channel_id}\x1f{thread_ts}"`.
///
/// Called immediately after a `views.open` failure. The companion
/// [`route_thread_reply`] function will deliver the operator's reply text
/// through this sender when `authorized_user_id` replies in the thread.
///
/// # Arguments
///
/// * `channel_id` — Slack channel ID where the fallback thread lives.
/// * `thread_ts` — Slack message timestamp identifying the fallback thread.
/// * `session_id` — Session that owns this fallback; used for cleanup on termination.
/// * `authorized_user_id` — The single Slack user ID allowed to complete this fallback.
/// * `tx` — The oneshot sender. The spawned fallback task holds the `rx` end.
/// * `pending` — Shared map of pending thread-reply senders.
pub async fn register_thread_reply_fallback(
    channel_id: &str,
    thread_ts: String,
    session_id: String,
    authorized_user_id: String,
    tx: oneshot::Sender<String>,
    pending: PendingThreadReplies,
) {
    let key = fallback_map_key(channel_id, &thread_ts);
    let mut guard = pending.lock().await;
    guard.insert(key, (session_id, authorized_user_id, tx));
}

/// Route an incoming thread reply to the waiting oneshot sender.
///
/// Looks up the composite key in the pending map and extracts the authorized
/// user from the stored tuple. If `sender_user_id` matches, removes the entry
/// and sends `text` through the oneshot. Unauthorized senders are silently
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
/// * `channel_id` — Slack channel ID to build the composite lookup key.
/// * `thread_ts` — Slack thread timestamp (second component of the composite key).
/// * `sender_user_id` — Slack user ID of the person who replied.
/// * `text` — The raw text of the thread reply.
/// * `pending` — Shared map of pending thread-reply senders.
///
/// # Errors
///
/// Returns an error string if the oneshot receiver was dropped before delivery.
pub async fn route_thread_reply(
    channel_id: &str,
    thread_ts: &str,
    sender_user_id: &str,
    text: &str,
    pending: PendingThreadReplies,
) -> Result<bool, String> {
    let key = fallback_map_key(channel_id, thread_ts);
    let mut guard = pending.lock().await;

    if !guard.contains_key(&key) {
        return Ok(false); // no pending fallback for this thread
    }

    // Clone the authorized user ID to release the shared borrow before remove().
    let authorized_user_id: String = {
        let Some((_, uid, _)) = guard.get(&key) else {
            return Ok(false);
        };
        uid.clone()
    };

    if sender_user_id != authorized_user_id.as_str() {
        warn!(
            channel_id,
            thread_ts,
            sender_user_id,
            authorized_user_id = authorized_user_id.as_str(),
            "unauthorized thread reply ignored (F-16/F-17 fallback)"
        );
        return Ok(false); // silently ignore unauthorized sender; entry remains
    }

    // Remove and send — only the first authorized reply is captured.
    let (_, _, tx) = guard
        .remove(&key)
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

/// Remove all pending thread-reply fallback entries for a terminated session.
///
/// Dropping the `oneshot::Sender` causes the corresponding `rx` in each
/// spawned task to receive `RecvError`, unblocking the task so it can exit
/// cleanly. Called from the session-termination path (Fix B — F-20).
pub async fn cleanup_session_fallbacks(session_id: &str, pending: &PendingThreadReplies) {
    let mut guard = pending.lock().await;
    guard.retain(|_key, entry| entry.0.as_str() != session_id);
}
