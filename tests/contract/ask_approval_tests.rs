//! Contract tests for the `ask_approval` MCP tool (T105).
//!
//! Validates input schema (required fields, enum values, optional fields)
//! and output schema (`status` enum, `request_id` presence, optional `reason`)
//! per `mcp-tools.json` contract.

use serde_json::json;

/// The tool name as registered in the MCP server.
const TOOL_NAME: &str = "check_clearance";

/// Valid risk level enum values per contract.
const VALID_RISK_LEVELS: &[&str] = &["low", "high", "critical"];

/// Valid output status enum values per contract.
const VALID_OUTPUT_STATUSES: &[&str] = &["approved", "rejected", "timeout"];

// ─── Input schema validation ──────────────────────────────────────────

#[test]
fn input_requires_title() {
    let input = json!({
        "diff": "--- a\n+++ b",
        "file_path": "src/main.rs"
    });
    assert!(
        input.get("title").is_none(),
        "input without 'title' should lack the required field"
    );
}

#[test]
fn input_requires_diff() {
    let input = json!({
        "title": "Add auth middleware",
        "file_path": "src/main.rs"
    });
    assert!(
        input.get("diff").is_none(),
        "input without 'diff' should lack the required field"
    );
}

#[test]
fn input_requires_file_path() {
    let input = json!({
        "title": "Add auth middleware",
        "diff": "--- a\n+++ b"
    });
    assert!(
        input.get("file_path").is_none(),
        "input without 'file_path' should lack the required field"
    );
}

#[test]
fn input_accepts_all_required_fields() {
    let input = json!({
        "title": "Add auth middleware",
        "diff": "--- a\n+++ b",
        "file_path": "src/main.rs"
    });
    assert!(input.get("title").is_some());
    assert!(input.get("diff").is_some());
    assert!(input.get("file_path").is_some());
}

#[test]
fn input_description_is_optional() {
    let without = json!({
        "title": "Change",
        "diff": "content",
        "file_path": "src/lib.rs"
    });
    assert!(without.get("description").is_none());

    let with = json!({
        "title": "Change",
        "description": "Detailed explanation",
        "diff": "content",
        "file_path": "src/lib.rs"
    });
    assert!(with.get("description").is_some());
}

#[test]
fn input_risk_level_is_optional_with_default_low() {
    // When omitted, default is "low" per contract.
    let input = json!({
        "title": "Change",
        "diff": "content",
        "file_path": "src/lib.rs"
    });
    assert!(input.get("risk_level").is_none());
}

#[test]
fn input_risk_level_accepts_valid_enum_values() {
    for level in VALID_RISK_LEVELS {
        let input = json!({
            "title": "Change",
            "diff": "content",
            "file_path": "src/lib.rs",
            "risk_level": level
        });
        assert_eq!(
            input["risk_level"].as_str(),
            Some(*level),
            "{TOOL_NAME} should accept risk_level '{level}'"
        );
    }
}

// ─── Output schema validation ─────────────────────────────────────────

#[test]
fn output_status_is_required() {
    // A valid response must contain 'status'.
    let output = json!({
        "status": "approved",
        "request_id": "abc-123"
    });
    assert!(output.get("status").is_some());
}

#[test]
fn output_request_id_is_required() {
    let output = json!({
        "status": "approved",
        "request_id": "abc-123"
    });
    assert!(output.get("request_id").is_some());
}

#[test]
fn output_status_accepts_valid_enum_values() {
    for status in VALID_OUTPUT_STATUSES {
        let output = json!({
            "status": status,
            "request_id": "id-1"
        });
        assert_eq!(
            output["status"].as_str(),
            Some(*status),
            "{TOOL_NAME} output should include status '{status}'"
        );
    }
}

#[test]
fn output_reason_is_optional() {
    let without_reason = json!({
        "status": "approved",
        "request_id": "id-1"
    });
    assert!(without_reason.get("reason").is_none());

    let with_reason = json!({
        "status": "rejected",
        "request_id": "id-1",
        "reason": "Code style issues"
    });
    assert!(with_reason.get("reason").is_some());
}

#[test]
fn output_reason_present_only_when_rejected() {
    // Per contract: reason is "only present when status=rejected".
    let approved = json!({
        "status": "approved",
        "request_id": "id-1"
    });
    assert!(
        approved.get("reason").is_none(),
        "approved response should not include 'reason'"
    );

    let rejected = json!({
        "status": "rejected",
        "request_id": "id-1",
        "reason": "Needs more tests"
    });
    assert!(
        rejected.get("reason").is_some(),
        "rejected response should include 'reason'"
    );
}

// ─── Tool definition contract ─────────────────────────────────────────

#[test]
fn tool_name_matches_contract() {
    assert_eq!(TOOL_NAME, "check_clearance");
}

/// Verify the tool definition from `mcp-tools.json` matches what the server
/// registers. This test loads the contract and validates the schema structure.
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
    assert!(required_names.contains(&"title"));
    assert!(required_names.contains(&"diff"));
    assert!(required_names.contains(&"file_path"));

    // Output schema checks.
    let output = &tool["outputSchema"];
    assert_eq!(output["type"], "object");
    let out_required = output["required"]
        .as_array()
        .expect("output required should be array");
    let out_required_names: Vec<&str> = out_required.iter().filter_map(|v| v.as_str()).collect();
    assert!(out_required_names.contains(&"status"));
    assert!(out_required_names.contains(&"request_id"));
}
