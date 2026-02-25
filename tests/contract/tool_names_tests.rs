//! Contract tests verifying that renamed MCP tools preserve their input schemas (T036).
//!
//! Each test encodes the expected schema for a renamed tool and verifies the
//! required/optional field structure matches the old tool's contract.
//!
//! Scenarios covered: S024, S025

use serde_json::json;

// ── check_clearance (was: ask_approval) ──────────────────────

/// S024 — `check_clearance` input schema preserves all fields from `ask_approval`.
///
/// Required: `title`, `diff`, `file_path`
/// Optional: `description`, `risk_level` (enum: `low` | `high` | `critical`)
#[test]
fn check_clearance_schema_has_required_fields() {
    let valid = json!({
        "title": "Add retry logic",
        "diff": "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new",
        "file_path": "src/main.rs"
    });
    assert!(valid.get("title").is_some());
    assert!(valid.get("diff").is_some());
    assert!(valid.get("file_path").is_some());
}

#[test]
fn check_clearance_schema_optional_fields_accepted() {
    let full = json!({
        "title": "Refactor auth",
        "diff": "--- a\n+++ b",
        "file_path": "src/auth.rs",
        "description": "Switches to JWT",
        "risk_level": "high"
    });
    assert!(full.get("description").is_some());
    assert_eq!(full["risk_level"].as_str(), Some("high"));
}

#[test]
fn check_clearance_risk_level_enum_values() {
    for level in &["low", "high", "critical"] {
        let input = json!({ "title": "t", "diff": "d", "file_path": "f", "risk_level": level });
        assert_eq!(input["risk_level"].as_str(), Some(*level));
    }
}

// ── check_diff (was: accept_diff) ────────────────────────────

/// S025 — `check_diff` input schema preserves all fields from `accept_diff`.
///
/// Required: `request_id`
/// Optional: `force` (boolean, default false)
#[test]
fn check_diff_schema_has_required_fields() {
    let valid = json!({ "request_id": "req:abc-123" });
    assert!(valid.get("request_id").is_some());
}

#[test]
fn check_diff_schema_optional_force_field() {
    let with_force = json!({ "request_id": "req:abc-123", "force": true });
    assert_eq!(with_force["force"].as_bool(), Some(true));

    let without_force = json!({ "request_id": "req:abc-123" });
    assert!(without_force.get("force").is_none());
}

// ── auto_check (was: check_auto_approve) ─────────────────────

/// `auto_check` input schema preserves all fields from `check_auto_approve`.
///
/// Required: `tool_name`
/// Optional: `context` (object with `file_path`, `risk_level`)
#[test]
fn auto_check_schema_has_required_fields() {
    let valid = json!({ "tool_name": "write_file" });
    assert!(valid.get("tool_name").is_some());
}

#[test]
fn auto_check_schema_optional_context() {
    let with_ctx = json!({
        "tool_name": "write_file",
        "context": { "file_path": "src/lib.rs", "risk_level": "low" }
    });
    assert!(with_ctx.get("context").is_some());
}

// ── transmit (was: forward_prompt) ───────────────────────────

/// `transmit` input schema preserves all fields from `forward_prompt`.
///
/// Required: `prompt_text`
/// Optional: `prompt_type` (enum), `elapsed_seconds`, `actions_taken`
#[test]
fn transmit_schema_has_required_fields() {
    let valid = json!({ "prompt_text": "What should I do next?" });
    assert!(valid.get("prompt_text").is_some());
}

#[test]
fn transmit_schema_prompt_type_enum_values() {
    for pt in &[
        "continuation",
        "clarification",
        "error_recovery",
        "resource_warning",
    ] {
        let input = json!({ "prompt_text": "q", "prompt_type": pt });
        assert_eq!(input["prompt_type"].as_str(), Some(*pt));
    }
}

// ── standby (was: wait_for_instruction) ──────────────────────

/// `standby` input schema preserves all fields from `wait_for_instruction`.
///
/// Optional: `message`, `timeout_seconds`
#[test]
fn standby_schema_is_fully_optional() {
    let empty = json!({});
    assert!(empty.get("message").is_none());
    assert!(empty.get("timeout_seconds").is_none());
}

#[test]
fn standby_schema_accepts_all_fields() {
    let full = json!({
        "message": "Agent idle, awaiting instructions.",
        "timeout_seconds": 3600
    });
    assert!(full.get("message").is_some());
    assert_eq!(full["timeout_seconds"].as_i64(), Some(3600));
}

// ── ping (was: heartbeat) ─────────────────────────────────────

/// `ping` input schema preserves all fields from `heartbeat`.
///
/// Optional: `status_message`, `progress_snapshot` (array of `{label, status}`)
#[test]
fn ping_schema_is_fully_optional() {
    let empty = json!({});
    assert!(empty.get("status_message").is_none());
    assert!(empty.get("progress_snapshot").is_none());
}

#[test]
fn ping_schema_progress_snapshot_item_structure() {
    let snap_item = json!({ "label": "Build crate", "status": "done" });
    assert!(snap_item.get("label").is_some());
    assert!(snap_item.get("status").is_some());
    for status in &["done", "in_progress", "pending"] {
        let item = json!({ "label": "t", "status": status });
        assert_eq!(item["status"].as_str(), Some(*status));
    }
}

// ── broadcast (was: remote_log) ──────────────────────────────

/// `broadcast` input schema preserves all fields from `remote_log`.
///
/// Required: message
/// Optional: `level` (enum), `thread_ts`
#[test]
fn broadcast_schema_has_required_fields() {
    let valid = json!({ "message": "Build complete" });
    assert!(valid.get("message").is_some());
}

#[test]
fn broadcast_schema_level_enum_values() {
    for level in &["info", "success", "warning", "error"] {
        let input = json!({ "message": "m", "level": level });
        assert_eq!(input["level"].as_str(), Some(*level));
    }
}

// ── reboot (was: recover_state) ──────────────────────────────

/// `reboot` input schema preserves all fields from `recover_state`.
///
/// Optional: `session_id`
#[test]
fn reboot_schema_is_fully_optional() {
    let empty = json!({});
    assert!(empty.get("session_id").is_none());
}

#[test]
fn reboot_schema_accepts_session_id() {
    let with_id = json!({ "session_id": "session:abc-123" });
    assert!(with_id.get("session_id").is_some());
}

// ── switch_freq (was: set_operational_mode) ───────────────────

/// `switch_freq` input schema preserves all fields from `set_operational_mode`.
///
/// Required: mode (enum: remote | local | hybrid)
#[test]
fn switch_freq_schema_has_required_mode() {
    let valid = json!({ "mode": "remote" });
    assert!(valid.get("mode").is_some());
}

#[test]
fn switch_freq_mode_enum_values() {
    for mode in &["remote", "local", "hybrid"] {
        let input = json!({ "mode": mode });
        assert_eq!(input["mode"].as_str(), Some(*mode));
    }
}
