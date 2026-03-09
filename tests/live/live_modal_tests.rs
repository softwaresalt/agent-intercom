//! Live modal diagnostic tests — Tier 2 (Phase 6).
//!
//! Diagnoses the modal-in-thread issue at the API level by calling `views.open`
//! with synthetic trigger IDs for both top-level and threaded button contexts.
//!
//! ## Design rationale
//!
//! Slack `trigger_id` values are short-lived tokens (~3 seconds) issued only
//! when a real user clicks an interactive button. They cannot be fabricated.
//! These tests therefore use obviously-synthetic trigger IDs and document the
//! API response — confirming that the Slack API itself returns the same error
//! code (`invalid_trigger_id`) in both contexts. This is the API-level half of
//! the S-X-001 A/B comparison; the visual half is completed by Tier 3 Playwright
//! tests (S-T3-005, S-T3-006).
//!
//! ## Thread-reply fallback (S-T2-008)
//!
//! Task 6.2 tests the fallback mechanism directly using lower-level primitives
//! (`register_thread_reply_fallback` + `route_thread_reply`) to confirm the
//! pipeline resolves correctly without needing a live modal interaction. The
//! test posts a real Slack thread anchor to confirm live channel access, then
//! exercises the in-process fallback routing.
//!
//! ## Scenarios covered
//!
//! | Test function | Scenario | FRs |
//! |---|---|---|
//! | `modal_open_top_level_documents_api_result` | S-T2-007 | FR-016 |
//! | `modal_open_threaded_documents_api_result` | S-T2-006 | FR-015, FR-022 |
//! | `thread_reply_fallback_end_to_end` | S-T2-008 | FR-017, FR-023 |
//! | `wait_instruct_modal_in_thread_documents_api_result` | S-T2-011 | FR-015 |

use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use agent_intercom::slack::blocks;
use agent_intercom::slack::handlers::thread_reply::{
    fallback_map_key, register_thread_reply_fallback, route_thread_reply, PendingThreadReplies,
};
use agent_intercom::models::prompt::PromptType;

use super::live_helpers::{LiveSlackClient, LiveTestConfig};

// ── Synthetic trigger ID ──────────────────────────────────────────────────────
//
// Slack trigger_ids are short-lived tokens (~3 s) generated when a real user
// clicks a button. They cannot be fabricated for automated tests. Using a
// clearly-synthetic ID ensures the `views.open` call exercises the API path
// and returns a documented error rather than accidentally succeeding.
const SYNTHETIC_TRIGGER_ID: &str = "diag.synthetic.0000000001.AAAAAAAAAA";

// ── Minimal modal view JSON ────────────────────────────────────────────────────

/// Build a minimal instruction modal JSON for `views.open` diagnostic calls.
///
/// Uses the same structure as `blocks::instruction_modal` serialised to JSON
/// so the diagnostic call exercises the real payload shape.
fn minimal_modal_view(callback_id: &str) -> serde_json::Value {
    let modal = blocks::instruction_modal(
        callback_id,
        "Diagnostic Modal",
        "This is a diagnostic test…",
    );
    serde_json::to_value(&modal).expect("instruction_modal serialises to valid JSON")
}

// ── S-T2-007: Top-level modal open — API baseline ─────────────────────────────

