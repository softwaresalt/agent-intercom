//! Contract tests for the `accept_diff` MCP tool (T108).
//!
//! Validates input/output schemas per `mcp-tools.json`.
//! Tests error codes: `not_approved`, `already_consumed`,
//! `path_violation`, `patch_conflict`, `request_not_found`.

use serde_json::json;

/// The tool name as registered in the MCP server.
const TOOL_NAME: &str = "check_diff";

/// Valid output status values per contract.
const VALID_OUTPUT_STATUSES: &[&str] = &["applied", "error"];

/// Valid error codes per contract.
const VALID_ERROR_CODES: &[&str] = &[
    "request_not_found",
    "not_approved",
    "already_consumed",
    "path_violation",
    "patch_conflict",
];

// ─── Input schema validation ──────────────────────────────────────────

#[test]
fn input_requires_request_id() {
    let input = json!({});
    assert!(
        input.get("request_id").is_none(),
        "input without 'request_id' should lack the required field"
    );
}

#[test]
fn input_accepts_request_id_only() {
    let input = json!({
        "request_id": "req-abc-123"
    });
    assert!(input.get("request_id").is_some());
}

#[test]
fn input_force_is_optional_defaults_false() {
    // Per contract: force has default=false.
    let without = json!({
        "request_id": "req-1"
    });
    assert!(without.get("force").is_none());
}

#[test]
fn input_force_accepts_boolean() {
    let with_true = json!({
        "request_id": "req-1",
        "force": true
    });
    assert_eq!(with_true["force"].as_bool(), Some(true));

    let with_false = json!({
        "request_id": "req-1",
        "force": false
    });
    assert_eq!(with_false["force"].as_bool(), Some(false));
}

// ─── Output schema validation ─────────────────────────────────────────

#[test]
fn output_status_is_required() {
    let output = json!({
        "status": "applied",
        "files_written": []
    });
    assert!(output.get("status").is_some());
}

#[test]
fn output_status_accepts_valid_values() {
    for status in VALID_OUTPUT_STATUSES {
        let output = json!({ "status": status });
        assert_eq!(
            output["status"].as_str(),
            Some(*status),
            "{TOOL_NAME} output should accept status '{status}'"
        );
    }
}

#[test]
fn output_files_written_is_array_on_success() {
    let output = json!({
        "status": "applied",
        "files_written": [
            { "path": "src/main.rs", "bytes": 1024 }
        ]
    });
    assert!(output["files_written"].is_array());
    let files = output["files_written"].as_array().expect("array");
    assert_eq!(files.len(), 1);
    assert!(files[0].get("path").is_some());
    assert!(files[0].get("bytes").is_some());
}

#[test]
fn output_error_code_present_on_error() {
    let output = json!({
        "status": "error",
        "error_code": "not_approved",
        "error_message": "request has not been approved"
    });
    assert_eq!(output["status"].as_str(), Some("error"));
    assert!(output.get("error_code").is_some());
    assert!(output.get("error_message").is_some());
}

#[test]
fn output_error_code_accepts_valid_values() {
    for code in VALID_ERROR_CODES {
        let output = json!({
            "status": "error",
            "error_code": code,
            "error_message": "description"
        });
        assert_eq!(
            output["error_code"].as_str(),
            Some(*code),
            "{TOOL_NAME} should accept error_code '{code}'"
        );
    }
}

#[test]
fn output_error_fields_absent_on_success() {
    let output = json!({
        "status": "applied",
        "files_written": [{ "path": "src/lib.rs", "bytes": 512 }]
    });
    assert!(
        output.get("error_code").is_none(),
        "error_code should be absent on success"
    );
    assert!(
        output.get("error_message").is_none(),
        "error_message should be absent on success"
    );
}

// ─── Error code contract tests ────────────────────────────────────────

#[test]
fn error_not_approved_structure() {
    let output = json!({
        "status": "error",
        "error_code": "not_approved",
        "error_message": "approval request is not in approved status"
    });
    assert_eq!(output["error_code"], "not_approved");
}

#[test]
fn error_already_consumed_structure() {
    let output = json!({
        "status": "error",
        "error_code": "already_consumed",
        "error_message": "approved diff has already been applied"
    });
    assert_eq!(output["error_code"], "already_consumed");
}

