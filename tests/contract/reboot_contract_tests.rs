//! Contract tests for extended `recover_state` (reboot) response shape (T030).
//!
//! Validates the JSON output schema for the `recover_state` tool when inbox
//! tasks are present and absent (scenarios S015-S016).
//! These are pure schema shape tests; live execution is covered in
//! `integration/inbox_flow_tests.rs`.

use serde_json::{json, Value};

// ─── S015: clean state — pending_tasks always present as empty array ───

/// When no inbox tasks exist the response carries `status: "clean"` and
/// an empty `pending_tasks` array. The field must always be present so that
/// clients can rely on a stable schema without nil-checking.
#[test]
fn clean_response_always_has_pending_tasks_field() {
    let response = json!({ "status": "clean", "pending_tasks": [] });

    let status = response.get("status").and_then(Value::as_str).unwrap_or("");
    assert_eq!(status, "clean");
    let tasks = response
        .get("pending_tasks")
        .and_then(Value::as_array)
        .expect("pending_tasks must always be present, even when empty");
    assert!(
        tasks.is_empty(),
        "pending_tasks must be empty when no inbox items exist"
    );
}

// ─── S016: recovered with tasks — pending_tasks array present ────────

/// When inbox tasks are present the response includes a `pending_tasks`
/// array whose items carry `task_id`, `message`, and `created_at` fields.
#[test]
fn recovered_response_includes_pending_tasks_array() {
    let response = json!({
        "status": "clean",
        "pending_tasks": [
            {
                "task_id": "task:abc-123",
                "message": "run integration tests",
                "source": "slack",
                "created_at": "2026-02-25T12:00:00Z"
            },
            {
                "task_id": "task:def-456",
                "message": "fix linting",
                "source": "ipc",
                "created_at": "2026-02-25T12:01:00Z"
            }
        ]
    });

    let tasks = response
        .get("pending_tasks")
        .and_then(Value::as_array)
        .expect("pending_tasks array required");

    assert_eq!(tasks.len(), 2, "both tasks should be present");

    for task in tasks {
        assert!(task.get("task_id").is_some(), "task_id field required");
        assert!(task.get("message").is_some(), "message field required");
        assert!(
            task.get("created_at").is_some(),
            "created_at field required"
        );
    }
}

/// `pending_tasks` items must have a `source` field that is `"slack"` or `"ipc"`.
#[test]
fn pending_task_source_is_enum_value() {
    let valid_sources = ["slack", "ipc"];

    let task = json!({
        "task_id": "task:abc",
        "message": "do something",
        "source": "slack",
        "created_at": "2026-02-25T12:00:00Z"
    });

    let source = task.get("source").and_then(Value::as_str).unwrap_or("");
    assert!(
        valid_sources.contains(&source),
        "source must be one of {valid_sources:?}, got {source}"
    );
}

/// A response may include both `pending_requests` (session recovery) and
/// `pending_tasks` (inbox delivery) simultaneously.
#[test]
fn response_may_carry_both_pending_requests_and_tasks() {
    let response = json!({
        "status": "recovered",
        "session_id": "session:xyz",
        "pending_requests": [
            {
                "request_id": "req:001",
                "type": "approval",
                "title": "Apply auth patch",
                "created_at": "2026-02-25T11:55:00Z"
            }
        ],
        "pending_tasks": [
            {
                "task_id": "task:abc",
                "message": "run integration tests",
                "source": "slack",
                "created_at": "2026-02-25T12:00:00Z"
            }
        ]
    });

    let status = response.get("status").and_then(Value::as_str).unwrap_or("");
    assert_eq!(status, "recovered");
    assert!(response.get("pending_requests").is_some());
    assert!(response.get("pending_tasks").is_some());

    let requests = response
        .get("pending_requests")
        .and_then(Value::as_array)
        .expect("array");
    let tasks = response
        .get("pending_tasks")
        .and_then(Value::as_array)
        .expect("array");

    assert_eq!(requests.len(), 1);
    assert_eq!(tasks.len(), 1);
}
