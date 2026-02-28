//! Integration tests for end-to-end operator steering flow (T017).
//!
//! Covers scenarios S001-S009:
//! - Steering messages stored via repo (S001, S004, S005)
//! - Unconsumed messages fetched for heartbeat delivery (S002, S003)
//! - Messages marked consumed after delivery (S002)
//! - Channel-scoped routing (S007)
//! - Concurrent messages stored in arrival order (S009)
//! - Terminated-session messages remain unconsumed (S008)

use std::sync::Arc;

use agent_intercom::models::session::SessionStatus;
use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::persistence::steering_repo::SteeringRepo;

use super::test_helpers::{create_active_session, test_app_state, test_config};

// ── S001: steering message stored from Slack ────────────────────────────

#[tokio::test]
async fn steering_message_stored_from_slack() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    let msg = SteeringMessage::new(
        session.id.clone(),
        Some("C1".to_owned()),
        "refocus on tests".to_owned(),
        SteeringSource::Slack,
    );
    let saved = repo.insert(&msg).await.expect("insert");

    assert_eq!(saved.session_id, session.id);
    assert_eq!(saved.channel_id, Some("C1".to_owned()));
    assert_eq!(saved.source, SteeringSource::Slack);
    assert!(!saved.consumed);
}

// ── S002: heartbeat flow fetches and marks messages consumed ────────────

#[tokio::test]
async fn heartbeat_flow_delivers_and_marks_steering_consumed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    // Insert two steering messages
    let m1 = SteeringMessage::new(
        session.id.clone(),
        None,
        "msg1".to_owned(),
        SteeringSource::Slack,
    );
    let m2 = SteeringMessage::new(
        session.id.clone(),
        None,
        "msg2".to_owned(),
        SteeringSource::Slack,
    );
    repo.insert(&m1).await.expect("insert m1");
    repo.insert(&m2).await.expect("insert m2");

    // Simulate heartbeat delivery logic (T018 will add this to heartbeat.rs)
    let pending = repo
        .fetch_unconsumed(&session.id)
        .await
        .expect("fetch unconsumed");
    assert_eq!(
        pending.len(),
        2,
        "two messages should be pending before delivery"
    );

    let texts: Vec<&str> = pending.iter().map(|m| m.message.as_str()).collect();
    assert!(texts.contains(&"msg1"));
    assert!(texts.contains(&"msg2"));

    // Mark consumed (heartbeat delivery)
    for m in &pending {
        repo.mark_consumed(&m.id).await.expect("mark consumed");
    }

    // Verify both are now consumed
    let after = repo
        .fetch_unconsumed(&session.id)
        .await
        .expect("fetch after");
    assert!(
        after.is_empty(),
        "all messages should be consumed after delivery"
    );
}

// ── S003: no messages → empty delivery ─────────────────────────────────

#[tokio::test]
async fn heartbeat_flow_no_messages_returns_empty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    let pending = repo.fetch_unconsumed(&session.id).await.expect("fetch");
    assert!(pending.is_empty());
}

// ── S005: IPC source stored correctly ───────────────────────────────────

#[tokio::test]
async fn steering_message_via_ipc_stored() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    let msg = SteeringMessage::new(
        session.id.clone(),
        None,
        "ipc steer message".to_owned(),
        SteeringSource::Ipc,
    );
    let saved = repo.insert(&msg).await.expect("insert");
    assert_eq!(saved.source, SteeringSource::Ipc);

    let pending = repo.fetch_unconsumed(&session.id).await.expect("fetch");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].source, SteeringSource::Ipc);
}

// ── S007: channel-scoped routing ────────────────────────────────────────

#[tokio::test]
async fn steering_messages_scoped_to_owning_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    // Two sessions on different channels
    let sess_c1 = create_active_session(&state.db, root).await;
    let sess_c2 = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    let m_c1 = SteeringMessage::new(
        sess_c1.id.clone(),
        Some("C1".to_owned()),
        "for C1".to_owned(),
        SteeringSource::Slack,
    );
    let m_c2 = SteeringMessage::new(
        sess_c2.id.clone(),
        Some("C2".to_owned()),
        "for C2".to_owned(),
        SteeringSource::Slack,
    );
    repo.insert(&m_c1).await.expect("insert C1");
    repo.insert(&m_c2).await.expect("insert C2");

    let c1_msgs = repo.fetch_unconsumed(&sess_c1.id).await.expect("fetch C1");
    let c2_msgs = repo.fetch_unconsumed(&sess_c2.id).await.expect("fetch C2");

    assert_eq!(c1_msgs.len(), 1);
    assert_eq!(c1_msgs[0].message, "for C1");
    assert_eq!(c2_msgs.len(), 1);
    assert_eq!(c2_msgs[0].message, "for C2");
}

// ── S008: messages for terminated session remain unconsumed ─────────────

#[tokio::test]
async fn steering_messages_persist_for_terminated_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    let msg = SteeringMessage::new(
        session.id.clone(),
        None,
        "undelivered".to_owned(),
        SteeringSource::Slack,
    );
    repo.insert(&msg).await.expect("insert");

    // Terminate session before ping arrives — message stays unconsumed
    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    session_repo
        .update_status(&session.id, SessionStatus::Terminated)
        .await
        .expect("terminate session");

    // Message still unconsumed
    let still_pending = repo.fetch_unconsumed(&session.id).await.expect("fetch");
    assert_eq!(still_pending.len(), 1);
    assert!(!still_pending[0].consumed);
}

// ── S009: concurrent messages stored in arrival order ───────────────────

#[tokio::test]
async fn steering_messages_stored_in_insertion_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SteeringRepo::new(Arc::clone(&state.db));

    for i in 0..5_u8 {
        let msg = SteeringMessage::new(
            session.id.clone(),
            None,
            format!("message {i}"),
            SteeringSource::Slack,
        );
        repo.insert(&msg).await.expect("insert");
    }

    let msgs = repo.fetch_unconsumed(&session.id).await.expect("fetch");
    assert_eq!(msgs.len(), 5);
    // Messages are ordered by created_at ASC per repo implementation
    for (i, m) in msgs.iter().enumerate() {
        assert_eq!(m.message, format!("message {i}"));
    }
}
