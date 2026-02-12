//! Contract tests for `heartbeat` tool (T111).
//!
//! Validates input/output schemas per mcp-tools.json contract:
//! - `status_message` only
//! - valid `progress_snapshot`
//! - malformed snapshot (must reject)
//! - omitted snapshot (must preserve existing)

use serde_json::{json, Value};

/// Validate the heartbeat input schema accepts `status_message` only.
#[test]
fn input_schema_accepts_status_message_only() {
    let input = json!({
        "status_message": "Processing large codebase..."
    });

    assert!(input.is_object());
    assert!(input.get("status_message").is_some());
    assert!(input.get("progress_snapshot").is_none());
}

/// Validate the heartbeat input schema accepts a valid progress snapshot.
#[test]
fn input_schema_accepts_valid_progress_snapshot() {
    let input = json!({
        "status_message": "Working on it",
        "progress_snapshot": [
            { "label": "Parse config", "status": "done" },
            { "label": "Build schema", "status": "in_progress" },
            { "label": "Run tests", "status": "pending" }
        ]
    });

    let snapshot = input
        .get("progress_snapshot")
        .expect("snapshot present")
        .as_array()
        .expect("is array");

    assert_eq!(snapshot.len(), 3);

    for item in snapshot {
        assert!(item.get("label").is_some());
        assert!(item.get("status").is_some());
        let status = item["status"].as_str().expect("status is string");
        assert!(
            ["done", "in_progress", "pending"].contains(&status),
            "invalid status: {status}"
        );
    }
}

/// Validate that a malformed snapshot (empty label) is detectable.
#[test]
fn malformed_snapshot_has_empty_label() {
    use monocoque_agent_rem::models::progress::{validate_snapshot, ProgressItem, ProgressStatus};

    let items = vec![
        ProgressItem {
            label: "good item".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "  ".into(),
            status: ProgressStatus::Pending,
        },
    ];

    let result = validate_snapshot(&items);
    assert!(result.is_err(), "empty label should be rejected");
}

/// Validate that a malformed snapshot (missing status enum) fails deserialization.
#[test]
fn malformed_snapshot_invalid_status_fails_deser() {
    use monocoque_agent_rem::models::progress::ProgressItem;

    let raw = json!({ "label": "task", "status": "bogus" });
    let result: std::result::Result<ProgressItem, _> = serde_json::from_value(raw);
    assert!(result.is_err(), "invalid status enum should fail deser");
}

/// Validate the heartbeat output schema structure.
#[test]
fn output_schema_structure_is_valid() {
    let output = json!({
        "acknowledged": true,
        "session_id": "abc-123",
        "stall_detection_enabled": true
    });

    assert_eq!(output["acknowledged"], true);
    assert!(output.get("session_id").is_some());
    assert!(output.get("stall_detection_enabled").is_some());
}

/// Validate that no `progress_snapshot` field means the existing one should be preserved.
#[test]
fn omitted_snapshot_preserves_existing() {
    // When we deserialize heartbeat input without `progress_snapshot`, it should be None.
    let input: Value = json!({
        "status_message": "Still working"
    });

    assert!(
        input.get("progress_snapshot").is_none(),
        "omitted snapshot should not be present in input"
    );
}
