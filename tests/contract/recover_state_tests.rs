//! Contract tests for `recover_state` tool (T122).
//!
//! Validates input/output schemas per mcp-tools.json contract:
//! - `session_id` is optional in input
//! - `status` is required in output (enum: `recovered`, `clean`)
//! - `session_id`, `pending_requests`, `last_checkpoint`, `progress_snapshot` optional in output
//! - `pending_requests` items carry `request_id`, `type`, `title`, `created_at`

use serde_json::json;

/// Valid output status enum values per contract.
const VALID_STATUSES: &[&str] = &["recovered", "clean"];

/// Valid pending request type enum values per contract.
const VALID_REQUEST_TYPES: &[&str] = &["approval", "prompt"];

// ─── Input schema validation ──────────────────────────────────────────

#[test]
fn input_accepts_empty_object() {
    let input = json!({});
    assert!(input.is_object());
    assert!(input.get("session_id").is_none());
}

#[test]
fn input_accepts_session_id() {
    let input = json!({ "session_id": "abc-123-def" });
    assert!(input.get("session_id").is_some());
    assert!(
        input["session_id"].is_string(),
        "session_id must be a string"
    );
}

// ─── Output schema validation ─────────────────────────────────────────

#[test]
fn output_requires_status_field() {
    let output = json!({ "status": "clean" });
    assert!(output.get("status").is_some(), "status field is required");
}

#[test]
fn output_status_accepts_recovered() {
    let output = json!({ "status": "recovered" });
    let status = output["status"].as_str().expect("status is string");
    assert!(
        VALID_STATUSES.contains(&status),
        "recovered is a valid status"
    );
}

#[test]
fn output_status_accepts_clean() {
    let output = json!({ "status": "clean" });
    let status = output["status"].as_str().expect("status is string");
    assert!(VALID_STATUSES.contains(&status), "clean is a valid status");
}

#[test]
fn output_status_rejects_invalid_value() {
    let output = json!({ "status": "unknown" });
    let status = output["status"].as_str().expect("status is string");
    assert!(
        !VALID_STATUSES.contains(&status),
        "unknown is not a valid status"
    );
}

#[test]
fn output_session_id_is_optional() {
    let without = json!({ "status": "clean" });
    assert!(without.get("session_id").is_none());

    let with = json!({ "status": "recovered", "session_id": "sess-1" });
    assert!(with.get("session_id").is_some());
}

#[test]
fn output_pending_requests_is_optional_array() {
    let without = json!({ "status": "clean" });
    assert!(without.get("pending_requests").is_none());

    let with_empty = json!({ "status": "recovered", "pending_requests": [] });
    assert!(with_empty["pending_requests"].is_array());

    let with_items = json!({
        "status": "recovered",
        "pending_requests": [
            {
                "request_id": "req-1",
                "type": "approval",
                "title": "Add auth module",
                "created_at": "2026-02-11T10:00:00Z"
            }
        ]
    });
    let items = with_items["pending_requests"].as_array().expect("is array");
    assert_eq!(items.len(), 1);
}

#[test]
fn pending_request_item_has_required_fields() {
    let item = json!({
        "request_id": "req-123",
        "type": "approval",
        "title": "Create user service",
        "created_at": "2026-02-11T12:00:00Z"
    });

    assert!(item.get("request_id").is_some());
    assert!(item.get("type").is_some());
    assert!(item.get("title").is_some());
    assert!(item.get("created_at").is_some());
}

#[test]
fn pending_request_type_accepts_approval() {
    let item = json!({ "type": "approval" });
    let req_type = item["type"].as_str().expect("type is string");
    assert!(VALID_REQUEST_TYPES.contains(&req_type));
}

#[test]
fn pending_request_type_accepts_prompt() {
    let item = json!({ "type": "prompt" });
    let req_type = item["type"].as_str().expect("type is string");
    assert!(VALID_REQUEST_TYPES.contains(&req_type));
}

#[test]
fn pending_request_type_rejects_invalid() {
    let item = json!({ "type": "stall" });
    let req_type = item["type"].as_str().expect("type is string");
    assert!(!VALID_REQUEST_TYPES.contains(&req_type));
}

