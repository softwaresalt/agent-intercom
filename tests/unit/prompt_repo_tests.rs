//! Unit tests for `PromptRepo` CRUD operations (T021).
//!
//! Validates:
//! - Create continuation prompt and verify all fields persisted
//! - `get_by_id` returns `None` for missing records
//! - `get_pending_for_session` returns only undecided prompts
//! - `update_decision` records decision and optional instruction
//! - `list_pending` returns all undecided prompts across sessions

use std::sync::Arc;

use monocoque_agent_rc::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use monocoque_agent_rc::persistence::{db, prompt_repo::PromptRepo};

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
