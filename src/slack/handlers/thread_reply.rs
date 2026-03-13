//! Thread-reply fallback handler (F-16, F-17 — US4, US17).
//!
//! When Slack modals cannot be opened (e.g. due to `trigger_id` expiry in
//! Socket Mode), this module provides the fallback mechanism: a message is
//! posted in the interaction thread asking the operator to reply with text,
//! and a [`tokio::sync::oneshot`] sender is registered. When the operator
//! replies, [`route_thread_reply`] captures the first reply from the
//! authorized user and delivers it through the oneshot channel.
//!
//! ## Text-only thread prompts (US17)
//!
//! When a session has a `thread_ts`, MCP tool handlers post plain text
//! prompts (no block-kit buttons) and register a thread-reply fallback
//! directly. The operator replies with `@agent-intercom <decision> [text]`.
//! [`parse_thread_decision`] extracts the decision keyword and optional
//! instruction from the stripped mention text.
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
use std::future::Future;
use std::sync::Arc;

use slack_morphism::prelude::{SlackChannelId, SlackTs};
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use crate::slack::blocks;
use crate::slack::client::{SlackMessage, SlackService};

/// Default time limit for a pending thread-reply fallback.
///
/// If the operator does not reply within this window the spawned task exits
/// and the pending map entry is removed.
pub(crate) const FALLBACK_REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

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
    if guard.contains_key(&key) {
        // LC-04: a duplicate registration for the same key is dropped to
        // preserve the original entry (e.g., mobile double-tap guard).
        warn!(
            channel_id,
            thread_ts,
            "thread-reply fallback: duplicate registration for existing key — dropping new sender (LC-04)"
        );
        return; // `tx` is dropped here, making `rx` resolve to `Err`
    }
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

/// Register, post, and spawn the thread-reply fallback path for any handler type.
///
/// Encapsulates the common fallback activation logic shared across `prompt.rs`,
/// `wait.rs`, and `approval.rs` (TQ-008), eliminating ~80 lines of triplication.
///
/// ## Sequence
///
/// 1. Registers the pending entry in `pending` (with duplicate guard — LC-04).
/// 2. Optionally replaces interactive buttons with an ⏳ status (FR-022) if
///    `button_msg_ts` is `Some`.
/// 3. Posts `fallback_text` to the Slack thread as an instruction for the operator.
/// 4. **Zombie-guard (Fix C — CS-05):** only spawns the waiter task if the post
///    succeeded. If posting fails, removes the pending entry and returns `Err`.
/// 5. Spawns a [`tokio::time::timeout`]-wrapped waiter that calls `resolve` with
///    the operator's reply text when it arrives.
///
/// # Arguments
///
/// * `chan_id` — Slack channel ID (for registration key and fallback message).
/// * `thread_ts` — Thread root timestamp (for registration key and reply routing).
/// * `session_id` — Session that owns this fallback (used for cleanup tracking).
/// * `authorized_user_id` — Only this Slack user's reply will be accepted.
/// * `fallback_text` — Instruction to post in the thread (visible to the operator).
/// * `button_msg_ts` — If `Some`, replace the button message with ⏳ status (FR-022).
/// * `slack` — Slack client for posting messages.
/// * `pending` — Shared pending map.
/// * `log_context` — Identifier for tracing log fields (e.g., `"prompt_id=abc"`).
/// * `resolve` — Async callback called with the operator's reply text on success.
///
/// # Errors
///
/// Returns `Err(reason)` if posting the fallback instruction message fails
/// (zombie-guard applied: pending entry removed, waiter task not spawned).
#[allow(clippy::too_many_arguments)] // 10 args needed to encapsulate 3 callers' fallback state
pub async fn activate_thread_reply_fallback<F, Fut>(
    chan_id: &str,
    thread_ts: &str,
    session_id: String,
    authorized_user_id: String,
    fallback_text: &str,
    button_msg_ts: Option<SlackTs>,
    slack: &SlackService,
    pending: PendingThreadReplies,
    log_context: &str,
    resolve: F,
) -> Result<(), String>
where
    F: FnOnce(String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = oneshot::channel::<String>();
    register_thread_reply_fallback(
        chan_id,
        thread_ts.to_owned(),
        session_id,
        authorized_user_id,
        tx,
        Arc::clone(&pending),
    )
    .await;

    // FR-022: Replace buttons with "awaiting thread reply" status immediately.
    if let Some(ts) = button_msg_ts {
        let replacement = vec![blocks::text_section(
            "\u{23f3} *Awaiting thread reply\u{2026}* (modal unavailable \u{2014} please reply in this thread)",
        )];
        if let Err(err) = slack
            .update_message(SlackChannelId(chan_id.to_owned()), ts, replacement)
            .await
        {
            warn!(
                log_context,
                %err,
                "thread-reply fallback: failed to replace buttons (FR-022) — non-fatal"
            );
        }
    }

    // Post fallback instruction in the thread so the operator knows to reply.
    let fallback_msg = SlackMessage {
        channel: SlackChannelId(chan_id.to_owned()),
        text: Some(fallback_text.to_owned()),
        blocks: None,
        thread_ts: Some(SlackTs(thread_ts.to_owned())),
    };

    // Fix C (CS-05/TQ-004): only spawn the waiter if the post succeeded.
    // If the operator never sees a prompt, there is nobody to reply.
    if let Err(post_err) = slack.enqueue(fallback_msg).await {
        warn!(
            log_context,
            %post_err,
            "thread-reply fallback: failed to post fallback message — removing pending entry and aborting (F-16)"
        );
        // Note: if the LC-04 duplicate guard fired above (register returned early
        // without inserting the key), this `remove` is a harmless no-op — the key
        // was never inserted in that path.
        pending
            .lock()
            .await
            .remove(&fallback_map_key(chan_id, thread_ts));
        return Err(format!("failed to post fallback message: {post_err}"));
    }

    // Spawn a task to wait for the operator's reply and call `resolve`.
    let log_ctx = log_context.to_owned();
    tokio::spawn(async move {
        match tokio::time::timeout(FALLBACK_REPLY_TIMEOUT, rx).await {
            Ok(Ok(reply_text)) => resolve(reply_text).await,
            Ok(Err(_)) => {
                // Sender dropped — this is the expected path when the LC-04 duplicate
                // guard fired and dropped `tx`, causing `rx` to resolve immediately to
                // `Err`. The waiter exits cleanly without resolution.
                warn!(
                    context = log_ctx,
                    "thread-reply fallback: sender dropped — task exiting (F-16)"
                );
            }
            Err(_elapsed) => {
                warn!(
                    context = log_ctx,
                    timeout_secs = FALLBACK_REPLY_TIMEOUT.as_secs(),
                    "thread-reply fallback: timed out — task exiting without resolution (F-16)"
                );
            }
        }
    });

    Ok(())
}