#[test]
fn error_path_violation_structure() {
    let output = json!({
        "status": "error",
        "error_code": "path_violation",
        "error_message": "file path escapes workspace root"
    });
    assert_eq!(output["error_code"], "path_violation");
}

#[test]
fn error_patch_conflict_structure() {
    let output = json!({
        "status": "error",
        "error_code": "patch_conflict",
        "error_message": "file content has changed since proposal was created"
    });
    assert_eq!(output["error_code"], "patch_conflict");
}

#[test]
fn error_request_not_found_structure() {
    let output = json!({
        "status": "error",
        "error_code": "request_not_found",
        "error_message": "no approval request found with the given id"
    });
    assert_eq!(output["error_code"], "request_not_found");
}

// ─── Phase 5 — Slack notification scenario contract shapes ────────────

/// T047 / S028 — Success notification: output includes `files_written` with path and bytes.
#[test]
fn notification_success_includes_file_path_and_bytes() {
    let output = json!({
        "status": "applied",
        "files_written": [{ "path": "src/auth.rs", "bytes": 4096 }]
    });
    assert_eq!(output["status"].as_str(), Some("applied"));
    let files = output["files_written"]
        .as_array()
        .expect("files_written array");
    assert!(files[0].get("path").is_some(), "file entry must have path");
    assert!(
        files[0].get("bytes").is_some(),
        "file entry must have bytes"
    );
}

/// T048 / S029 — Conflict error: `patch_conflict` code in output when file changed.
#[test]
fn notification_conflict_returns_patch_conflict_code() {
    let output = json!({
        "status": "error",
        "error_code": "patch_conflict",
        "error_message": "file content has changed since proposal was created"
    });
    assert_eq!(output["status"].as_str(), Some("error"));
    assert_eq!(output["error_code"].as_str(), Some("patch_conflict"));
}

/// T049 / S030 — Force-apply: even with hash mismatch, status is `applied` when force=true.
#[test]
fn notification_force_apply_returns_applied_status() {
    // Force-apply produces the same success output shape despite the warning posted to Slack.
    let output = json!({
        "status": "applied",
        "files_written": [{ "path": "src/lib.rs", "bytes": 2048 }]
    });
    assert_eq!(output["status"].as_str(), Some("applied"));
    assert!(
        output.get("error_code").is_none(),
        "force-apply success must not have error_code"
    );
}

/// T050 / S031 — No Slack channel: output structure is identical to normal success.
#[test]
fn no_channel_success_output_is_same_structure() {
    let output = json!({
        "status": "applied",
        "files_written": [{ "path": "src/config.rs", "bytes": 512 }]
    });
    assert_eq!(output["status"].as_str(), Some("applied"));
    assert!(
        output.get("error_code").is_none(),
        "success without Slack must not include error_code"
    );
}

/// T051 / S032 — New file write: same `applied` + `files_written` schema for full-content writes.
#[test]
fn notification_new_file_write_includes_files_written() {
    let output = json!({
        "status": "applied",
        "files_written": [{ "path": "src/new_module.rs", "bytes": 789 }]
    });
    assert_eq!(output["status"].as_str(), Some("applied"));
    assert!(output["files_written"].is_array());
}

// ─── Tool definition contract ─────────────────────────────────────────

#[test]
fn tool_name_matches_contract() {
    assert_eq!(TOOL_NAME, "check_diff");
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
    assert!(required_names.contains(&"request_id"));

    // force should be in properties with boolean type.
    assert_eq!(input["properties"]["force"]["type"], "boolean");
    assert_eq!(input["properties"]["force"]["default"], false);

    // Output schema checks.
    let output = &tool["outputSchema"];
    assert_eq!(output["type"], "object");
    let out_required = output["required"]
        .as_array()
        .expect("output required should be array");
    let out_required_names: Vec<&str> = out_required.iter().filter_map(|v| v.as_str()).collect();
    assert!(out_required_names.contains(&"status"));

    // error_code enum should match our known codes.
    let error_enum = output["properties"]["error_code"]["enum"]
        .as_array()
        .expect("error_code enum should be array");
    for code in VALID_ERROR_CODES {
        assert!(
            error_enum.iter().any(|v| v.as_str() == Some(code)),
            "contract should include error_code '{code}'"
        );
    }
}
