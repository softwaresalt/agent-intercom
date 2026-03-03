//! Unit tests for ACP audit event types (T139).
//!
//! Covers:
//! - T139 (S101): ACP session-start writes a correctly-typed audit entry
//! - T139 (S102): ACP session-stop writes a correctly-typed audit entry
//! - T139 (S103): All six new ACP event types serialize with `snake_case` names

use agent_intercom::audit::{AuditEntry, AuditEventType};

// ── T139 (S101): ACP session-start audit entry serializes correctly ───────────

/// `AuditEventType::AcpSessionStart` must serialize as `"acp_session_start"`
/// and an entry built from it must roundtrip through JSON serde without loss.
#[test]
fn acp_session_start_entry_serializes_correctly() {
    let entry = AuditEntry::new(AuditEventType::AcpSessionStart)
        .with_session("sess-acp-start".to_owned())
        .with_result("session started in workspace-root".to_owned());

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse json");

    assert_eq!(
        parsed["event_type"], "acp_session_start",
        "event_type must serialize as 'acp_session_start'"
    );
    assert_eq!(
        parsed["session_id"], "sess-acp-start",
        "session_id must be present"
    );
    assert_eq!(
        parsed["result_summary"], "session started in workspace-root",
        "result_summary must be present"
    );
}

// ── T139 (S102): ACP session-stop audit entry serializes correctly ────────────

/// `AuditEventType::AcpSessionStop` must serialize as `"acp_session_stop"`.
#[test]
fn acp_session_stop_entry_serializes_correctly() {
    let entry = AuditEntry::new(AuditEventType::AcpSessionStop)
        .with_session("sess-acp-stop".to_owned())
        .with_result("session stopped by operator".to_owned());

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse json");

    assert_eq!(parsed["event_type"], "acp_session_stop");
}

// ── T139 (S103): All six ACP event types use snake_case serialization ─────────

/// Every new ACP audit event type must serialize to the expected `snake_case`
/// string and deserialize back to the same variant (serde roundtrip).
#[test]
fn all_acp_event_types_serialize_with_snake_case() {
    let cases: &[(AuditEventType, &str)] = &[
        (AuditEventType::AcpSessionStart, "acp_session_start"),
        (AuditEventType::AcpSessionStop, "acp_session_stop"),
        (AuditEventType::AcpSessionPause, "acp_session_pause"),
        (AuditEventType::AcpSessionResume, "acp_session_resume"),
        (AuditEventType::AcpSteerDelivered, "acp_steer_delivered"),
        (AuditEventType::AcpTaskQueued, "acp_task_queued"),
    ];

    for (event_type, expected) in cases {
        let serialized =
            serde_json::to_string(event_type).expect("serialize AuditEventType variant");
        assert_eq!(
            serialized,
            format!("\"{expected}\""),
            "variant {event_type:?} must serialize as {expected}"
        );

        let restored: AuditEventType =
            serde_json::from_str(&serialized).expect("deserialize AuditEventType");
        assert_eq!(
            &restored, event_type,
            "deserialized variant must equal original"
        );
    }
}

// ── T139: ACP audit entry roundtrips serde ────────────────────────────────────

/// An `AuditEntry` with ACP event type and all optional fields populated must
/// survive a full serde roundtrip with field values preserved.
#[test]
fn acp_audit_entry_roundtrips_serde() {
    let original = AuditEntry::new(AuditEventType::AcpSteerDelivered)
        .with_session("sess-steer".to_owned())
        .with_operator("U-OP-001".to_owned())
        .with_result("steering message delivered".to_owned());

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: AuditEntry = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.event_type, AuditEventType::AcpSteerDelivered);
    assert_eq!(restored.session_id, Some("sess-steer".to_owned()));
    assert_eq!(restored.operator_id, Some("U-OP-001".to_owned()));
    assert_eq!(
        restored.result_summary,
        Some("steering message delivered".to_owned())
    );
}
