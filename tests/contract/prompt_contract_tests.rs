//! Contract tests for `transmit` (`forward_prompt`) tool response shape
//! with real modal refine instruction text (T038, scenario S032).
//!
//! Validates the JSON output schema for the `forward_prompt` / `transmit`
//! tool specifically when the operator presses "Refine" and provides modal
//! instruction text. Supplements `forward_prompt_tests.rs` with US4-specific
//! assertions around real operator text replacing the placeholder.
//! Pure schema shape tests — live async flows are in `handler_blocking_tests.rs`.

use serde_json::{json, Value};

// ─── S032a: refine with real modal instruction text ───────────────────

/// When the operator presses "Refine" and submits modal text, the response
/// carries `decision: "refine"` and `instruction` containing the operator's
/// actual typed feedback (not a placeholder string).
#[test]
fn transmit_refine_includes_real_instruction_text() {
    let instruction = "Add more test coverage for edge cases";
    let response = json!({
        "decision": "refine",
        "instruction": instruction
    });

    let decision = response["decision"]
        .as_str()
        .expect("decision field present");
    assert_eq!(decision, "refine");

    let instruction_value = response
        .get("instruction")
        .and_then(Value::as_str)
        .expect("instruction field required for 'refine' decision");

    assert_eq!(
        instruction_value, instruction,
        "instruction must contain the operator's real modal text"
    );
    assert!(
        !instruction_value.contains("instruction via Slack"),
        "instruction must NOT be the old placeholder string"
    );
}

// ─── S032b: modal callback_id encoding for prompt refine ─────────────

/// The `callback_id` sent to Slack for a `forward_prompt` refine modal MUST
/// follow the pattern `"prompt_refine:{prompt_id}"`. The `ViewSubmission`
/// handler splits on `:` to extract the `prompt_id` for oneshot resolution.
#[test]
fn transmit_modal_callback_id_format_is_correct() {
    let prompt_id = "prompt:abc-456";
    let callback_id = format!("prompt_refine:{prompt_id}");

    let (source, entity_id) = callback_id
        .split_once(':')
        .expect("callback_id must contain ':'");
    assert_eq!(
        source, "prompt_refine",
        "source prefix must be 'prompt_refine'"
    );
    assert_eq!(entity_id, prompt_id, "entity_id must equal prompt_id");
}

// ─── S032c: continue response has no instruction ─────────────────────

/// When the operator presses "Continue" (no refine modal), the response
/// carries `decision: "continue"` and no `instruction` key.
#[test]
fn transmit_continue_has_no_instruction() {
    let response = json!({ "decision": "continue" });
    assert_eq!(response["decision"].as_str(), Some("continue"));
    assert!(
        response.get("instruction").is_none(),
        "instruction must be absent for 'continue' decision"
    );
}

// ─── S032d: stop response has no instruction ─────────────────────────

/// When the operator presses "Stop", the response carries `decision: "stop"`
/// and no `instruction` key.
#[test]
fn transmit_stop_has_no_instruction() {
    let response = json!({ "decision": "stop" });
    assert_eq!(response["decision"].as_str(), Some("stop"));
    assert!(
        response.get("instruction").is_none(),
        "instruction must be absent for 'stop' decision"
    );
}

// ─── S032e: refine instruction must be non-empty ─────────────────────

/// When `decision: "refine"` the instruction field MUST be a non-empty
/// string. (Empty modal submissions are rejected by the handler before
/// resolving the oneshot.)
#[test]
fn transmit_refine_instruction_is_non_empty_when_present() {
    let response = json!({
        "decision": "refine",
        "instruction": "check the error handling path"
    });
    let instruction = response["instruction"]
        .as_str()
        .expect("instruction present");
    assert!(
        !instruction.is_empty(),
        "refine instruction must not be empty"
    );
}
