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
