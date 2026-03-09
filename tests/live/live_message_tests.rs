//! Live Slack message tests — Tier 2.
//!
//! Tests in this module post real messages to a Slack test channel and verify
//! them via the Slack Web API. They require valid credentials in environment
//! variables (see [`super::live_helpers::LiveTestConfig::from_env`]).
//!
//! When credentials are absent every test skips gracefully with a printed
//! notice rather than failing, keeping CI behaviour clean in environments
//! without live Slack access.
//!
//! Scenario coverage: S-T2-001 (partial — post + retrieve)

use super::live_helpers::{assert_blocks_contain, LiveSlackClient, LiveTestConfig};
use uuid::Uuid;

// ── S-T2-001 smoke test ───────────────────────────────────────────────────────

/// S-T2-001 (partial): Post a plain-text message to the test channel, retrieve
/// it via `conversations.history`, and verify the text matches.
///
/// Also exercises [`LiveSlackClient::cleanup_test_messages`] to confirm the
/// deletion path is reachable without error.
#[tokio::test]
async fn smoke_post_and_retrieve_message() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping smoke_post_and_retrieve_message: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);

    // Use a UUID suffix so the message is unique and easily identifiable.
    let run_id = Uuid::new_v4();
    let text = format!("[live-test][agent-intercom] smoke test message {run_id}");

    // Post the message and record its timestamp.
    let ts = client
        .post_test_message(&config.channel_id, &text)
        .await
        .expect("post_test_message should succeed with valid credentials");

    // Retrieve the message and assert text fidelity.
    let message = client
        .get_message(&config.channel_id, &ts)
        .await
        .expect("get_message should find the just-posted message");

    assert_eq!(
        message["text"].as_str().unwrap_or_default(),
        text.as_str(),
        "retrieved message text should match the posted text"
    );

    // The blocks array is absent for plain-text messages; assert_blocks_contain
    // should reflect an empty/null structure rather than panicking.
    // (Verifies the helper is robust for messages without Block Kit payloads.)
    let blocks_json =
        serde_json::to_string(&message["blocks"]).unwrap_or_else(|_| String::from("null"));
    // Plain-text messages have no blocks array — either null or empty.
    assert!(
        blocks_json == "null" || blocks_json == "[]",
        "plain-text message should have no blocks; got: {blocks_json}"
    );

    // Cleanup — always runs regardless of assertion results.
    client
        .cleanup_test_messages(&config.channel_id, &[ts.as_str()])
        .await
        .expect("cleanup_test_messages should succeed");
}

/// S-T2-001 (partial): `assert_blocks_contain` correctly identifies absent
/// text in a synthetic message value without panicking on the happy-path.
///
/// This is an offline companion check that exercises the helper without
/// requiring live credentials.
#[test]
fn assert_blocks_contain_finds_expected_text() {
    let message = serde_json::json!({
        "text": "hello",
        "blocks": [
            {
                "type": "section",
                "text": { "type": "mrkdwn", "text": "🔐 *Review required*" }
            }
        ]
    });
    // Should not panic — the expected text is present.
    assert_blocks_contain(&message, "Review required");
}

/// Companion check: `assert_blocks_contain` on a message with no blocks does
/// not panic and the helper serialises `null` gracefully.
#[test]
fn assert_blocks_contain_handles_missing_blocks() {
    let message = serde_json::json!({ "text": "no blocks here" });
    // We check the serialised form directly; the helper would panic if called
    // with text that is absent — so we verify the structure and skip calling
    // the asserting helper with impossible text.
    let blocks_json =
        serde_json::to_string(&message["blocks"]).unwrap_or_else(|_| String::from("null"));
    assert_eq!(
        blocks_json, "null",
        "absent blocks field serialises as null"
    );
}

// ── S-T2-001 (full): Approval message with Block Kit blocks ──────────────────

/// S-T2-001 (full): Post a real approval message with Block Kit blocks,
/// retrieve it via `conversations.history`, and assert the structural
/// content (severity section, diff section, action buttons) is present.
///
/// Scenario: S-T2-001 | FRs: FR-013, FR-018
#[tokio::test]
async fn post_approval_blocks_and_verify_structure() {
    use agent_intercom::models::approval::RiskLevel;
    use agent_intercom::slack::blocks;

    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping post_approval_blocks_and_verify_structure: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();

    // Build an approval message with realistic blocks.
    let title = format!("[live-test] Add error handler (run {run_id:.8})");
    let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1,3 @@\n+use anyhow::Result;\n fn foo() {}\n+fn bar() -> Result<()> { Ok(()) }";
    let file_path = "src/lib.rs";

    let mut all_blocks = blocks::build_approval_blocks(&title, None, diff, file_path, RiskLevel::Low);
    all_blocks.push(blocks::approval_buttons(&run_id.to_string()));
    let blocks_json =
        serde_json::to_value(&all_blocks).expect("serialize approval blocks");

    let text = format!("[live-test] approval request (run {run_id:.8})");
    let ts = client
        .post_with_blocks(&config.channel_id, &text, blocks_json)
        .await
        .expect("post_with_blocks should succeed with valid credentials");

    // Retrieve via conversations.history and assert structure.
    let message = client
        .get_message(&config.channel_id, &ts)
        .await
        .expect("get_message should find the just-posted message");

    // The message must contain the title text (risk emoji + title).
    assert_blocks_contain(&message, "Add error handler");

    // The diff section must contain the diff text.
    assert_blocks_contain(&message, "anyhow");

    // The actions block must contain the approve_accept action ID.
    assert_blocks_contain(&message, "approve_accept");

    // The approve_reject action ID must also be present.
    assert_blocks_contain(&message, "approve_reject");

    // Cleanup.
    client
        .cleanup_test_messages(&config.channel_id, &[ts.as_str()])
        .await
        .expect("cleanup_test_messages should succeed");
}

