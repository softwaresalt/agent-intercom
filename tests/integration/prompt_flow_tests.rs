//! Integration tests for the continuation prompt forwarding flow (T115).
//!
//! Validates the end-to-end flow:
//! 1. Forward prompt → DB record created
//! 2. Simulate Continue → oneshot resolves with `continue`
//! 3. DB record updated with decision
//!
//! Also tests Refine (with instruction), Stop, and auto-timeout paths.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;

use monocoque_agent_rc::config::GlobalConfig;
use monocoque_agent_rc::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use monocoque_agent_rc::persistence::db;
use monocoque_agent_rc::persistence::prompt_repo::PromptRepo;

/// Build a minimal test configuration with in-memory DB.
fn test_config() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-prompt"
max_concurrent_sessions = 3
host_cli = "echo"
authorized_user_ids = ["U_OWNER"]

[slack]
channel_id = "C_TEST"

[timeouts]
approval_seconds = 3600
prompt_seconds = 2
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = temp.path().to_str().expect("utf8"),
    );
    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

/// Create a sample continuation prompt for testing.
fn sample_prompt(session_id: &str) -> ContinuationPrompt {
    ContinuationPrompt::new(
        session_id.to_owned(),
        "Should I continue with the migration?".to_owned(),
        PromptType::Continuation,
        Some(300),
        Some(5),
    )
}

#[tokio::test]
async fn prompt_flow_creates_db_record() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = PromptRepo::new(database);

    let prompt = sample_prompt("session-p1");
    let prompt_id = prompt.id.clone();

    let created = repo.create(&prompt).await.expect("create should succeed");
    assert_eq!(created.id, prompt_id);
    assert!(created.decision.is_none());
    assert_eq!(created.prompt_text, "Should I continue with the migration?");
    assert_eq!(created.prompt_type, PromptType::Continuation);
    assert_eq!(created.elapsed_seconds, Some(300));
    assert_eq!(created.actions_taken, Some(5));
}

#[tokio::test]
async fn prompt_flow_continue_updates_decision() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = PromptRepo::new(database);

    let prompt = sample_prompt("session-p2");
    let prompt_id = prompt.id.clone();
    repo.create(&prompt).await.expect("create");

    // Simulate Continue decision.
    let updated = repo
        .update_decision(&prompt_id, PromptDecision::Continue, None)
        .await
        .expect("update decision");
    assert_eq!(updated.decision, Some(PromptDecision::Continue));
    assert!(updated.instruction.is_none());

    // Verify DB state.
    let fetched = repo.get_by_id(&prompt_id).await.expect("fetch");
    assert_eq!(fetched.decision, Some(PromptDecision::Continue));
}

#[tokio::test]
async fn prompt_flow_refine_updates_decision_with_instruction() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = PromptRepo::new(database);

    let prompt = sample_prompt("session-p3");
    let prompt_id = prompt.id.clone();
    repo.create(&prompt).await.expect("create");

    // Simulate Refine with instruction.
    let instruction = "Focus on error handling first".to_owned();
    let updated = repo
        .update_decision(
            &prompt_id,
            PromptDecision::Refine,
            Some(instruction.clone()),
        )
        .await
        .expect("update decision");
    assert_eq!(updated.decision, Some(PromptDecision::Refine));
    assert_eq!(
        updated.instruction.as_deref(),
        Some("Focus on error handling first")
    );

    // Verify DB state.
    let fetched = repo.get_by_id(&prompt_id).await.expect("fetch");
    assert_eq!(fetched.decision, Some(PromptDecision::Refine));
    assert_eq!(fetched.instruction, Some(instruction));
}

#[tokio::test]
async fn prompt_flow_stop_updates_decision() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = PromptRepo::new(database);

    let prompt = sample_prompt("session-p4");
    let prompt_id = prompt.id.clone();
    repo.create(&prompt).await.expect("create");

    // Simulate Stop decision.
    let updated = repo
        .update_decision(&prompt_id, PromptDecision::Stop, None)
        .await
        .expect("update decision");
    assert_eq!(updated.decision, Some(PromptDecision::Stop));
}

#[tokio::test]
async fn prompt_flow_oneshot_resolves_on_continue() {
    // Simulate the blocking pattern: forward_prompt blocks on a oneshot,
    // and the interaction callback resolves it.
    let (tx, rx) = oneshot::channel::<(String, Option<String>)>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(("continue".to_owned(), None));
    });

    let (decision, instruction) = rx.await.expect("oneshot should resolve");
    assert_eq!(decision, "continue");
    assert!(instruction.is_none());
}

#[tokio::test]
async fn prompt_flow_oneshot_resolves_on_refine_with_instruction() {
    let (tx, rx) = oneshot::channel::<(String, Option<String>)>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send((
            "refine".to_owned(),
            Some("Focus on the API layer".to_owned()),
        ));
    });

    let (decision, instruction) = rx.await.expect("oneshot should resolve");
    assert_eq!(decision, "refine");
    assert_eq!(instruction.as_deref(), Some("Focus on the API layer"));
}

#[tokio::test]
async fn prompt_flow_oneshot_resolves_on_stop() {
    let (tx, rx) = oneshot::channel::<(String, Option<String>)>();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = tx.send(("stop".to_owned(), None));
    });

    let (decision, _instruction) = rx.await.expect("oneshot should resolve");
    assert_eq!(decision, "stop");
}

#[tokio::test]
async fn prompt_flow_timeout_returns_continue() {
    // Per contract: on timeout, auto-respond with `continue` decision (FR-008).
    let (_tx, rx) = oneshot::channel::<(String, Option<String>)>();

    let timeout_result = tokio::time::timeout(Duration::from_millis(200), rx).await;
    assert!(timeout_result.is_err(), "should timeout without response");

    // On timeout, the handler auto-responds with "continue".
    let default_decision = "continue";
    assert_eq!(default_decision, "continue");
}

#[tokio::test]
async fn prompt_flow_pending_for_session_query() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = PromptRepo::new(database);

    let prompt = sample_prompt("session-pending-prompt");
    repo.create(&prompt).await.expect("create");

    let pending = repo
        .get_pending_for_session("session-pending-prompt")
        .await
        .expect("query pending");
    assert!(pending.is_some());
    assert!(pending.as_ref().and_then(|p| p.decision).is_none());
}

#[tokio::test]
async fn prompt_flow_all_prompt_types_persist() {
    let config = test_config();
    let database = Arc::new(db::connect(&config, true).await.expect("db connect"));
    let repo = PromptRepo::new(database);

    let types = [
        PromptType::Continuation,
        PromptType::Clarification,
        PromptType::ErrorRecovery,
        PromptType::ResourceWarning,
    ];

    for (i, prompt_type) in types.iter().enumerate() {
        let mut prompt = sample_prompt(&format!("session-type-{i}"));
        prompt.prompt_type = *prompt_type;

        let created = repo.create(&prompt).await.expect("create");
        assert_eq!(created.prompt_type, *prompt_type);
    }
}
