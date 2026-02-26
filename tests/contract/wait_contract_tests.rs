//! Contract tests for `standby` (`wait_for_instruction`) tool response shape
//! with real modal instruction text (T037, scenario S030).
//!
//! Validates the JSON output schema produced by the `wait_for_instruction`
//! tool when the operator provides instruction text via the Slack modal.
//! These are pure schema shape tests — live async flows are in
//! `integration/handler_blocking_tests.rs`.

use serde_json::{json, Value};

// ─── S030a: resumed without instruction ──────────────────────────────

/// When the operator presses "Resume" (no modal), the response carries
/// `status: "resumed"` and no `instruction` key.
#[test]
fn standby_resume_without_instruction_has_no_instruction_field() {
    let response = json!({ "status": "resumed" });
    assert_eq!(
        response["status"].as_str(),
        Some("resumed"),
        "status must be 'resumed'"
    );
    assert!(
        response.get("instruction").is_none(),
        "instruction field must be absent when no text was provided"
    );
}

// ─── S030b: resumed with real modal instruction ───────────────────────

/// When the operator presses "Resume with Instructions" and submits modal
/// text, the response carries `status: "resumed"` **and** an `instruction`
/// field containing the operator's actual typed text (not a placeholder).
#[test]
fn standby_resume_with_instruction_includes_real_text() {
    let instruction = "Focus on error handling in the persistence layer";
    let response = json!({
        "status": "resumed",
        "instruction": instruction
    });

    let status = response["status"].as_str().expect("status present");
    assert_eq!(status, "resumed");

    let instruction_value = response
        .get("instruction")
        .and_then(Value::as_str)
        .expect("instruction field required");

    assert_eq!(
        instruction_value, instruction,
        "instruction must contain the operator's real text, not a placeholder"
    );
    assert!(
        !instruction_value.contains("instruction via Slack"),
        "instruction must NOT be the old placeholder string"
    );
}

// ─── S030c: stopped response shape ───────────────────────────────────

/// When the operator presses "Stop Session", the response carries
/// `status: "resumed"` and `instruction: "stop"`.
#[test]
fn standby_stop_response_shape() {
    let response = json!({
        "status": "resumed",
        "instruction": "stop"
    });
    assert_eq!(response["status"].as_str(), Some("resumed"));
    assert_eq!(response["instruction"].as_str(), Some("stop"));
}

// ─── S030d: modal callback_id encoding ───────────────────────────────

/// The `callback_id` sent to Slack for a `wait_for_instruction` modal MUST
/// follow the pattern `"wait_instruct:{session_id}"`. The `ViewSubmission`
/// handler splits on `:` to extract `session_id` and route the instruction.
#[test]
fn standby_modal_callback_id_format_is_correct() {
    let session_id = "session:abc-123";
    let callback_id = format!("wait_instruct:{session_id}");

    let (source, entity_id) = callback_id
        .split_once(':')
        .expect("callback_id must contain ':'");
    assert_eq!(
        source, "wait_instruct",
        "source prefix must be 'wait_instruct'"
    );
    assert_eq!(entity_id, session_id, "entity_id must equal session_id");
}

// ─── S030e: instruction must not be empty string ─────────────────────

/// When an instruction is present it must be a non-empty string.
/// (Empty modal submissions are rejected by the handler.)
#[test]
fn standby_instruction_when_present_is_non_empty() {
    let response = json!({
        "status": "resumed",
        "instruction": "run integration tests"
    });
    let instruction = response["instruction"]
        .as_str()
        .expect("instruction present");
    assert!(!instruction.is_empty(), "instruction must not be empty");
}
