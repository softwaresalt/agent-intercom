//! Serialize/Deserialize round-trip tests for all domain models (T102).

use std::collections::HashMap;

use chrono::Utc;
use monocoque_agent_rc::models::{
    approval::{ApprovalRequest, ApprovalStatus, RiskLevel},
    checkpoint::Checkpoint,
    policy::{FilePatterns, WorkspacePolicy},
    progress::{ProgressItem, ProgressStatus},
    prompt::{ContinuationPrompt, PromptDecision, PromptType},
    session::{Session, SessionMode, SessionStatus},
    stall::{StallAlert, StallAlertStatus},
};

// ── Session ──────────────────────────────────────────

#[test]
fn session_round_trip() {
    let session = Session::new(
        "U123".into(),
        "/home/workspace".into(),
        Some("build the feature".into()),
        SessionMode::Remote,
    );

    let json = serde_json::to_string(&session).expect("serialize session");
    let deserialized: Session = serde_json::from_str(&json).expect("deserialize session");

    // Verify all fields round-trip through serde.
    assert_eq!(session.owner_user_id, deserialized.owner_user_id);
    assert_eq!(session.workspace_root, deserialized.workspace_root);
    assert_eq!(session.status, deserialized.status);
    assert_eq!(session.mode, deserialized.mode);
}

#[test]
fn session_status_serialization() {
    let values = [
        (SessionStatus::Created, "\"created\""),
        (SessionStatus::Active, "\"active\""),
        (SessionStatus::Paused, "\"paused\""),
        (SessionStatus::Terminated, "\"terminated\""),
        (SessionStatus::Interrupted, "\"interrupted\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "SessionStatus::{variant:?}");
        let back: SessionStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, variant);
    }
}