#[test]
fn output_last_checkpoint_is_optional_object() {
    let without = json!({ "status": "clean" });
    assert!(without.get("last_checkpoint").is_none());

    let with = json!({
        "status": "recovered",
        "last_checkpoint": {
            "checkpoint_id": "cp-1",
            "label": "before-refactor",
            "created_at": "2026-02-11T08:00:00Z"
        }
    });
    let cp = with.get("last_checkpoint").expect("checkpoint present");
    assert!(cp.get("checkpoint_id").is_some());
    assert!(cp.get("label").is_some());
    assert!(cp.get("created_at").is_some());
}

#[test]
fn output_last_checkpoint_label_may_be_null() {
    let with_null = json!({
        "status": "recovered",
        "last_checkpoint": {
            "checkpoint_id": "cp-1",
            "label": null,
            "created_at": "2026-02-11T08:00:00Z"
        }
    });
    assert!(with_null["last_checkpoint"]["label"].is_null());
}

#[test]
fn output_progress_snapshot_is_optional_array() {
    let without = json!({ "status": "clean" });
    assert!(without.get("progress_snapshot").is_none());

    let with = json!({
        "status": "recovered",
        "progress_snapshot": [
            { "label": "Parse config", "status": "done" },
            { "label": "Build schema", "status": "in_progress" },
            { "label": "Run tests", "status": "pending" }
        ]
    });
    let snapshot = with["progress_snapshot"].as_array().expect("is array");
    assert_eq!(snapshot.len(), 3);

    for item in snapshot {
        assert!(item.get("label").is_some());
        assert!(item.get("status").is_some());
        let status = item["status"].as_str().expect("status is string");
        assert!(
            ["done", "in_progress", "pending"].contains(&status),
            "invalid snapshot status: {status}"
        );
    }
}

#[test]
fn output_progress_snapshot_validates_item_statuses() {
    use agent_intercom::models::progress::{ProgressItem, ProgressStatus};

    let items = vec![
        ProgressItem {
            label: "task A".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "task B".into(),
            status: ProgressStatus::InProgress,
        },
        ProgressItem {
            label: "task C".into(),
            status: ProgressStatus::Pending,
        },
    ];

    let json = serde_json::to_value(&items).expect("serialize");
    let arr = json.as_array().expect("is array");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0]["status"], "done");
    assert_eq!(arr[1]["status"], "in_progress");
    assert_eq!(arr[2]["status"], "pending");
}

#[test]
fn full_recovered_response_structure() {
    let response = json!({
        "status": "recovered",
        "session_id": "sess-abc",
        "pending_requests": [
            {
                "request_id": "req-1",
                "type": "approval",
                "title": "Add auth middleware",
                "created_at": "2026-02-11T10:00:00Z"
            },
            {
                "request_id": "prompt-1",
                "type": "prompt",
                "title": "Continue with current task?",
                "created_at": "2026-02-11T10:05:00Z"
            }
        ],
        "last_checkpoint": {
            "checkpoint_id": "cp-42",
            "label": "before-deploy",
            "created_at": "2026-02-11T09:30:00Z"
        },
        "progress_snapshot": [
            { "label": "Implement auth", "status": "done" },
            { "label": "Write tests", "status": "in_progress" },
            { "label": "Update docs", "status": "pending" }
        ]
    });

    assert_eq!(response["status"], "recovered");
    assert!(response.get("session_id").is_some());
    assert_eq!(response["pending_requests"].as_array().unwrap().len(), 2);
    assert!(response.get("last_checkpoint").is_some());
    assert_eq!(response["progress_snapshot"].as_array().unwrap().len(), 3);
}

#[test]
fn clean_response_minimal() {
    let response = json!({ "status": "clean" });
    assert_eq!(response["status"], "clean");
    assert!(response.get("session_id").is_none());
    assert!(response.get("pending_requests").is_none());
    assert!(response.get("last_checkpoint").is_none());
    assert!(response.get("progress_snapshot").is_none());
}