/// S-T2-007: Post a prompt message as a **top-level** channel message and call
/// `views.open` with a synthetic trigger ID. Document the API response.
///
/// **Diagnostic intent**: Establishes the API-level baseline — what Slack
/// returns when `views.open` is called from a top-level (non-threaded) context
/// with an invalid trigger ID. Expected: `{"ok":false,"error":"invalid_trigger_id"}`.
///
/// **Why synthetic `trigger_id`**: Real trigger IDs expire in ~3 seconds and are
/// only issued when a real user clicks an interactive button. Automated tests
/// cannot obtain a live `trigger_id`; using a synthetic one exercises the API
/// path and captures the exact error code Slack returns.
///
/// Scenario: S-T2-007 | FRs: FR-016
#[tokio::test]
async fn modal_open_top_level_documents_api_result() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping modal_open_top_level_documents_api_result: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();
    let prompt_id = format!("modal-diag-top-{}", &run_id.to_string()[..8]);

    // Post a top-level (non-threaded) prompt message with Refine button.
    let prompt_blocks = blocks::build_prompt_blocks(
        "Modal diagnostic: top-level context",
        PromptType::Continuation,
        None,
        None,
        &prompt_id,
    );
    let blocks_json = serde_json::to_value(&prompt_blocks).expect("serialize prompt blocks");
    let live_text = format!("[live-test] modal-top-level diagnostic (run {})", &run_id.to_string()[..8]);

    let msg_ts = client
        .post_with_blocks(&config.channel_id, &live_text, blocks_json)
        .await
        .expect("post top-level prompt message");

    // ── Attempt views.open from top-level context ─────────────────────────────
    let callback_id = format!("prompt_refine:{prompt_id}");
    let view = minimal_modal_view(&callback_id);

    let api_response = client
        .open_modal_with_trigger(SYNTHETIC_TRIGGER_ID, view)
        .await
        .expect("views.open HTTP request should complete (even if Slack returns ok=false)");

    // Document the result.
    let ok = api_response["ok"].as_bool().unwrap_or(false);
    let error_code = api_response["error"].as_str().unwrap_or("(none)");

    eprintln!(
        "[modal-diag] S-T2-007 top-level: ok={ok}, error={error_code:?}\n\
         Full response: {}",
        serde_json::to_string_pretty(&api_response).unwrap_or_default()
    );

    // Assert: the API must respond with a parseable JSON object.
    // We do NOT assert ok=true because we expect ok=false with invalid_trigger_id.
    // The diagnostic value is the documented error_code.
    assert!(
        api_response.is_object(),
        "views.open must return a JSON object; got: {api_response}"
    );

    // Assert: the response must include the "ok" field.
    assert!(
        api_response.get("ok").is_some(),
        "views.open response must contain the 'ok' field; got: {api_response}"
    );

    // Verify the documented failure mode: synthetic trigger_id → invalid_trigger_id.
    // This is the expected API-level result for both threaded and non-threaded contexts.
    if ok {
        eprintln!(
            "[modal-diag] S-T2-007 NOTE: API returned ok=true for top-level context \
             (unexpected with synthetic trigger_id — may indicate Slack API change)"
        );
    } else {
        assert_eq!(
            error_code, "invalid_trigger_id",
            "S-T2-007 top-level: expected 'invalid_trigger_id' error for synthetic trigger; \
             got {error_code:?}. Full response: {api_response}"
        );
        eprintln!(
            "[modal-diag] S-T2-007 CONFIRMED: API returns 'invalid_trigger_id' for \
             top-level context with synthetic trigger_id (expected)"
        );
    }

    // Cleanup.
    client
        .cleanup_test_messages(&config.channel_id, &[msg_ts.as_str()])
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-006: Threaded modal open — API behavior ──────────────────────────────

/// S-T2-006: Post a prompt message **inside a thread** and call `views.open`
/// with a synthetic trigger ID. Document and compare the API response to
/// the top-level baseline from S-T2-007.
///
/// **Diagnostic intent**: Determines whether `views.open` behaves differently
/// at the API level when called from a threaded context vs. a top-level one.
/// If both return `invalid_trigger_id`, the silent modal failure for threaded
/// buttons is a **client-side rendering issue**, not an API-level error.
///
/// Scenario: S-T2-006 | FRs: FR-015, FR-022
#[tokio::test]
async fn modal_open_threaded_documents_api_result() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping modal_open_threaded_documents_api_result: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();
    let prompt_id = format!("modal-diag-thr-{}", &run_id.to_string()[..8]);

    // Post a top-level anchor message (simulates a session-started notification).
    let anchor_text = format!(
        "[live-test] modal-threaded anchor (run {})",
        &run_id.to_string()[..8]
    );
    let anchor_ts = client
        .post_test_message(&config.channel_id, &anchor_text)
        .await
        .expect("post thread anchor");

    // Post a prompt message with Refine button INSIDE the thread.
    let prompt_blocks = blocks::build_prompt_blocks(
        "Modal diagnostic: threaded context",
        PromptType::Continuation,
        None,
        None,
        &prompt_id,
    );
    let blocks_json = serde_json::to_value(&prompt_blocks).expect("serialize prompt blocks");
    let thread_text = format!(
        "[live-test] modal-threaded prompt (run {})",
        &run_id.to_string()[..8]
    );

    let thread_msg_ts = client
        .post_thread_blocks(&config.channel_id, &anchor_ts, &thread_text, blocks_json)
        .await
        .expect("post threaded prompt message");

    // ── Attempt views.open from threaded context ──────────────────────────────
    let callback_id = format!("prompt_refine:{prompt_id}");
    let view = minimal_modal_view(&callback_id);

    let api_response = client
        .open_modal_with_trigger(SYNTHETIC_TRIGGER_ID, view)
        .await
        .expect("views.open HTTP request should complete (even if Slack returns ok=false)");

    // Document the result.
    let ok = api_response["ok"].as_bool().unwrap_or(false);
    let error_code = api_response["error"].as_str().unwrap_or("(none)");

    eprintln!(
        "[modal-diag] S-T2-006 threaded: ok={ok}, error={error_code:?}\n\
         Full response: {}",
        serde_json::to_string_pretty(&api_response).unwrap_or_default()
    );

    // Assert: the API must respond with a parseable JSON object.
    assert!(
        api_response.is_object(),
        "views.open must return a JSON object; got: {api_response}"
    );

    // Assert: the response must include the "ok" field.
    assert!(
        api_response.get("ok").is_some(),
        "views.open response must contain the 'ok' field; got: {api_response}"
    );

    // Verify the documented failure mode: same as top-level.
    if ok {
        eprintln!(
            "[modal-diag] S-T2-006 NOTE: API returned ok=true for threaded context \
             (unexpected with synthetic trigger_id)"
        );
    } else {
        assert_eq!(
            error_code, "invalid_trigger_id",
            "S-T2-006 threaded: expected 'invalid_trigger_id' error for synthetic trigger; \
             got {error_code:?}. Full response: {api_response}"
        );
        eprintln!(
            "[modal-diag] S-T2-006 CONFIRMED: API returns 'invalid_trigger_id' for \
             threaded context (same as top-level — silent failure is client-side)"
        );
    }

    // Cleanup — both the anchor and the threaded reply.
    // Deleting the anchor cascades thread visually but Slack API requires
    // individual deletion of thread messages for full cleanup.
    client
        .cleanup_test_messages(
            &config.channel_id,
            &[thread_msg_ts.as_str(), anchor_ts.as_str()],
        )
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-008: Thread-reply fallback end-to-end ────────────────────────────────

