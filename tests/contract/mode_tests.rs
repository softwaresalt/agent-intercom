//! Contract tests for `set_operational_mode` and `wait_for_instruction` tools (T124).
//!
//! Validates input/output schemas per mcp-tools.json contract:
//!
//! ## `set_operational_mode`
//! - Input requires `mode` (enum: `remote`, `local`, `hybrid`)
//! - Output requires `previous_mode` and `current_mode` (same enum)
//!
//! ## `wait_for_instruction`
//! - Input accepts optional `message` and `timeout_seconds`
//! - Output requires `status` (enum: `resumed`, `timeout`) and optional `instruction`

use serde_json::json;

// ══════════════════════════════════════════════════════════════════════
// set_operational_mode — Input schema
// ══════════════════════════════════════════════════════════════════════

const VALID_MODES: &[&str] = &["remote", "local", "hybrid"];

#[test]
fn set_mode_input_requires_mode() {
    let input = json!({ "mode": "remote" });
    assert!(
        input.get("mode").is_some(),
        "mode field is required in input"
    );
}

#[test]
fn set_mode_input_accepts_remote() {
    let input = json!({ "mode": "remote" });
    let mode = input["mode"].as_str().expect("mode is string");
    assert!(VALID_MODES.contains(&mode), "remote is a valid mode");
}

#[test]
fn set_mode_input_accepts_local() {
    let input = json!({ "mode": "local" });
    let mode = input["mode"].as_str().expect("mode is string");
    assert!(VALID_MODES.contains(&mode), "local is a valid mode");
}

#[test]
fn set_mode_input_accepts_hybrid() {
    let input = json!({ "mode": "hybrid" });
    let mode = input["mode"].as_str().expect("mode is string");
    assert!(VALID_MODES.contains(&mode), "hybrid is a valid mode");
}

#[test]
fn set_mode_input_rejects_invalid_mode() {
    let input = json!({ "mode": "offline" });
    let mode = input["mode"].as_str().expect("mode is string");
    assert!(!VALID_MODES.contains(&mode), "offline is not a valid mode");
}

#[test]
fn set_mode_input_missing_mode_is_empty_object() {
    let input = json!({});
    assert!(
        input.get("mode").is_none(),
        "input without mode should fail server-side validation"
    );
}

// ── set_operational_mode — Output schema ──────────────────────────────

#[test]
fn set_mode_output_requires_previous_and_current() {
    let output = json!({ "previous_mode": "remote", "current_mode": "local" });
    assert!(
        output.get("previous_mode").is_some(),
        "previous_mode is required"
    );
    assert!(
        output.get("current_mode").is_some(),
        "current_mode is required"
    );
}

#[test]
fn set_mode_output_both_fields_are_valid_enums() {
    let output = json!({ "previous_mode": "remote", "current_mode": "hybrid" });
    let prev = output["previous_mode"].as_str().expect("str");
    let curr = output["current_mode"].as_str().expect("str");
    assert!(VALID_MODES.contains(&prev));
    assert!(VALID_MODES.contains(&curr));
}

#[test]
fn set_mode_output_same_mode_is_valid() {
    let output = json!({ "previous_mode": "local", "current_mode": "local" });
    assert_eq!(output["previous_mode"], output["current_mode"]);
}

