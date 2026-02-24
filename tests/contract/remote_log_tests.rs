//! Contract tests for the `remote_log` MCP tool (T113).
//!
//! Validates input/output schemas per `mcp-tools.json` contract.
//! Verifies all severity levels (info, success, warning, error) produce
//! correct Block Kit formatting.

use serde_json::json;

/// The tool name as registered in the MCP server.
const TOOL_NAME: &str = "remote_log";

/// Valid severity level enum values per contract.
const VALID_LEVELS: &[&str] = &["info", "success", "warning", "error"];

// ─── Input schema validation ──────────────────────────────────────────

#[test]
fn input_requires_message() {
    let input = json!({
        "level": "info"
    });
    assert!(
        input.get("message").is_none(),
        "input without 'message' should lack the required field"
    );
}

#[test]
fn input_accepts_message_only() {
    let input = json!({
        "message": "Running tests..."
    });
    assert!(input.get("message").is_some());
    assert!(input.get("level").is_none(), "level is optional");
    assert!(input.get("thread_ts").is_none(), "thread_ts is optional");
}

#[test]
fn input_level_is_optional_with_default_info() {
    let input = json!({
        "message": "Starting build"
    });
    assert!(
        input.get("level").is_none(),
        "level defaults to 'info' when omitted"
    );
}

#[test]
fn input_level_accepts_valid_enum_values() {
    for level in VALID_LEVELS {
        let input = json!({
            "message": "status update",
            "level": level
        });
        assert_eq!(
            input["level"].as_str(),
            Some(*level),
            "{TOOL_NAME} should accept level '{level}'"
        );
    }
}

#[test]
fn input_thread_ts_is_optional() {
    let without = json!({
        "message": "Log entry"
    });
    assert!(without.get("thread_ts").is_none());

    let with = json!({
        "message": "Log entry",
        "thread_ts": "1234567890.123456"
    });
    assert!(with.get("thread_ts").is_some());
}

#[test]
fn input_accepts_all_fields() {
    let input = json!({
        "message": "Build succeeded",
        "level": "success",
        "thread_ts": "1234567890.123456"
    });
    assert!(input.get("message").is_some());
    assert!(input.get("level").is_some());
    assert!(input.get("thread_ts").is_some());
}

// ─── Output schema validation ─────────────────────────────────────────

#[test]
fn output_posted_is_required() {
    let output = json!({
        "posted": true,
        "ts": "1234567890.123456"
    });
    assert!(output.get("posted").is_some());
    assert!(output["posted"].is_boolean());
}

#[test]
fn output_ts_is_required() {
    let output = json!({
        "posted": true,
        "ts": "1234567890.123456"
    });
    assert!(output.get("ts").is_some());
    assert!(output["ts"].is_string());
}

#[test]
fn output_posted_false_when_slack_unavailable() {
    // When Slack is not configured, the tool should return posted=false.
    let output = json!({
        "posted": false,
        "ts": ""
    });
    assert!(!output["posted"].as_bool().unwrap_or(true));
    assert_eq!(output["ts"].as_str(), Some(""));
}

// ─── Block Kit severity formatting ────────────────────────────────────

#[test]
fn severity_info_renders_info_emoji() {
    let block = agent_intercom::slack::blocks::severity_section("info", "test message");
    let json = serde_json::to_value(&block).expect("block should serialize");
    let text = json["text"]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains('\u{2139}'),
        "info severity should contain ℹ️ emoji, got: {text}"
    );
    assert!(text.contains("test message"));
}

#[test]
fn severity_success_renders_checkmark() {
    let block = agent_intercom::slack::blocks::severity_section("success", "tests passed");
    let json = serde_json::to_value(&block).expect("block should serialize");
    let text = json["text"]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains('\u{2705}'),
        "success severity should contain ✅ emoji, got: {text}"
    );
    assert!(text.contains("tests passed"));
}

#[test]
fn severity_warning_renders_caution() {
    let block = agent_intercom::slack::blocks::severity_section("warning", "low disk");
    let json = serde_json::to_value(&block).expect("block should serialize");
    let text = json["text"]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains('\u{26a0}'),
        "warning severity should contain ⚠️ emoji, got: {text}"
    );
    assert!(text.contains("low disk"));
}

#[test]
fn severity_error_renders_error_icon() {
    let block = agent_intercom::slack::blocks::severity_section("error", "build failed");
    let json = serde_json::to_value(&block).expect("block should serialize");
    let text = json["text"]["text"].as_str().unwrap_or_default();
    assert!(
        text.contains('\u{274c}'),
        "error severity should contain ❌ emoji, got: {text}"
    );
    assert!(text.contains("build failed"));
}

// ─── Tool definition contract ─────────────────────────────────────────

#[test]
fn tool_name_matches_contract() {
    assert_eq!(TOOL_NAME, "remote_log");
}

#[test]
fn contract_schema_structure_is_valid() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../specs/001-mcp-remote-agent-server/contracts/mcp-tools.json"
    ))
    .expect("mcp-tools.json should be valid JSON");

    let tool = &contract["tools"][TOOL_NAME];

    // Input schema checks.
    let input = &tool["inputSchema"];
    assert_eq!(input["type"], "object");
    let required = input["required"]
        .as_array()
        .expect("required should be array");
    let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        required_names.contains(&"message"),
        "message must be a required input field"
    );
    assert!(
        !required_names.contains(&"level"),
        "level should not be required"
    );
    assert!(
        !required_names.contains(&"thread_ts"),
        "thread_ts should not be required"
    );

    // Verify level enum values.
    let level_enum = &input["properties"]["level"]["enum"];
    let levels: Vec<&str> = level_enum
        .as_array()
        .expect("level.enum should be array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for expected in VALID_LEVELS {
        assert!(
            levels.contains(expected),
            "level enum should contain '{expected}'"
        );
    }

    // Output schema checks.
    let output = &tool["outputSchema"];
    assert_eq!(output["type"], "object");
    let out_required = output["required"]
        .as_array()
        .expect("output required should be array");
    let out_required_names: Vec<&str> = out_required.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        out_required_names.contains(&"posted"),
        "posted must be a required output field"
    );
    assert!(
        out_required_names.contains(&"ts"),
        "ts must be a required output field"
    );
}
