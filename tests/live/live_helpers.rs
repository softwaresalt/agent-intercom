//! Live Slack test helpers — Tier 2 test infrastructure.
//!
//! Provides [`LiveTestConfig`], [`LiveSlackClient`], and assertion utilities
//! for tests that communicate with a real Slack workspace via the Web API.
//!
//! **Loading credentials**
//!
//! Call [`LiveTestConfig::from_env`] at the top of each test. When the required
//! environment variables are absent the function returns `Err`, and callers
//! should print a skip notice and return early rather than panic.
//!
//! **Cleanup discipline**
//!
//! Always call [`LiveSlackClient::cleanup_test_messages`] in every code path
//! (success and failure) so that test messages do not accumulate in the
//! channel. Prefer wrapping cleanup in a deferred pattern or asserting after
//! cleanup rather than before.

use std::env;

use reqwest::Client;
use serde_json::{json, Value};

/// Base URL for all Slack Web API calls.
const SLACK_API_BASE: &str = "https://slack.com/api";

// ── LiveTestConfig ────────────────────────────────────────────────────────────

/// Configuration for live Slack tests, sourced from environment variables.
///
/// # Examples
///
/// ```ignore
/// let config = match LiveTestConfig::from_env() {
///     Ok(c)  => c,
///     Err(e) => { eprintln!("Skipping live test: {e}"); return; }
/// };
/// ```
pub struct LiveTestConfig {
    /// Slack bot token (`xoxb-…`), authorised to post in the test channel.
    pub bot_token: String,
    /// Slack channel ID (`C…`) for posting and verifying test messages.
    pub channel_id: String,
}

impl LiveTestConfig {
    /// Load configuration from environment variables.
    ///
    /// Reads `SLACK_TEST_BOT_TOKEN` and `SLACK_TEST_CHANNEL_ID`.
    ///
    /// # Errors
    ///
    /// Returns an error string describing the first missing variable when
    /// either required environment variable is absent.
    pub fn from_env() -> Result<Self, String> {
        let bot_token = env::var("SLACK_TEST_BOT_TOKEN")
            .map_err(|_| "SLACK_TEST_BOT_TOKEN not set".to_owned())?;
        let channel_id = env::var("SLACK_TEST_CHANNEL_ID")
            .map_err(|_| "SLACK_TEST_CHANNEL_ID not set".to_owned())?;
        Ok(Self {
            bot_token,
            channel_id,
        })
    }
}

// ── LiveSlackClient ───────────────────────────────────────────────────────────

/// Lightweight Slack Web API client for live test operations.
///
/// Wraps [`reqwest::Client`] with bot-token authorisation and convenience
/// methods that map directly to Slack API endpoints used by the test suite.
pub struct LiveSlackClient {
    http: Client,
    bot_token: String,
}

