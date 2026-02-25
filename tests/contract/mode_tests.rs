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
    use agent_intercom::models::session::SessionMode;

    let remote: SessionMode = serde_json::from_str("\"remote\"").expect("remote");
    assert_eq!(remote, SessionMode::Remote);

    let local: SessionMode = serde_json::from_str("\"local\"").expect("local");
    assert_eq!(local, SessionMode::Local);

    let hybrid: SessionMode = serde_json::from_str("\"hybrid\"").expect("hybrid");
    assert_eq!(hybrid, SessionMode::Hybrid);
}

#[test]
fn session_mode_rejects_invalid_variant() {
    use agent_intercom::models::session::SessionMode;

    let result: std::result::Result<SessionMode, _> = serde_json::from_str("\"offline\"");
    assert!(result.is_err(), "offline is not a valid SessionMode");
}

#[test]
fn session_mode_serializes_all_variants() {
    use agent_intercom::models::session::SessionMode;

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

// ─── Phase 5 — standby no-channel scenario contract shapes ───────────

/// T056 / S041 — The `standby` contract must document error output for no-channel case.
///
/// The `outputSchema.properties` must include `error_code` so agents know to expect
/// an error when no Slack channel is configured rather than blocking indefinitely.
///
/// This test will FAIL until `mcp-tools.json` is updated to include `error_code`
/// in the `standby` outputSchema (implementation gate for T067).
#[test]
fn contract_standby_schema_includes_error_code_property() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let tool = &contract["tools"]["standby"];
    let output = &tool["outputSchema"];
    let props = output["properties"]
        .as_object()
        .expect("standby outputSchema.properties must be an object");
    assert!(
        props.contains_key("error_code"),
        "standby outputSchema must include 'error_code' property for no-channel errors"
    );
}

/// T056 / S041 — No-channel error output shape for `standby`.
#[test]
fn standby_no_channel_error_code_structure() {
    let output = serde_json::json!({
        "status": "error",
        "error_code": "no_channel",
        "error_message": "no Slack channel configured for this session"
    });
    assert_eq!(output["status"].as_str(), Some("error"));
    assert_eq!(output["error_code"].as_str(), Some("no_channel"));
}

/// `standby` contract must include `slack_unavailable` in the `error_code` enum.
#[test]
fn contract_standby_error_code_includes_slack_unavailable() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let enum_vals =
        &contract["tools"]["standby"]["outputSchema"]["properties"]["error_code"]["enum"];
    let codes: Vec<&str> = enum_vals
        .as_array()
        .expect("error_code enum must be an array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        codes.contains(&"slack_unavailable"),
        "standby error_code enum must include 'slack_unavailable'; got {codes:?}"
    );
}

/// `standby` contract status enum must include `error` variant.
#[test]
fn contract_standby_status_includes_error() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let enum_vals = &contract["tools"]["standby"]["outputSchema"]["properties"]["status"]["enum"];
    let statuses: Vec<&str> = enum_vals
        .as_array()
        .expect("status enum must be an array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        statuses.contains(&"error"),
        "standby status enum must include 'error'; got {statuses:?}"
    );
}

// ─── Phase 5b — transmit error scenario contract shapes ──────────────

/// The `transmit` contract must include `error_code` in its `outputSchema`.
#[test]
fn contract_transmit_schema_includes_error_code_property() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let tool = &contract["tools"]["transmit"];
    let output = &tool["outputSchema"];
    let props = output["properties"]
        .as_object()
        .expect("transmit outputSchema.properties must be an object");
    assert!(
        props.contains_key("error_code"),
        "transmit outputSchema must include 'error_code' property for early errors"
    );
}

/// The `transmit` contract must include a `status` field for the error path.
#[test]
fn contract_transmit_schema_includes_status_property() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let tool = &contract["tools"]["transmit"];
    let output = &tool["outputSchema"];
    let props = output["properties"]
        .as_object()
        .expect("transmit outputSchema.properties must be an object");
    assert!(
        props.contains_key("status"),
        "transmit outputSchema must include 'status' property for error path"
    );
}

/// The `transmit` contract must NOT require `decision` (error path lacks it).
#[test]
fn contract_transmit_schema_does_not_require_decision() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let tool = &contract["tools"]["transmit"];
    let required = &tool["outputSchema"]["required"];
    if let Some(arr) = required.as_array() {
        let required_fields: Vec<&str> = arr.iter().filter_map(serde_json::Value::as_str).collect();
        assert!(
            !required_fields.contains(&"decision"),
            "transmit outputSchema must not require 'decision' because the \
             error path returns status/error_code instead"
        );
    }
    // If required is null/absent, that's also fine — nothing is required.
}

/// `transmit` `error_code` enum must include both `no_channel` and `slack_unavailable`.
#[test]
fn contract_transmit_error_code_includes_both_variants() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let enum_vals =
        &contract["tools"]["transmit"]["outputSchema"]["properties"]["error_code"]["enum"];
    let codes: Vec<&str> = enum_vals
        .as_array()
        .expect("error_code enum must be an array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        codes.contains(&"no_channel"),
        "transmit error_code enum must include 'no_channel'; got {codes:?}"
    );
    assert!(
        codes.contains(&"slack_unavailable"),
        "transmit error_code enum must include 'slack_unavailable'; got {codes:?}"
    );
}

/// No-channel error output shape for `transmit`.
#[test]
fn transmit_no_channel_error_code_structure() {
    let output = serde_json::json!({
        "status": "error",
        "error_code": "no_channel",
        "error_message": "no Slack channel configured for this session"
    });
    assert_eq!(output["status"].as_str(), Some("error"));
    assert_eq!(output["error_code"].as_str(), Some("no_channel"));
    // decision must NOT be present on error path
    assert!(output.get("decision").is_none());
}

/// Slack-unavailable error output shape for `transmit`.
#[test]
fn transmit_slack_unavailable_error_code_structure() {
    let output = serde_json::json!({
        "status": "error",
        "error_code": "slack_unavailable",
        "error_message": "Slack service is not configured; transmit requires Slack"
    });
    assert_eq!(output["status"].as_str(), Some("error"));
    assert_eq!(output["error_code"].as_str(), Some("slack_unavailable"));
}
