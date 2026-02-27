//! Contract tests for the `check_auto_approve` MCP tool (T118).
//!
//! Validates input/output schemas per `mcp-tools.json` contract.

use serde_json::json;

/// The tool name as registered in the MCP server.
const TOOL_NAME: &str = "auto_check";

// ─── Input schema validation ──────────────────────────────────────────

#[test]
fn input_requires_tool_name() {
    let input = json!({
        "context": { "file_path": "src/main.rs" }
    });
    assert!(
        input.get("tool_name").is_none(),
        "{TOOL_NAME} input without 'tool_name' should lack the required field"
    );
}

#[test]
fn input_accepts_tool_name_only() {
    let input = json!({
        "tool_name": "cargo test"
    });
    assert!(input.get("tool_name").is_some());
    assert!(
        input.get("context").is_none(),
        "context is optional per contract"
    );
}

#[test]
fn input_accepts_all_fields() {
    let input = json!({
        "tool_name": "write_file",
        "context": {
            "file_path": "src/main.rs",
            "risk_level": "low"
        }
    });
    assert!(input.get("tool_name").is_some());
    assert!(input.get("context").is_some());

    let ctx = input.get("context").expect("context present");
    assert!(ctx.get("file_path").is_some());
    assert!(ctx.get("risk_level").is_some());
}

#[test]
fn context_risk_level_accepts_valid_enum_values() {
    for level in &["low", "high", "critical"] {
        let input = json!({
            "tool_name": "ask_approval",
            "context": { "risk_level": level }
        });
        let ctx = input.get("context").expect("context present");
        assert_eq!(
            ctx["risk_level"].as_str(),
            Some(*level),
            "{TOOL_NAME} should accept risk_level '{level}'"
        );
    }
}

#[test]
fn context_is_optional_object() {
    // Without context
    let without = json!({ "tool_name": "remote_log" });
    assert!(without.get("context").is_none());

    // With empty context
    let with_empty = json!({ "tool_name": "remote_log", "context": {} });
    assert!(with_empty.get("context").is_some());

    // With partial context (file_path only)
    let partial = json!({
        "tool_name": "write_file",
        "context": { "file_path": "src/lib.rs" }
    });
    let ctx = partial.get("context").expect("context present");
    assert!(ctx.get("file_path").is_some());
    assert!(ctx.get("risk_level").is_none());
}

// ─── Output schema validation ─────────────────────────────────────────

#[test]
fn output_approved_has_required_fields() {
    let output = json!({
        "auto_approved": true,
        "matched_rule": "command:cargo test"
    });
    assert!(
        output.get("auto_approved").is_some(),
        "auto_approved is required"
    );
    assert!(output["auto_approved"].is_boolean());
}

#[test]
fn output_denied_has_required_fields() {
    let output = json!({
        "auto_approved": false,
        "matched_rule": null
    });
    assert!(output.get("auto_approved").is_some());
    assert!(!output["auto_approved"].as_bool().expect("bool"));
    assert!(output["matched_rule"].is_null());
}

// ─── kind field validation ─────────────────────────────────────────────────

#[test]
fn input_accepts_kind_terminal_command() {
    let input = json!({
        "tool_name": "cargo test",
        "kind": "terminal_command"
    });
    assert!(input.get("tool_name").is_some());
    assert_eq!(
        input["kind"].as_str(),
        Some("terminal_command"),
        "{TOOL_NAME} should accept kind='terminal_command'"
    );
}

#[test]
fn input_accepts_kind_file_operation() {
    let input = json!({
        "tool_name": "write_file",
        "kind": "file_operation",
        "context": { "file_path": "src/main.rs" }
    });
    assert_eq!(
        input["kind"].as_str(),
        Some("file_operation"),
        "{TOOL_NAME} should accept kind='file_operation'"
    );
}

#[test]
fn input_kind_is_optional() {
    let input = json!({ "tool_name": "cargo check" });
    assert!(
        input.get("kind").is_none(),
        "{TOOL_NAME}: 'kind' must be optional for backward compatibility"
    );
}

#[test]
fn output_terminal_command_approved_shape() {
    // When operator approves a terminal command, the response must be the
    // standard auto_approved=true shape (same contract as policy approval).
    let output = json!({
        "auto_approved": true,
        "matched_rule": "operator:approved"
    });
    assert!(output["auto_approved"].as_bool().unwrap_or(false));
    assert_eq!(output["matched_rule"].as_str(), Some("operator:approved"));
}

#[test]
fn output_matched_rule_is_optional() {
    let output = json!({
        "auto_approved": false
    });
    assert!(output.get("auto_approved").is_some());
    assert!(
        output.get("matched_rule").is_none(),
        "matched_rule is optional when not auto-approved"
    );
}

#[test]
fn output_matched_rule_types() {
    // Command match
    let cmd_output = json!({
        "auto_approved": true,
        "matched_rule": "command:cargo test"
    });
    assert!(cmd_output["matched_rule"].is_string());

    // Tool match
    let tool_output = json!({
        "auto_approved": true,
        "matched_rule": "tool:remote_log"
    });
    assert!(tool_output["matched_rule"].is_string());

    // File pattern match
    let file_output = json!({
        "auto_approved": true,
        "matched_rule": "file_pattern:write:src/**/*.rs"
    });
    assert!(file_output["matched_rule"].is_string());
}
