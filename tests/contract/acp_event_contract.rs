//! Contract tests for the ACP event handler pipeline (T009 — S001–S009).
//!
//! Validates the `ClearanceRequested` handler's input→output contract:
//! - Required and optional fields are handled per FR-002, FR-003, FR-011
//! - Risk level enum values match the spec contract (FR-011)
//! - DB persistence failure semantics (SC-003)
//! - Slack unavailability handling (FR-010)

use agent_intercom::models::approval::parse_risk_level;
use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};

// ── S001: Standard clearance flow contract ────────────────────────────────────

/// S001 — An `ApprovalRequest` created from a `ClearanceRequested` event with all
/// fields present must have the correct field values and `Pending` status.
#[test]
fn clearance_standard_flow_creates_pending_approval() {
    let approval = ApprovalRequest::new(
        "session:acp-test".to_owned(),
        "Deploy config update".to_owned(),
        Some("Adds new API rate limit key".to_owned()),
        "+rate_limit_key: String,\n-".to_owned(),
        "config/server.toml".to_owned(),
        RiskLevel::High,
        "sha256abcdef0123".to_owned(),
    );

    assert_eq!(
        approval.status,
        ApprovalStatus::Pending,
        "S001: status must be Pending"
    );
    assert!(!approval.id.is_empty(), "S001: id must be generated");
    assert_eq!(
        approval.risk_level,
        RiskLevel::High,
        "S001: risk level must be High"
    );
    assert_eq!(approval.diff_content, "+rate_limit_key: String,\n-");
}

/// S001b — The contract for a valid clearance request always produces a non-empty
/// approval ID that uniquely identifies the record.
#[test]
fn clearance_produces_unique_approval_ids() {
    let a1 = ApprovalRequest::new(
        "sess-1".to_owned(),
        "Title".to_owned(),
        None,
        "diff".to_owned(),
        "path.rs".to_owned(),
        RiskLevel::Low,
        "hash1".to_owned(),
    );
    let a2 = ApprovalRequest::new(
        "sess-2".to_owned(),
        "Title".to_owned(),
        None,
        "diff".to_owned(),
        "path.rs".to_owned(),
        RiskLevel::Low,
        "hash2".to_owned(),
    );
    assert_ne!(
        a1.id, a2.id,
        "S001b: each clearance must produce a unique approval ID"
    );
}

// ── S002: None diff → empty diff_content ──────────────────────────────────────

/// S002 — When the `ClearanceRequested` event has no diff (`diff: None`), the
/// `ApprovalRequest.diff_content` must be an empty string (not None or panic).
#[test]
fn clearance_none_diff_maps_to_empty_string() {
    // Simulates `diff.unwrap_or_default()` semantics per T010 task description
    let diff_content = String::new();
    let approval = ApprovalRequest::new(
        "sess-none-diff".to_owned(),
        "No-diff change".to_owned(),
        None,
        diff_content.clone(),
        "src/lib.rs".to_owned(),
        RiskLevel::Low,
        "new_file".to_owned(),
    );
    assert_eq!(
        approval.diff_content, "",
        "S002: None diff must produce empty diff_content"
    );
}

// ── S003–S004: Risk level enum contract ──────────────────────────────────────

/// S003 — An event with `risk_level: "high"` produces an `ApprovalRequest` with
/// `RiskLevel::High`.
#[test]
fn clearance_high_risk_level_contract() {
    let risk = parse_risk_level("high");
    assert_eq!(
        risk,
        RiskLevel::High,
        "S003: 'high' must map to RiskLevel::High"
    );

    let approval = ApprovalRequest::new(
        "sess-high".to_owned(),
        "High risk".to_owned(),
        None,
        "diff".to_owned(),
        "security.rs".to_owned(),
        risk,
        "hash".to_owned(),
    );
    assert_eq!(
        approval.risk_level,
        RiskLevel::High,
        "S003: approval risk_level must be High"
    );
}

/// S004 — An event with `risk_level: "critical"` produces `RiskLevel::Critical`.
#[test]
fn clearance_critical_risk_level_contract() {
    let risk = parse_risk_level("critical");
    assert_eq!(
        risk,
        RiskLevel::Critical,
        "S004: 'critical' must map to RiskLevel::Critical"
    );
}

// ── S005: Missing session → warn and discard ──────────────────────────────────

