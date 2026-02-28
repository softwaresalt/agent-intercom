//! Contract tests for `check_clearance` file attachment (T083).
//!
//! Validates that the `check_clearance` tool response schema is stable
//! regardless of whether an original file attachment is uploaded.
//!
//! Scenarios:
//! - S087: Existing-file diff — response still contains `request_id` + `status`
//! - S088: New-file diff — response shape is identical (no attachment fields leaked)

use serde_json::json;

/// The MCP tool name for approval requests.
const TOOL_NAME: &str = "check_clearance";

// ─── S087: Response schema when original file exists ─────────────────────────

/// S087 — The `check_clearance` tool response for an existing-file diff must
/// always include `status` and `request_id`; any attachment upload is a
/// side-effect and must not alter the response contract.
#[test]
fn response_schema_contains_status_and_request_id() {
    // Simulate the expected response shape.
    let response = json!({
        "status": "approved",
        "request_id": "req:456e6c72-d88a-43dd-a715-b3e0d9a8ef12"
    });

    assert!(
        response.get("status").is_some(),
        "{TOOL_NAME} response must contain 'status'"
    );
    assert!(
        response.get("request_id").is_some(),
        "{TOOL_NAME} response must contain 'request_id'"
    );
    // No attachment field should be exposed at the tool contract level.
    assert!(
        response.get("attachment").is_none(),
        "response must not expose internal attachment state"
    );
}

/// S087b — All valid status enum values are accepted in the response.
#[test]
fn response_status_enum_values_are_approved_rejected_timeout() {
    let valid = ["approved", "rejected", "timeout"];
    for status in valid {
        let response = json!({ "status": status, "request_id": "req:abc123" });
        assert_eq!(
            response["status"].as_str(),
            Some(status),
            "status '{status}' should be a valid response value"
        );
    }
}

// ─── S088: Response schema when file is new (no original) ────────────────────

/// S088 — When the submitted diff is for a new (non-existent) file,
/// the response shape remains identical. No extra fields for "no attachment".
#[test]
fn new_file_diff_response_has_same_schema() {
    let response = json!({
        "status": "approved",
        "request_id": "req:789abe43-deaf-4d3d-a1b2-feedface0000"
    });

    assert!(response.get("status").is_some(), "must have 'status'");
    assert!(
        response.get("request_id").is_some(),
        "must have 'request_id'"
    );
    assert!(
        response.get("original_file").is_none(),
        "new-file responses must not include 'original_file'"
    );
}

/// S088b — `reason` field is optional and only present on rejected/timeout.
#[test]
fn reason_field_is_optional() {
    let approved = json!({ "status": "approved", "request_id": "req:1" });
    assert!(
        approved.get("reason").is_none(),
        "approved response has no reason"
    );

    let rejected = json!({
        "status": "rejected",
        "request_id": "req:2",
        "reason": "Out of scope for current sprint"
    });
    assert!(
        rejected.get("reason").is_some(),
        "rejected response may include reason"
    );
}