impl LiveSlackClient {
    /// Construct a new client authenticated with `bot_token`.
    #[must_use]
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            bot_token: bot_token.into(),
        }
    }

    /// Build the `Authorization: Bearer …` header value.
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.bot_token)
    }

    /// Post a plain-text message to `channel_id`.
    ///
    /// Returns the Slack message timestamp (`ts`) assigned by the API, which
    /// serves as the message's unique identifier for subsequent retrieval and
    /// deletion.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails or Slack responds
    /// with `"ok": false`.
    pub async fn post_test_message(&self, channel_id: &str, text: &str) -> Result<String, String> {
        let body = json!({ "channel": channel_id, "text": text });

        let resp: Value = self
            .http
            .post(format!("{SLACK_API_BASE}/chat.postMessage"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("chat.postMessage request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("chat.postMessage parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "chat.postMessage error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        resp["ts"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "chat.postMessage: missing ts in response".to_owned())
    }

    /// Retrieve the message with timestamp `ts` from `channel_id`'s history.
    ///
    /// Uses `conversations.history` with `oldest=ts`, `latest=ts`,
    /// `inclusive=true`, and `limit=1` to fetch exactly that message.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails, Slack responds
    /// with `"ok": false`, or no message with the given timestamp is found.
    pub async fn get_message(&self, channel_id: &str, ts: &str) -> Result<Value, String> {
        // Slack channel IDs (C…) and timestamps (digits + dot) are ASCII-safe;
        // no percent-encoding is needed for direct URL embedding.
        let url = format!(
            "{SLACK_API_BASE}/conversations.history\
             ?channel={channel_id}&oldest={ts}&latest={ts}&limit=1&inclusive=true"
        );

        let resp: Value = self
            .http
            .get(url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("conversations.history request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("conversations.history parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "conversations.history error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        resp["messages"]
            .as_array()
            .and_then(|msgs: &Vec<Value>| msgs.first().cloned())
            .ok_or_else(|| format!("no message found with ts={ts}"))
    }

    /// Retrieve all messages in the thread anchored by `thread_ts`.
    ///
    /// Uses `conversations.replies`. The first element in the returned `Vec`
    /// is the parent message itself; subsequent elements are replies.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails or Slack responds
    /// with `"ok": false`.
    pub async fn get_thread_replies(
        &self,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Vec<Value>, String> {
        // Slack channel IDs and timestamps are ASCII-safe for direct URL embedding.
        let url =
            format!("{SLACK_API_BASE}/conversations.replies?channel={channel_id}&ts={thread_ts}");

        let resp: Value = self
            .http
            .get(url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("conversations.replies request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("conversations.replies parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "conversations.replies error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        Ok(resp["messages"].as_array().cloned().unwrap_or_default())
    }

    /// Delete messages identified by the given timestamps from `channel_id`.
    ///
    /// Deletes each message individually via `chat.delete`. Every deletion is
    /// attempted regardless of prior failures — the method is best-effort.
    /// If one or more deletions fail, an aggregated error string listing all
    /// failures is returned after all deletions have been attempted.
    ///
    /// # Errors
    ///
    /// Returns an aggregated error string when any HTTP request fails or Slack
    /// responds with `"ok": false` for any deletion. Failures for individual
    /// timestamps do not prevent the remaining timestamps from being deleted.
    pub async fn cleanup_test_messages(
        &self,
        channel_id: &str,
        timestamps: &[&str],
    ) -> Result<(), String> {
        let mut errors: Vec<String> = Vec::new();

        for ts in timestamps {
            let body = json!({ "channel": channel_id, "ts": ts });

            let result: Result<Value, String> = async {
                self.http
                    .post(format!("{SLACK_API_BASE}/chat.delete"))
                    .header("Authorization", self.auth_header())
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("chat.delete request failed for ts={ts}: {e}"))?
                    .json::<Value>()
                    .await
                    .map_err(|e| format!("chat.delete parse failed for ts={ts}: {e}"))
            }
            .await;

            match result {
                Ok(resp) => {
                    if resp["ok"].as_bool() != Some(true) {
                        errors.push(format!(
                            "chat.delete error for ts={ts}: {}",
                            resp["error"].as_str().unwrap_or("unknown")
                        ));
                    }
                }
                Err(e) => errors.push(e),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    /// Post a message with Block Kit blocks to `channel_id`.
    ///
    /// Returns the Slack timestamp (`ts`) for the posted message.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails or Slack responds
    /// with `"ok": false`.
    pub async fn post_with_blocks(
        &self,
        channel_id: &str,
        text: &str,
        blocks: serde_json::Value,
    ) -> Result<String, String> {
        let body = json!({ "channel": channel_id, "text": text, "blocks": blocks });

        let resp: Value = self
            .http
            .post(format!("{SLACK_API_BASE}/chat.postMessage"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("chat.postMessage (blocks) request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("chat.postMessage (blocks) parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "chat.postMessage (blocks) error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        resp["ts"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "chat.postMessage (blocks): missing ts in response".to_owned())
    }

    /// Post a plain-text reply in the thread anchored by `thread_ts`.
    ///
    /// Returns the Slack timestamp of the new reply.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails or Slack responds
    /// with `"ok": false`.
    pub async fn post_thread_message(
        &self,
        channel_id: &str,
        thread_ts: &str,
        text: &str,
    ) -> Result<String, String> {
        let body = json!({
            "channel": channel_id,
            "thread_ts": thread_ts,
            "text": text,
        });

        let resp: Value = self
            .http
            .post(format!("{SLACK_API_BASE}/chat.postMessage"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("chat.postMessage (thread) request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("chat.postMessage (thread) parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "chat.postMessage (thread) error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        resp["ts"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "chat.postMessage (thread): missing ts in response".to_owned())
    }

    /// Post Block Kit blocks as a reply in the thread anchored by `thread_ts`.
    ///
    /// Returns the Slack timestamp of the new reply.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails or Slack responds
    /// with `"ok": false`.
    pub async fn post_thread_blocks(
        &self,
        channel_id: &str,
        thread_ts: &str,
        text: &str,
        blocks: serde_json::Value,
    ) -> Result<String, String> {
        let body = json!({
            "channel": channel_id,
            "thread_ts": thread_ts,
            "text": text,
            "blocks": blocks,
        });

        let resp: Value = self
            .http
            .post(format!("{SLACK_API_BASE}/chat.postMessage"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("chat.postMessage (thread blocks) request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("chat.postMessage (thread blocks) parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "chat.postMessage (thread blocks) error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        resp["ts"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "chat.postMessage (thread blocks): missing ts in response".to_owned())
    }

    /// Call `views.open` with the given `trigger_id` and `view` JSON.
    ///
    /// Used by modal-diagnostic tests to document the API-level response when
    /// `views.open` is invoked for a top-level vs. threaded button context.
    /// In live diagnostic runs the `trigger_id` will be synthetic (invalid),
    /// so callers should inspect the returned `Value` rather than panicking on
    /// `"ok": false`.
    ///
    /// Returns the raw Slack API response JSON so the caller can log and assert
    /// on the `ok` flag and `error` field.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request itself fails (network
    /// error, TLS error). A Slack-side `"ok": false` is returned as `Ok(Value)`
    /// so the caller can inspect and document the specific error code.
    pub async fn open_modal_with_trigger(
        &self,
        trigger_id: &str,
        view: serde_json::Value,
    ) -> Result<Value, String> {
        let body = json!({ "trigger_id": trigger_id, "view": view });

        self.http
            .post(format!("{SLACK_API_BASE}/views.open"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("views.open request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("views.open response parse failed: {e}"))
    }

    /// Update an existing message's content.
    ///
    /// Replaces the message at `ts` in `channel_id` with the given `text` and
    /// optional `blocks`. When `blocks` is `Some`, the Block Kit payload
    /// replaces the original blocks; when `None`, only `text` is updated.
    ///
    /// # Errors
    ///
    /// Returns an error string when the HTTP request fails or Slack responds
    /// with `"ok": false`.
    pub async fn update_message(
        &self,
        channel_id: &str,
        ts: &str,
        text: &str,
        blocks: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let mut body = json!({ "channel": channel_id, "ts": ts, "text": text });

        if let Some(blks) = blocks {
            body["blocks"] = blks;
        }

        let resp: Value = self
            .http
            .post(format!("{SLACK_API_BASE}/chat.update"))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("chat.update request failed: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("chat.update parse failed: {e}"))?;

        if resp["ok"].as_bool() != Some(true) {
            return Err(format!(
                "chat.update error: {}",
                resp["error"].as_str().unwrap_or("unknown")
            ));
        }

        Ok(())
    }
}

// ── Assertion helpers ─────────────────────────────────────────────────────────

/// Assert that the `blocks` array of a Slack API message contains `expected`.
///
/// Serialises the entire `blocks` value to JSON and checks for the substring.
/// This covers nested structures without requiring deep traversal.
///
/// # Panics
///
/// Panics when no block in `message["blocks"]` contains `expected` as a
/// JSON substring.
pub fn assert_blocks_contain(message: &Value, expected: &str) {
    let blocks_json =
        serde_json::to_string(&message["blocks"]).unwrap_or_else(|_| String::from("null"));
    assert!(
        blocks_json.contains(expected),
        "expected blocks to contain {expected:?}\nblocks JSON: {blocks_json}"
    );
}