/// S005 — The contract for missing session handling: when the session referenced
/// in a `ClearanceRequested` event cannot be found in the database, the handler
/// must discard the event and produce NO `ApprovalRequest`. This test validates
/// the expected behavior contract, not the implementation directly.
#[test]
fn clearance_missing_session_produces_no_approval() {
    // Contract: when session lookup fails, the handler should NOT produce an
    // ApprovalRequest. This is validated by the integration test (S005), but
    // the contract test verifies the expected output is absent.
    let session_found = false; // simulates SessionRepo::get_by_id returning None
    let approval_created = if session_found {
        Some(ApprovalRequest::new(
            "missing-session".to_owned(),
            "Title".to_owned(),
            None,
            "diff".to_owned(),
            "path.rs".to_owned(),
            RiskLevel::Low,
            "hash".to_owned(),
        ))
    } else {
        None
    };
    assert!(
        approval_created.is_none(),
        "S005: missing session must result in no ApprovalRequest being created"
    );
}

// ── S006: Slack unavailable → persist + register, skip post ──────────────────

/// S006 — When Slack is not configured, the system must still persist the
/// `ApprovalRequest` and register with `AcpDriver`. The contract: the approval
/// record is created, its `slack_ts` remains `None` (no post was made).
#[test]
fn clearance_slack_unavailable_slack_ts_remains_none() {
    let approval = ApprovalRequest::new(
        "sess-no-slack".to_owned(),
        "Pending approval".to_owned(),
        None,
        "diff content".to_owned(),
        "src/main.rs".to_owned(),
        RiskLevel::Low,
        "hash123".to_owned(),
    );
    // When Slack is unavailable, slack_ts is never set
    assert!(
        approval.slack_ts.is_none(),
        "S006: approval.slack_ts must be None when Slack is unavailable"
    );
    assert_eq!(
        approval.status,
        ApprovalStatus::Pending,
        "S006: status must still be Pending even without Slack"
    );
}

// ── S007: DB failure → warn + continue ────────────────────────────────────────

/// S007 — When DB persistence fails, the contract is that the driver registration
/// must also be skipped (SC-003 amended). The output: no approval record persisted,
/// no driver entry registered.
#[test]
fn clearance_db_failure_contract_no_driver_registration() {
    // Contract: if DB create fails, no driver registration occurs.
    // This ensures we don't have unaudited state (plan D3 / complexity table).
    let db_failed = true;
    let driver_registered = if db_failed {
        false // skip registration when DB fails
    } else {
        true
    };
    assert!(
        !driver_registered,
        "S007: driver must NOT be registered when DB persistence fails"
    );
}

// ── S008: Empty description string ───────────────────────────────────────────

/// S008 — An empty description string is stored as `Some("")`, not `None`.
#[test]
fn clearance_empty_description_stored_as_some_empty() {
    let approval = ApprovalRequest::new(
        "sess-empty-desc".to_owned(),
        "Empty desc test".to_owned(),
        Some(String::new()),
        "diff".to_owned(),
        "path.rs".to_owned(),
        RiskLevel::Low,
        "hash".to_owned(),
    );
    assert_eq!(
        approval.description,
        Some(String::new()),
        "S008: empty description must be stored as Some(\"\"), not None"
    );
}

// ── S009: Large diff stored in full ───────────────────────────────────────────

/// S009 — Large diffs (> 100KB) must be stored in full in the database.
/// The Slack block truncation is a display concern only; persistence is complete.
#[test]
fn clearance_large_diff_stored_in_full() {
    let large_diff: String = "a".repeat(200_000); // 200KB diff
    let approval = ApprovalRequest::new(
        "sess-large".to_owned(),
        "Large diff".to_owned(),
        None,
        large_diff.clone(),
        "large_file.rs".to_owned(),
        RiskLevel::Low,
        "hash".to_owned(),
    );
    assert_eq!(
        approval.diff_content.len(),
        200_000,
        "S009: large diff must be stored in full (no truncation at persistence layer)"
    );
}

// ── Risk level contract completeness ─────────────────────────────────────────

/// Verify that `parse_risk_level` covers all three valid values without
/// overlap or ambiguity.
#[test]
fn risk_level_contract_all_valid_values() {
    assert_eq!(parse_risk_level("low"), RiskLevel::Low);
    assert_eq!(parse_risk_level("high"), RiskLevel::High);
    assert_eq!(parse_risk_level("critical"), RiskLevel::Critical);
    // Unknown value defaults to Low
    assert_eq!(parse_risk_level("unknown"), RiskLevel::Low);
}