/// Parsed decision from an operator's @-mention thread reply (US17).
///
/// The decision keyword is the first whitespace-delimited word of the
/// stripped mention text (after `<@BOT_ID>` removal). Everything after
/// the keyword is the instruction or reason text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadDecision {
    /// Normalized decision keyword: `continue`, `refine`, `stop`,
    /// `approve`, `reject`, `resume`, or the raw first word if unrecognised.
    pub keyword: String,
    /// Remainder of the reply after the decision keyword, trimmed.
    /// Empty string when no additional text was provided.
    pub instruction: String,
}

/// Parse a decision keyword and optional instruction from an operator's
/// thread reply text.
///
/// The input `text` should already have the `<@BOT_ID>` mention prefix
/// stripped (via [`super::steer::strip_mention`]). The first
/// whitespace-delimited token is matched case-insensitively against the
/// known decision keywords. Everything after the first token is returned
/// as the instruction text.
///
/// # Examples
///
/// ```text
/// "continue"                     → { keyword: "continue", instruction: "" }
/// "refine fix the error"         → { keyword: "refine",   instruction: "fix the error" }
/// "stop"                         → { keyword: "stop",     instruction: "" }
/// "approve"                      → { keyword: "approve",  instruction: "" }
/// "reject path is wrong"         → { keyword: "reject",   instruction: "path is wrong" }
/// "resume check schemas"         → { keyword: "resume",   instruction: "check schemas" }
/// "unknown stuff"                → { keyword: "unknown",  instruction: "stuff" }
/// ""                             → { keyword: "continue", instruction: "" }
/// ```
#[must_use]
pub fn parse_thread_decision(text: &str) -> ThreadDecision {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        // Empty reply defaults to "continue" — matches FR-008 auto-continue
        // semantics so an accidental empty mention is non-destructive.
        return ThreadDecision {
            keyword: "continue".to_owned(),
            instruction: String::new(),
        };
    }

    let (first_word, rest) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, ""), |(w, r)| (w, r.trim()));

    let keyword = first_word.to_lowercase();

    ThreadDecision {
        keyword,
        instruction: rest.to_owned(),
    }
}