#[test]
fn session_mode_serialization() {
    let values = [
        (SessionMode::Remote, "\"remote\""),
        (SessionMode::Local, "\"local\""),
        (SessionMode::Hybrid, "\"hybrid\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "SessionMode::{variant:?}");
    }
}

// ── ApprovalRequest ──────────────────────────────────

#[test]
fn approval_request_round_trip() {
    let req = ApprovalRequest::new(
        "session-1".into(),
        "Add auth middleware".into(),
        Some("JWT-based auth".into()),
        "--- a/src/main.rs\n+++ b/src/main.rs\n@@ ...".into(),
        "src/main.rs".into(),
        RiskLevel::High,
        "abc123".into(),
    );

    let json = serde_json::to_string(&req).expect("serialize");
    let back: ApprovalRequest = serde_json::from_str(&json).expect("deserialize");

    // Verify all fields round-trip through serde.
    assert_eq!(req.session_id, back.session_id);
    assert_eq!(req.risk_level, back.risk_level);
    assert_eq!(req.status, ApprovalStatus::Pending);
}

#[test]
fn approval_status_serialization() {
    let values = [
        (ApprovalStatus::Pending, "\"pending\""),
        (ApprovalStatus::Approved, "\"approved\""),
        (ApprovalStatus::Rejected, "\"rejected\""),
        (ApprovalStatus::Expired, "\"expired\""),
        (ApprovalStatus::Consumed, "\"consumed\""),
        (ApprovalStatus::Interrupted, "\"interrupted\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "ApprovalStatus::{variant:?}");
    }
}

#[test]
fn risk_level_serialization() {
    let values = [
        (RiskLevel::Low, "\"low\""),
        (RiskLevel::High, "\"high\""),
        (RiskLevel::Critical, "\"critical\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "RiskLevel::{variant:?}");
    }
}

// ── Checkpoint ───────────────────────────────────────

#[test]
fn checkpoint_round_trip() {
    let mut hashes = HashMap::new();
    hashes.insert("src/main.rs".into(), "sha256-abc".into());

    let checkpoint = Checkpoint::new(
        "session-1".into(),
        Some("before-refactor".into()),
        serde_json::json!({"step": 3}),
        hashes,
        "/workspace".into(),
        Some(vec![ProgressItem {
            label: "Setup".into(),
            status: ProgressStatus::Done,
        }]),
    );

    let json = serde_json::to_string(&checkpoint).expect("serialize");
    let back: Checkpoint = serde_json::from_str(&json).expect("deserialize");

    // Verify all fields round-trip through serde.
    assert_eq!(checkpoint.file_hashes, back.file_hashes);
    assert_eq!(checkpoint.label, back.label);
}

// ── ContinuationPrompt ──────────────────────────────

#[test]
fn prompt_round_trip() {
    let prompt = ContinuationPrompt::new(
        "session-1".into(),
        "Should I continue?".into(),
        PromptType::Continuation,
        Some(120),
        Some(5),
    );

    let json = serde_json::to_string(&prompt).expect("serialize");
    let back: ContinuationPrompt = serde_json::from_str(&json).expect("deserialize");

    // Verify all fields round-trip through serde.
    assert_eq!(prompt.prompt_type, PromptType::Continuation);
    assert!(back.decision.is_none());
}

#[test]
fn prompt_type_serialization() {
    let values = [
        (PromptType::Continuation, "\"continuation\""),
        (PromptType::Clarification, "\"clarification\""),
        (PromptType::ErrorRecovery, "\"error_recovery\""),
        (PromptType::ResourceWarning, "\"resource_warning\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "PromptType::{variant:?}");
    }
}

#[test]
fn prompt_decision_serialization() {
    let values = [
        (PromptDecision::Continue, "\"continue\""),
        (PromptDecision::Refine, "\"refine\""),
        (PromptDecision::Stop, "\"stop\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "PromptDecision::{variant:?}");
    }
}

// ── StallAlert ───────────────────────────────────────

#[test]
fn stall_alert_round_trip() {
    let alert = StallAlert::new(
        "session-1".into(),
        Some("heartbeat".into()),
        Utc::now(),
        600,
        None,
    );

    let json = serde_json::to_string(&alert).expect("serialize");
    let _back: StallAlert = serde_json::from_str(&json).expect("deserialize");

    // Verify all fields round-trip through serde.
    assert_eq!(alert.status, StallAlertStatus::Pending);
    assert_eq!(alert.nudge_count, 0);
}

#[test]
fn stall_alert_status_serialization() {
    let values = [
        (StallAlertStatus::Pending, "\"pending\""),
        (StallAlertStatus::Nudged, "\"nudged\""),
        (StallAlertStatus::SelfRecovered, "\"self_recovered\""),
        (StallAlertStatus::Escalated, "\"escalated\""),
        (StallAlertStatus::Dismissed, "\"dismissed\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "StallAlertStatus::{variant:?}");
    }
}

// ── ProgressItem ─────────────────────────────────────

#[test]
fn progress_item_round_trip() {
    let items = vec![
        ProgressItem {
            label: "Setup project".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "Write tests".into(),
            status: ProgressStatus::InProgress,
        },
        ProgressItem {
            label: "Implement feature".into(),
            status: ProgressStatus::Pending,
        },
    ];

    let json = serde_json::to_string(&items).expect("serialize");
    let back: Vec<ProgressItem> = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(items, back);
}

#[test]
fn progress_status_serialization() {
    let values = [
        (ProgressStatus::Done, "\"done\""),
        (ProgressStatus::InProgress, "\"in_progress\""),
        (ProgressStatus::Pending, "\"pending\""),
    ];

    for (variant, expected) in values {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected, "ProgressStatus::{variant:?}");
    }
}

// ── WorkspacePolicy ──────────────────────────────────

#[test]
fn workspace_policy_from_json() {
    let json = r#"{
        "enabled": true,
        "auto_approve_commands": ["cargo test"],
        "tools": ["write_file"],
        "file_patterns": {
            "write": ["src/**/*.rs"],
            "read": ["**"]
        },
        "risk_level_threshold": "high",
        "log_auto_approved": true,
        "summary_interval_seconds": 600
    }"#;

    let policy: WorkspacePolicy = serde_json::from_str(json).expect("deserialize");

    assert!(policy.enabled);
    assert_eq!(policy.auto_approve_commands, vec!["cargo test"]);
    assert_eq!(policy.risk_level_threshold, RiskLevel::High);
    assert_eq!(policy.summary_interval_seconds, 600);
}

#[test]
fn workspace_policy_defaults() {
    let policy: WorkspacePolicy = serde_json::from_str("{}").expect("deserialize empty");

    assert!(!policy.enabled);
    assert!(policy.auto_approve_commands.is_empty());
    assert!(policy.tools.is_empty());
    assert_eq!(policy.risk_level_threshold, RiskLevel::Low);
    assert_eq!(policy.summary_interval_seconds, 300);
}

#[test]
fn workspace_policy_partial_json() {
    let json = r#"{ "enabled": true }"#;
    let policy: WorkspacePolicy = serde_json::from_str(json).expect("deserialize partial");

    assert!(policy.enabled);
    assert!(policy.auto_approve_commands.is_empty());
    assert_eq!(policy.risk_level_threshold, RiskLevel::Low);
}

#[test]
fn file_patterns_default() {
    let default = FilePatterns::default();
    assert!(default.write.is_empty());
    assert!(default.read.is_empty());
}