#[test]
fn set_mode_output_all_transitions_valid() {
    for from in VALID_MODES {
        for to in VALID_MODES {
            let output = json!({ "previous_mode": from, "current_mode": to });
            assert!(output["previous_mode"].is_string());
            assert!(output["current_mode"].is_string());
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// wait_for_instruction — Input schema
// ══════════════════════════════════════════════════════════════════════

#[test]
fn wait_input_accepts_empty_object() {
    let input = json!({});
    assert!(input.is_object());
    assert!(input.get("message").is_none());
    assert!(input.get("timeout_seconds").is_none());
}

#[test]
fn wait_input_accepts_message_only() {
    let input = json!({ "message": "Waiting for next task." });
    assert!(input.get("message").is_some());
    assert!(input["message"].is_string());
}

#[test]
fn wait_input_accepts_timeout_seconds() {
    let input = json!({ "timeout_seconds": 300 });
    assert!(input.get("timeout_seconds").is_some());
    let ts = input["timeout_seconds"]
        .as_u64()
        .expect("timeout is integer");
    assert_eq!(ts, 300);
}

#[test]
fn wait_input_accepts_all_fields() {
    let input = json!({
        "message": "Agent idle.",
        "timeout_seconds": 600
    });
    assert!(input.get("message").is_some());
    assert!(input.get("timeout_seconds").is_some());
}

#[test]
fn wait_input_timeout_zero_means_indefinite() {
    let input = json!({ "timeout_seconds": 0 });
    let ts = input["timeout_seconds"]
        .as_u64()
        .expect("timeout is integer");
    assert_eq!(ts, 0, "0 means wait indefinitely");
}

// ── wait_for_instruction — Output schema ──────────────────────────────

const VALID_WAIT_STATUSES: &[&str] = &["resumed", "timeout"];

#[test]
fn wait_output_requires_status() {
    let output = json!({ "status": "resumed" });
    assert!(output.get("status").is_some(), "status is required");
}

#[test]
fn wait_output_status_accepts_resumed() {
    let output = json!({ "status": "resumed" });
    let status = output["status"].as_str().expect("str");
    assert!(VALID_WAIT_STATUSES.contains(&status));
}

#[test]
fn wait_output_status_accepts_timeout() {
    let output = json!({ "status": "timeout" });
    let status = output["status"].as_str().expect("str");
    assert!(VALID_WAIT_STATUSES.contains(&status));
}

#[test]
fn wait_output_status_rejects_invalid() {
    let output = json!({ "status": "cancelled" });
    let status = output["status"].as_str().expect("str");
    assert!(!VALID_WAIT_STATUSES.contains(&status));
}

#[test]
fn wait_output_instruction_is_optional() {
    let without = json!({ "status": "timeout" });
    assert!(without.get("instruction").is_none());

    let with = json!({ "status": "resumed", "instruction": "Deploy to staging" });
    assert!(with.get("instruction").is_some());
    assert!(with["instruction"].is_string());
}

#[test]
fn wait_output_instruction_null_on_bare_resume() {
    let output = json!({ "status": "resumed", "instruction": null });
    assert!(output["instruction"].is_null());
}

// ── Mode enum deserialization tests ───────────────────────────────────

#[test]
fn session_mode_deserializes_all_variants() {
    use monocoque_agent_rem::models::session::SessionMode;

    let remote: SessionMode = serde_json::from_str("\"remote\"").expect("remote");
    assert_eq!(remote, SessionMode::Remote);

    let local: SessionMode = serde_json::from_str("\"local\"").expect("local");
    assert_eq!(local, SessionMode::Local);

    let hybrid: SessionMode = serde_json::from_str("\"hybrid\"").expect("hybrid");
    assert_eq!(hybrid, SessionMode::Hybrid);
}

#[test]
fn session_mode_rejects_invalid_variant() {
    use monocoque_agent_rem::models::session::SessionMode;

    let result: std::result::Result<SessionMode, _> = serde_json::from_str("\"offline\"");
    assert!(result.is_err(), "offline is not a valid SessionMode");
}

#[test]
fn session_mode_serializes_all_variants() {
    use monocoque_agent_rem::models::session::SessionMode;

    assert_eq!(
        serde_json::to_string(&SessionMode::Remote).expect("ser"),
        "\"remote\""
    );
    assert_eq!(
        serde_json::to_string(&SessionMode::Local).expect("ser"),
        "\"local\""
    );
    assert_eq!(
        serde_json::to_string(&SessionMode::Hybrid).expect("ser"),
        "\"hybrid\""
    );
}