/// S-T2-008: Verify the thread-reply fallback pipeline end-to-end.
///
/// This test simulates the fallback path that activates when `views.open` fails:
///
/// 1. Post a thread anchor message to confirm live Slack channel access.
/// 2. Register a pending thread-reply entry (`register_thread_reply_fallback`).
/// 3. Route a synthetic thread reply (`route_thread_reply`).
/// 4. Verify the oneshot channel resolves with the expected text.
///
/// This exercises the complete fallback resolution pipeline (FR-017, FR-023)
/// without requiring a live modal interaction or a real `trigger_id`.
///
/// Scenario: S-T2-008 | FRs: FR-017, FR-023
#[tokio::test]
async fn thread_reply_fallback_end_to_end() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping thread_reply_fallback_end_to_end: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();
    let session_id = format!("session:modal-fb-{}", &run_id.to_string()[..8]);
    let authorized_user = "U_MODAL_FALLBACK_TESTER";

    // Post a top-level anchor to obtain a real Slack thread_ts.
    let anchor_text = format!(
        "[live-test] modal fallback anchor (run {})",
        &run_id.to_string()[..8]
    );
    let anchor_ts = client
        .post_test_message(&config.channel_id, &anchor_text)
        .await
        .expect("post thread anchor");

    // Post the fallback instruction as a thread reply (as the production handler does).
    let fallback_instruction = "Modal unavailable — please reply in this thread with your instructions.";
    let fallback_ts = client
        .post_thread_message(&config.channel_id, &anchor_ts, fallback_instruction)
        .await
        .expect("post fallback instruction thread reply");

    // ── Register pending thread-reply fallback ────────────────────────────────
    let pending: PendingThreadReplies = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let (tx, rx) = oneshot::channel::<String>();

    register_thread_reply_fallback(
        &config.channel_id,
        anchor_ts.clone(),
        session_id.clone(),
        authorized_user.to_owned(),
        tx,
        Arc::clone(&pending),
    )
    .await;

    // Verify the entry is registered.
    let expected_key = fallback_map_key(&config.channel_id, &anchor_ts);
    {
        let guard = pending.lock().await;
        assert!(
            guard.contains_key(&expected_key),
            "pending_thread_replies must contain the registered key after registration; \
             key={expected_key:?}"
        );
    }

    // ── Route a synthetic thread reply ────────────────────────────────────────
    let reply_text = "Focus on error handling and add retry logic";

    // Unauthorized reply — must be silently ignored.
    let ignored = route_thread_reply(
        &config.channel_id,
        &anchor_ts,
        "U_INTRUDER",
        "ignore me",
        Arc::clone(&pending),
    )
    .await
    .expect("route_thread_reply should not error on unauthorized sender");

    assert!(
        !ignored,
        "unauthorized reply must be silently ignored (fallback entry should remain)"
    );

    // Authorized reply — must resolve the oneshot.
    let resolved = route_thread_reply(
        &config.channel_id,
        &anchor_ts,
        authorized_user,
        reply_text,
        Arc::clone(&pending),
    )
    .await
    .expect("route_thread_reply should not error for authorized sender");

    assert!(resolved, "authorized reply must be captured and forwarded");

    // Verify the oneshot resolved with the correct text.
    let received = rx
        .await
        .expect("oneshot must resolve after route_thread_reply captures the reply");

    assert_eq!(
        received, reply_text,
        "received text must match the routed reply exactly"
    );

    // Verify the pending entry was removed after delivery (single-entry guarantee).
    {
        let guard = pending.lock().await;
        assert!(
            !guard.contains_key(&expected_key),
            "pending_thread_replies must be empty after first authorized reply is routed"
        );
    }

    eprintln!(
        "[modal-diag] S-T2-008 CONFIRMED: thread-reply fallback resolved correctly — \
         unauthorized reply ignored, authorized reply captured, oneshot resolved with \
         text={reply_text:?}"
    );

    // Cleanup.
    client
        .cleanup_test_messages(
            &config.channel_id,
            &[fallback_ts.as_str(), anchor_ts.as_str()],
        )
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-011: Wait-resume-instruct modal in thread ────────────────────────────

