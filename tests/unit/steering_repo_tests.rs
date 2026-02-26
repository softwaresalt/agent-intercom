//! Unit tests for `SteeringRepo` CRUD operations (T014, T015).
//!
//! Covers scenarios S001-S005, S007, S010-S011:
//! - Insert and retrieve unconsumed messages
//! - `mark_consumed` marks delivered messages
//! - Channel-scoped routing: messages route to correct session
//! - Boundary: empty message, long message

use std::sync::Arc;

use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
use agent_intercom::persistence::{db, steering_repo::SteeringRepo};

fn sample_msg(session_id: &str, channel_id: Option<&str>, text: &str) -> SteeringMessage {
    SteeringMessage::new(
        session_id.to_owned(),
        channel_id.map(str::to_owned),
        text.to_owned(),
        SteeringSource::Slack,
    )
}

// ─── S001 / S004: insert stores message ──────────────────────────────

#[tokio::test]
async fn insert_persists_all_fields() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let msg = sample_msg("sess-1", Some("C1"), "refocus on tests");
    let saved = repo.insert(&msg).await.expect("insert");

    assert_eq!(saved.session_id, "sess-1");
    assert_eq!(saved.channel_id, Some("C1".to_owned()));
    assert_eq!(saved.message, "refocus on tests");
    assert_eq!(saved.source, SteeringSource::Slack);
    assert!(!saved.consumed);
}

// ─── S002: fetch_unconsumed returns only unconsumed messages ─────────

#[tokio::test]
async fn fetch_unconsumed_returns_pending_messages() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let m1 = sample_msg("sess-2", None, "first message");
    let m2 = sample_msg("sess-2", None, "second message");
    repo.insert(&m1).await.expect("insert m1");
    repo.insert(&m2).await.expect("insert m2");

    let msgs = repo.fetch_unconsumed("sess-2").await.expect("fetch");
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].message, "first message");
    assert_eq!(msgs[1].message, "second message");
}

// ─── S003: fetch_unconsumed returns nothing when all consumed ────────

#[tokio::test]
async fn fetch_unconsumed_excludes_consumed_messages() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let msg = sample_msg("sess-3", None, "consumed already");
    let saved = repo.insert(&msg).await.expect("insert");
    repo.mark_consumed(&saved.id).await.expect("consume");

    let msgs = repo.fetch_unconsumed("sess-3").await.expect("fetch");
    assert!(msgs.is_empty());
}

// ─── S005: IPC source stored correctly ───────────────────────────────

#[tokio::test]
async fn insert_ipc_source_stored() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let msg = SteeringMessage::new(
        "sess-4".to_owned(),
        None,
        "refocus".to_owned(),
        SteeringSource::Ipc,
    );
    let saved = repo.insert(&msg).await.expect("insert");
    assert_eq!(saved.source, SteeringSource::Ipc);
}

// ─── S007: channel routing — messages scoped to correct session ──────

#[tokio::test]
async fn fetch_unconsumed_scoped_to_session() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let m_c1 = sample_msg("sess-c1", Some("C1"), "for session C1");
    let m_c2 = sample_msg("sess-c2", Some("C2"), "for session C2");
    repo.insert(&m_c1).await.expect("insert C1");
    repo.insert(&m_c2).await.expect("insert C2");

    let c1_msgs = repo.fetch_unconsumed("sess-c1").await.expect("fetch C1");
    let c2_msgs = repo.fetch_unconsumed("sess-c2").await.expect("fetch C2");

    assert_eq!(c1_msgs.len(), 1);
    assert_eq!(c1_msgs[0].message, "for session C1");
    assert_eq!(c2_msgs.len(), 1);
    assert_eq!(c2_msgs[0].message, "for session C2");
}

// ─── S008: message stays unconsumed for terminated session ───────────

#[tokio::test]
async fn unconsumed_messages_persist_after_session_ends() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let msg = sample_msg("sess-term", None, "for terminated session");
    let saved = repo.insert(&msg).await.expect("insert");

    // Message NOT consumed — simulates session terminated before ping
    let msgs = repo.fetch_unconsumed("sess-term").await.expect("fetch");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, saved.id);
    assert!(!msgs[0].consumed);
}

// ─── S010: empty message text boundary ───────────────────────────────

#[tokio::test]
async fn insert_empty_message_is_stored() {
    // The repo itself does not enforce non-empty; validation is at the command layer.
    // This test documents the repo-level behaviour (no error on empty string).
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let msg = sample_msg("sess-empty", None, "");
    let saved = repo.insert(&msg).await.expect("insert");
    assert_eq!(saved.message, "");
}

// ─── S011: very long message stored intact ───────────────────────────

#[tokio::test]
async fn insert_very_long_message_stored_intact() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let long_msg = "x".repeat(10_000);
    let msg = sample_msg("sess-long", None, &long_msg);
    let saved = repo.insert(&msg).await.expect("insert");
    assert_eq!(saved.message.len(), 10_000);
}

// ─── purge removes old rows ───────────────────────────────────────────

#[tokio::test]
async fn purge_removes_old_messages() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let msg = sample_msg("sess-purge", None, "old message");
    repo.insert(&msg).await.expect("insert");

    let cutoff = chrono::Utc::now() + chrono::Duration::seconds(10);
    let deleted = repo.purge(cutoff).await.expect("purge");
    assert_eq!(deleted, 1);

    let remaining = repo.fetch_unconsumed("sess-purge").await.expect("fetch");
    assert!(remaining.is_empty());
}