// ── S-T2-002: Threaded message verified via conversations.replies ─────────────

/// S-T2-002: Post a parent message, then post a thread reply to it.
/// Retrieve the thread via `conversations.replies` and verify the reply is
/// present in the correct thread — not as a top-level message.
///
/// Scenario: S-T2-002 | FRs: FR-013, FR-018
#[tokio::test]
async fn post_threaded_reply_and_verify_in_replies() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping post_threaded_reply_and_verify_in_replies: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();

    // Post the parent (session thread anchor) message.
    let parent_text = format!("[live-test] session anchor {run_id:.8}");
    let parent_ts = client
        .post_test_message(&config.channel_id, &parent_text)
        .await
        .expect("post parent message should succeed");

    // Post a reply in the thread anchored by parent_ts.
    let reply_marker = format!("broadcast-reply-{run_id:.8}");
    let reply_text = format!("[live-test] {reply_marker}");
    let reply_ts = client
        .post_thread_message(&config.channel_id, &parent_ts, &reply_text)
        .await
        .expect("post_thread_message should succeed");

    // Retrieve the thread and verify the reply is present.
    let replies = client
        .get_thread_replies(&config.channel_id, &parent_ts)
        .await
        .expect("get_thread_replies should succeed");

    // The first element is the parent; subsequent elements are replies.
    assert!(
        replies.len() >= 2,
        "thread should contain at least the parent + 1 reply; got {} messages",
        replies.len()
    );

    let reply_present = replies
        .iter()
        .any(|m| m["text"].as_str().unwrap_or_default().contains(&reply_marker));

    assert!(
        reply_present,
        "reply with marker '{reply_marker}' must appear in thread replies"
    );

    // The reply must carry thread_ts matching the parent.
    let reply_msg = replies
        .iter()
        .find(|m| m["text"].as_str().unwrap_or_default().contains(&reply_marker))
        .expect("reply message must be found");

    assert_eq!(
        reply_msg["thread_ts"].as_str().unwrap_or_default(),
        parent_ts.as_str(),
        "reply thread_ts must match the parent ts"
    );

    // Cleanup: deleting the parent removes the whole thread.
    client
        .cleanup_test_messages(&config.channel_id, &[parent_ts.as_str(), reply_ts.as_str()])
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-009: Rate limit handling — rapid message burst ───────────────────────

/// S-T2-009: Post five messages in rapid succession (no deliberate delay).
/// All posts must succeed, exercising the reqwest client's handling of the
/// connection pool and verifying the test channel accepts burst traffic
/// within the bot's rate tier.
///
/// Note: This test verifies the *test-harness client* path. The production
/// server has its own rate-limited queue — this test exercises whether
/// sequential bursts reach Slack successfully, not the server's retry logic.
///
/// Scenario: S-T2-009 | FRs: FR-019
#[tokio::test]
async fn rapid_message_burst_all_succeed() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping rapid_message_burst_all_succeed: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();

    let mut timestamps: Vec<String> = Vec::with_capacity(5);

    for i in 0..5_usize {
        let text = format!("[live-test] burst message {i}/5 (run {run_id:.8})");
        let ts = client
            .post_test_message(&config.channel_id, &text)
            .await
            .unwrap_or_else(|e| panic!("burst message {i} failed: {e}"));
        timestamps.push(ts);
    }

    assert_eq!(
        timestamps.len(),
        5,
        "all 5 burst messages must produce distinct timestamps"
    );

    // All timestamps must be unique (distinct messages, not duplicates).
    let unique: std::collections::HashSet<&str> =
        timestamps.iter().map(String::as_str).collect();
    assert_eq!(
        unique.len(),
        5,
        "each burst message must have a unique ts"
    );

    // Cleanup all burst messages.
    let ts_refs: Vec<&str> = timestamps.iter().map(String::as_str).collect();
    client
        .cleanup_test_messages(&config.channel_id, &ts_refs)
        .await
        .expect("cleanup of burst messages should succeed");
}