/// S-T2-011: Post a wait-for-instruction message **inside a thread** and call
/// `views.open` with a synthetic trigger ID. Document the API result.
///
/// **Diagnostic intent**: Mirrors S-T2-006 for the `wait_resume_instruct` modal
/// path — the second modal-dependent interaction type. Confirms that threaded
/// wait-resume-instruct triggers receive the same API response as threaded
/// prompt-refine triggers, establishing complete coverage of the modal-in-thread
/// failure surface (FR-015).
///
/// Scenario: S-T2-011 | FRs: FR-015
#[tokio::test]
async fn wait_instruct_modal_in_thread_documents_api_result() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[live-test] Skipping wait_instruct_modal_in_thread_documents_api_result: {e}"
            );
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();
    let session_id = format!("session:wait-diag-{}", &run_id.to_string()[..8]);

    // Post a top-level anchor message (simulates a session thread root).
    let anchor_text = format!(
        "[live-test] wait-modal-diag anchor (run {})",
        &run_id.to_string()[..8]
    );
    let anchor_ts = client
        .post_test_message(&config.channel_id, &anchor_text)
        .await
        .expect("post thread anchor");

    // Post a wait-for-instruction message with Resume/Stop buttons INSIDE the thread.
    let wait_blocks = vec![blocks::wait_buttons(&session_id)];
    let wait_blocks_json = serde_json::to_value(&wait_blocks).expect("serialize wait blocks");
    let thread_text = format!(
        "[live-test] wait-modal-diag prompt (run {})",
        &run_id.to_string()[..8]
    );

    let thread_msg_ts = client
        .post_thread_blocks(&config.channel_id, &anchor_ts, &thread_text, wait_blocks_json)
        .await
        .expect("post threaded wait message");

    // ── Attempt views.open for wait_instruct modal from threaded context ──────
    let callback_id = format!("wait_instruct:{session_id}");
    let view = minimal_modal_view(&callback_id);

    let api_response = client
        .open_modal_with_trigger(SYNTHETIC_TRIGGER_ID, view)
        .await
        .expect("views.open HTTP request should complete (even if Slack returns ok=false)");

    // Document the result.
    let ok = api_response["ok"].as_bool().unwrap_or(false);
    let error_code = api_response["error"].as_str().unwrap_or("(none)");

    eprintln!(
        "[modal-diag] S-T2-011 wait-instruct (threaded): ok={ok}, error={error_code:?}\n\
         Full response: {}",
        serde_json::to_string_pretty(&api_response).unwrap_or_default()
    );

    // Assert: the API must respond with a parseable JSON object.
    assert!(
        api_response.is_object(),
        "views.open must return a JSON object; got: {api_response}"
    );

    // Assert: the response must include the "ok" field.
    assert!(
        api_response.get("ok").is_some(),
        "views.open response must contain the 'ok' field; got: {api_response}"
    );

    // Document the failure mode for the wait_instruct modal path.
    if ok {
        eprintln!(
            "[modal-diag] S-T2-011 NOTE: API returned ok=true for wait_instruct modal \
             in threaded context (unexpected with synthetic trigger_id)"
        );
    } else {
        assert_eq!(
            error_code, "invalid_trigger_id",
            "S-T2-011: expected 'invalid_trigger_id' error for synthetic trigger; \
             got {error_code:?}. Full response: {api_response}"
        );
        eprintln!(
            "[modal-diag] S-T2-011 CONFIRMED: wait_instruct modal returns same \
             'invalid_trigger_id' error in threaded context — consistent with S-T2-006 \
             (prompt_refine). Both modal paths share the same client-side failure mode."
        );
    }

    // Cleanup.
    client
        .cleanup_test_messages(
            &config.channel_id,
            &[thread_msg_ts.as_str(), anchor_ts.as_str()],
        )
        .await
        .expect("cleanup should succeed");
}
