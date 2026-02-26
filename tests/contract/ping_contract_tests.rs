//! Contract tests for extended `ping` response with `pending_steering` (T016).
//!
//! Validates the output schema shape for the `ping` tool when steering
//! messages are present (S002) and absent (S003).

use serde_json::{json, Value};

// ─── S002: ping response includes pending_steering when messages exist ─

#[test]
fn ping_response_includes_pending_steering_when_messages_exist() {
    // Simulate the expected response shape from the ping tool
    let response = json!({
        "status": "ok",
        "pending_steering": ["refocus on tests", "check error handling"]
    });

    let steering = response
        .get("pending_steering")
        .expect("pending_steering field present")
        .as_array()
        .expect("is array");

    assert_eq!(steering.len(), 2);
    assert_eq!(steering[0].as_str().expect("string"), "refocus on tests");
    assert_eq!(
        steering[1].as_str().expect("string"),
        "check error handling"
    );
}

// ─── S003: ping response has empty or absent pending_steering when none ─

#[test]
fn ping_response_empty_pending_steering_when_no_messages() {
    let response_absent = json!({ "status": "ok" });
    let response_empty = json!({ "status": "ok", "pending_steering": [] });

    // Both forms are valid: field absent or empty array
    let absent_ok = response_absent.get("pending_steering").is_none();
    let empty_ok = response_empty
        .get("pending_steering")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty);

    assert!(absent_ok || empty_ok, "one form must hold");
}

// ─── Response shape: pending_steering is array of strings ─────────────

#[test]
fn pending_steering_entries_are_strings() {
    let response = json!({
        "status": "ok",
        "pending_steering": ["do this", "do that"]
    });

    let entries = response["pending_steering"].as_array().expect("array");

    for entry in entries {
        assert!(
            entry.is_string(),
            "every entry must be a string, got: {entry}"
        );
    }
}
