//! Integration tests for the `check_auto_approve` tool handler logic.
//!
//! Validates:
//! - Workspace with `.intercom/settings.json` policy file → correct evaluation
//! - Missing policy file → deny-all (`auto_approved`: false)
//! - Policy matches tool name → `auto_approved`: true
//! - Policy with `risk_level` threshold enforcement
//! - Policy `file_patterns` glob matching
//! - No active session → error

use std::sync::Arc;

use agent_intercom::models::policy::CompiledWorkspacePolicy;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::policy::evaluator::{AutoApproveContext, PolicyEvaluator};
use agent_intercom::policy::loader::PolicyLoader;

use super::test_helpers::{create_active_session, test_app_state, test_config};

// ── Auto-approve: missing policy file → deny-all ─────────────

#[tokio::test]
async fn auto_approve_missing_policy_denies_all() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let workspace_root = std::path::PathBuf::from(&session.workspace_root);

    let policy = PolicyLoader::load(&workspace_root).expect("load policy");

    // Default (deny-all) policy.
    assert!(!policy.raw.enabled, "missing policy should not be enabled");
}

// ── Auto-approve: disabled policy → deny-all ─────────────────

#[tokio::test]
async fn auto_approve_disabled_policy_denies() {
    let policy = CompiledWorkspacePolicy::deny_all();

    let result = PolicyEvaluator::check("any_tool", &None, &policy);
    assert!(!result.auto_approved);
    assert!(result.matched_rule.is_none());
}

// ── Auto-approve: enabled policy with matching tool ──────────

#[tokio::test]
async fn auto_approve_matching_tool_approved() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    // Create .intercom/settings.json with a permissive policy.
    let agentrc_dir = root.join(".intercom");
    std::fs::create_dir_all(&agentrc_dir).expect("create dir");
    let policy_json = serde_json::json!({
        "enabled": true,
        "tools": ["heartbeat", "remote_log"],
        "auto_approve_commands": [],
        "file_patterns": { "write": [], "read": [] },
        "risk_level_threshold": "low"
    });
    std::fs::write(
        agentrc_dir.join("settings.json"),
        serde_json::to_string_pretty(&policy_json).expect("json"),
    )
    .expect("write policy");

    let policy = PolicyLoader::load(root).expect("load policy");
    assert!(policy.raw.enabled);

    let result = PolicyEvaluator::check("heartbeat", &None, &policy);
    assert!(result.auto_approved, "heartbeat should be auto-approved");
    assert!(result.matched_rule.is_some());
}

// ── Auto-approve: enabled policy with non-matching tool ──────

#[tokio::test]
async fn auto_approve_non_matching_tool_denied() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let agentrc_dir = root.join(".intercom");
    std::fs::create_dir_all(&agentrc_dir).expect("create dir");
    let policy_json = serde_json::json!({
        "enabled": true,
        "tools": ["heartbeat"],
        "auto_approve_commands": [],
        "file_patterns": { "write": [], "read": [] },
        "risk_level_threshold": "low"
    });
    std::fs::write(
        agentrc_dir.join("settings.json"),
        serde_json::to_string_pretty(&policy_json).expect("json"),
    )
    .expect("write policy");

    let policy = PolicyLoader::load(root).expect("load policy");

    let result = PolicyEvaluator::check("ask_approval", &None, &policy);
    assert!(
        !result.auto_approved,
        "ask_approval should not be auto-approved"
    );
}

// ── Auto-approve: risk level threshold enforcement ───────────

#[tokio::test]
async fn auto_approve_risk_level_blocks_high_risk() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let agentrc_dir = root.join(".intercom");
    std::fs::create_dir_all(&agentrc_dir).expect("create dir");
    let policy_json = serde_json::json!({
        "enabled": true,
        "tools": ["ask_approval"],
        "auto_approve_commands": [],
        "file_patterns": { "write": ["**/*.rs"], "read": [] },
        "risk_level_threshold": "low"
    });
    std::fs::write(
        agentrc_dir.join("settings.json"),
        serde_json::to_string_pretty(&policy_json).expect("json"),
    )
    .expect("write policy");

    let policy = PolicyLoader::load(root).expect("load policy");

    // High risk context should be blocked by low threshold.
    let ctx = AutoApproveContext {
        file_path: Some("src/main.rs".into()),
        risk_level: Some("high".into()),
    };
    let result = PolicyEvaluator::check("ask_approval", &Some(ctx), &policy);
    assert!(
        !result.auto_approved,
        "high risk should be blocked by low threshold"
    );
}

// ── Auto-approve: file pattern glob matching ─────────────────

#[tokio::test]
async fn auto_approve_file_pattern_match() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let agentrc_dir = root.join(".intercom");
    std::fs::create_dir_all(&agentrc_dir).expect("create dir");
    let policy_json = serde_json::json!({
        "enabled": true,
        "tools": ["ask_approval"],
        "auto_approve_commands": [],
        "file_patterns": { "write": ["src/**/*.rs"], "read": [] },
        "risk_level_threshold": "low"
    });
    std::fs::write(
        agentrc_dir.join("settings.json"),
        serde_json::to_string_pretty(&policy_json).expect("json"),
    )
    .expect("write policy");

    let policy = PolicyLoader::load(root).expect("load policy");

    // File matching the write pattern.
    let ctx = AutoApproveContext {
        file_path: Some("src/main.rs".into()),
        risk_level: Some("low".into()),
    };
    let result = PolicyEvaluator::check("ask_approval", &Some(ctx), &policy);
    assert!(
        result.auto_approved,
        "file matching pattern should be approved"
    );
}

// ── Auto-approve: malformed policy file → deny-all ───────────

#[tokio::test]
async fn auto_approve_malformed_policy_denies_all() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    let agentrc_dir = root.join(".intercom");
    std::fs::create_dir_all(&agentrc_dir).expect("create dir");
    std::fs::write(agentrc_dir.join("settings.json"), "not valid json {{{").expect("write policy");

    let policy = PolicyLoader::load(root).expect("load policy");
    assert!(
        !policy.raw.enabled,
        "malformed policy should degrade to deny-all"
    );
}

// ── Auto-approve: session workspace_root used ────────────────

#[tokio::test]
async fn auto_approve_uses_session_workspace_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;

    // The handler resolves workspace_root from the session, not config.
    let workspace_from_session = std::path::PathBuf::from(&session.workspace_root);
    assert!(
        workspace_from_session.exists(),
        "workspace root from session should exist"
    );
}

// ── Auto-approve: no active session detected ─────────────────

#[tokio::test]
async fn auto_approve_no_active_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let sessions = repo.list_active().await.expect("list active");
    assert!(sessions.is_empty());
}
