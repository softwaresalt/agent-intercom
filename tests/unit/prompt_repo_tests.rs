//! Unit tests for `PromptRepo` CRUD operations (T021).
//!
//! Validates:
//! - Create continuation prompt and verify all fields persisted
//! - `get_by_id` returns `None` for missing records
//! - `get_pending_for_session` returns only undecided prompts
//! - `update_decision` records decision and optional instruction
//! - `list_pending` returns all undecided prompts across sessions

use std::sync::Arc;

use agent_intercom::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use agent_intercom::persistence::{db, prompt_repo::PromptRepo};

fn sample_prompt(session_id: &str) -> ContinuationPrompt {
    ContinuationPrompt::new(
        session_id.to_owned(),
        "Should I continue?".to_owned(),
        PromptType::Continuation,
        Some(120),
        Some(3),
    )
}

#[tokio::test]
async fn create_persists_all_fields() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let prompt = sample_prompt("sess-1");
    let id = prompt.id.clone();
    let created = repo.create(&prompt).await.expect("create");

    assert_eq!(created.id, id);
    assert_eq!(created.session_id, "sess-1");
    assert_eq!(created.prompt_type, PromptType::Continuation);
    assert_eq!(created.elapsed_seconds, Some(120));
    assert_eq!(created.actions_taken, Some(3));
    assert!(created.decision.is_none());
    assert!(created.instruction.is_none());
}

#[tokio::test]
async fn get_by_id_returns_none_for_missing() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let result = repo.get_by_id("nonexistent").await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn get_by_id_round_trips() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let prompt = sample_prompt("sess-2");
    let id = prompt.id.clone();
    repo.create(&prompt).await.expect("create");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.prompt_text, "Should I continue?");
    assert_eq!(fetched.prompt_type, PromptType::Continuation);
}

#[tokio::test]
async fn update_decision_continue() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let prompt = sample_prompt("sess-3");
    let id = prompt.id.clone();
    repo.create(&prompt).await.expect("create");

    repo.update_decision(&id, PromptDecision::Continue, None)
        .await
        .expect("decide");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.decision, Some(PromptDecision::Continue));
    assert!(fetched.instruction.is_none());
}

#[tokio::test]
async fn update_decision_refine_with_instruction() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let prompt = sample_prompt("sess-4");
    let id = prompt.id.clone();
    repo.create(&prompt).await.expect("create");

    repo.update_decision(
        &id,
        PromptDecision::Refine,
        Some("Focus on tests".to_owned()),
    )
    .await
    .expect("decide");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.decision, Some(PromptDecision::Refine));
    assert_eq!(fetched.instruction.as_deref(), Some("Focus on tests"));
}

#[tokio::test]
async fn get_pending_for_session_returns_undecided() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let p1 = sample_prompt("sess-5");
    let p2 = sample_prompt("sess-5");
    let id1 = p1.id.clone();
    repo.create(&p1).await.expect("create1");
    repo.create(&p2).await.expect("create2");

    // Decide p1 — only p2 should be pending.
    repo.update_decision(&id1, PromptDecision::Stop, None)
        .await
        .expect("decide");

    let pending = repo.get_pending_for_session("sess-5").await.expect("query");
    assert!(pending.is_some());
    assert_eq!(pending.as_ref().map(|p| &p.id), Some(&p2.id));
}

#[tokio::test]
async fn list_pending_returns_all_undecided() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let p1 = sample_prompt("sess-6a");
    let p2 = sample_prompt("sess-6b");
    repo.create(&p1).await.expect("create1");
    repo.create(&p2).await.expect("create2");

    let pending = repo.list_pending().await.expect("list");
    assert_eq!(pending.len(), 2);
}

#[tokio::test]
async fn all_prompt_types_round_trip() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    let types = [
        PromptType::Continuation,
        PromptType::Clarification,
        PromptType::ErrorRecovery,
        PromptType::ResourceWarning,
    ];

    for (i, pt) in types.iter().enumerate() {
        let mut prompt = sample_prompt(&format!("sess-type-{i}"));
        prompt.prompt_type = *pt;
        let id = prompt.id.clone();
        repo.create(&prompt).await.expect("create");

        let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
        assert_eq!(fetched.prompt_type, *pt);
    }
}

// ─── F.3-T3: pending-prompt persistence + resume rebind ──────────────

/// Reassigning carries only the *undecided* prompts of a crashed session to
/// the resumed session, preserving the ACP correlation id (the prompt id).
#[tokio::test]
async fn reassign_pending_carries_prompt_to_resumed_session() {
    let db = db::connect_memory().await.expect("db");
    let repo = PromptRepo::new(Arc::new(db));

    // One pending (undecided) prompt and one decided prompt for the crashed session.
    let pending = sample_prompt("sess-crashed");
    let pending_id = pending.id.clone();
    repo.create(&pending).await.expect("create pending");

    let decided = sample_prompt("sess-crashed");
    let decided_id = decided.id.clone();
    repo.create(&decided).await.expect("create decided");
    repo.update_decision(&decided_id, PromptDecision::Continue, None)
        .await
        .expect("decide");

    let moved = repo
        .reassign_pending_to_session("sess-crashed", "sess-resumed")
        .await
        .expect("reassign");
    assert_eq!(moved, 1, "only the undecided prompt moves");

    // The crashed session no longer has a pending prompt.
    assert!(repo
        .get_pending_for_session("sess-crashed")
        .await
        .expect("fetch crashed")
        .is_none());

    // The resumed session inherits the pending prompt with the same
    // correlation id (prompt id) restored.
    let resumed = repo
        .get_pending_for_session("sess-resumed")
        .await
        .expect("fetch resumed")
        .expect("pending present");
    assert_eq!(resumed.id, pending_id, "correlation id must be preserved");
    assert!(resumed.decision.is_none());

    // The decided prompt stays with the crashed session.
    let decided_after = repo
        .get_by_id(&decided_id)
        .await
        .expect("fetch decided")
        .expect("present");
    assert_eq!(decided_after.session_id, "sess-crashed");
}

/// A pending prompt survives a full DB restart (close pool, reopen the same
/// file-backed database) with its correlation id and undecided state intact.
#[tokio::test]
async fn pending_prompt_survives_db_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("prompt-restart.db");
    let path_str = path.to_str().expect("utf8");

    let saved_id = {
        let db = db::connect(path_str).await.expect("connect");
        let repo = PromptRepo::new(Arc::new(db));
        let prompt = sample_prompt("sess-restart");
        let id = prompt.id.clone();
        repo.create(&prompt).await.expect("create");
        id
    }; // pool dropped == server shutdown

    let db2 = db::connect(path_str).await.expect("reconnect");
    let repo2 = PromptRepo::new(Arc::new(db2));
    let restored = repo2
        .get_pending_for_session("sess-restart")
        .await
        .expect("fetch after restart")
        .expect("pending present after restart");
    assert_eq!(restored.id, saved_id);
    assert!(restored.decision.is_none());
}
