//! Live Slack threading tests — Tier 2.
//!
//! Verifies that messages posted to distinct session threads stay in their
//! respective threads and do not bleed across thread boundaries.
//!
//! These tests use only the `LiveSlackClient` (no handler dispatch or DB),
//! making them a pure Slack API threading correctness check.
//!
//! Scenarios covered:
//! - S-T2-003: Multi-session thread isolation — messages appear only in their
//!   target thread, not in the other session's thread.

use uuid::Uuid;

use super::live_helpers::{LiveSlackClient, LiveTestConfig};

// ── S-T2-003: Multi-session thread isolation ──────────────────────────────────

/// S-T2-003: Create two independent session thread anchors in the same channel.
/// Post a distinct broadcast message into each thread. Verify:
/// - Thread A's replies contain Session A's message and NOT Session B's message.
/// - Thread B's replies contain Session B's message and NOT Session A's message.
///
/// This exercises the threading model that all session-scoped messages use:
/// every session thread is anchored by its session-started `ts`, and all
/// subsequent messages for that session are posted as replies to that `ts`.
///
/// Scenario: S-T2-003 | FRs: FR-018
#[tokio::test]
#[allow(clippy::similar_names)] // `anchor_a_*` / `anchor_b_*` are intentionally parallel names
async fn two_sessions_in_separate_threads_are_isolated() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping two_sessions_in_separate_threads_are_isolated: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();

    // ── Anchor A — simulates Session A's "session started" message ───────────
    let anchor_a_text = format!("[live-test] session A anchor (run {run_id:.8})");
    let anchor_a_ts = client
        .post_test_message(&config.channel_id, &anchor_a_text)
        .await
        .expect("post session A anchor");

    // ── Anchor B — simulates Session B's "session started" message ───────────
    let anchor_b_text = format!("[live-test] session B anchor (run {run_id:.8})");
    let anchor_b_ts = client
        .post_test_message(&config.channel_id, &anchor_b_text)
        .await
        .expect("post session B anchor");

    // ── Post a broadcast reply into Session A's thread ────────────────────────
    let marker_a = format!("broadcast-session-A-{run_id:.8}");
    client
        .post_thread_message(
            &config.channel_id,
            &anchor_a_ts,
            &format!("[live-test] {marker_a}"),
        )
        .await
        .expect("post reply into session A thread");

    // ── Post a broadcast reply into Session B's thread ────────────────────────
    let marker_b = format!("broadcast-session-B-{run_id:.8}");
    client
        .post_thread_message(
            &config.channel_id,
            &anchor_b_ts,
            &format!("[live-test] {marker_b}"),
        )
        .await
        .expect("post reply into session B thread");

    // ── Retrieve Session A's thread and assert isolation ──────────────────────
    let replies_a = client
        .get_thread_replies(&config.channel_id, &anchor_a_ts)
        .await
        .expect("get session A thread replies");

    let a_texts: Vec<&str> = replies_a
        .iter()
        .filter_map(|m| m["text"].as_str())
        .collect();

    assert!(
        a_texts.iter().any(|t| t.contains(&marker_a)),
        "session A thread must contain session A's broadcast message; replies: {a_texts:?}"
    );
    assert!(
        !a_texts.iter().any(|t| t.contains(&marker_b)),
        "session A thread must NOT contain session B's broadcast message; replies: {a_texts:?}"
    );

    // ── Retrieve Session B's thread and assert isolation ──────────────────────
    let replies_b = client
        .get_thread_replies(&config.channel_id, &anchor_b_ts)
        .await
        .expect("get session B thread replies");

    let b_texts: Vec<&str> = replies_b
        .iter()
        .filter_map(|m| m["text"].as_str())
        .collect();

    assert!(
        b_texts.iter().any(|t| t.contains(&marker_b)),
        "session B thread must contain session B's broadcast message; replies: {b_texts:?}"
    );
    assert!(
        !b_texts.iter().any(|t| t.contains(&marker_a)),
        "session B thread must NOT contain session A's broadcast message; replies: {b_texts:?}"
    );

    // ── Cleanup both anchors (which removes associated thread replies) ─────────
    client
        .cleanup_test_messages(
            &config.channel_id,
            &[anchor_a_ts.as_str(), anchor_b_ts.as_str()],
        )
        .await
        .expect("cleanup session anchors should succeed");
}

// ── Additional: Block Kit threaded message stays in correct thread ────────────

/// Post Block Kit blocks as a thread reply and verify they are retrievable via
/// `conversations.replies` with correct block structure — not just plain text.
///
/// This ensures that session broadcast messages with Block Kit formatting are
/// correctly threaded, which is the normal production path for all event
/// notifications (stall alerts, prompt messages, approval follow-ups).
///
/// Scenario: S-T2-002 (supplemental) | FRs: FR-018
#[tokio::test]
async fn block_kit_threaded_reply_is_retrievable() {
    use agent_intercom::slack::blocks;

    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping block_kit_threaded_reply_is_retrievable: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();

    // Post the thread anchor (session-started message).
    let anchor_text = format!("[live-test] thread anchor for block-kit test (run {run_id:.8})");
    let anchor_ts = client
        .post_test_message(&config.channel_id, &anchor_text)
        .await
        .expect("post thread anchor");

    // Build a stall alert block (representative threaded message type).
    let alert_blocks = blocks::stall_alert_blocks("live-test-stall", 120);
    let blocks_json = serde_json::to_value(&alert_blocks).expect("serialize stall blocks");
    let reply_text = format!("[live-test] stall alert in thread (run {run_id:.8})");

    let reply_ts = client
        .post_thread_blocks(&config.channel_id, &anchor_ts, &reply_text, blocks_json)
        .await
        .expect("post thread block reply");

    // Retrieve the thread.
    let replies = client
        .get_thread_replies(&config.channel_id, &anchor_ts)
        .await
        .expect("get thread replies");

    // Find the block Kit reply.
    let block_reply = replies
        .iter()
        .find(|m| m["ts"].as_str() == Some(reply_ts.as_str()))
        .expect("block-kit reply must be found in thread");

    // Verify block structure — stall alert contains the nudge action ID.
    let blocks_json_str =
        serde_json::to_string(&block_reply["blocks"]).unwrap_or_else(|_| "null".to_owned());
    assert!(
        blocks_json_str.contains("stall_nudge"),
        "stall alert reply must contain 'stall_nudge' action ID; got: {blocks_json_str}"
    );

    // Verify thread_ts is the anchor ts (reply is in correct thread).
    assert_eq!(
        block_reply["thread_ts"].as_str().unwrap_or_default(),
        anchor_ts.as_str(),
        "block-kit reply thread_ts must match the anchor ts"
    );

    // Cleanup.
    client
        .cleanup_test_messages(&config.channel_id, &[anchor_ts.as_str(), reply_ts.as_str()])
        .await
        .expect("cleanup should succeed");
}
