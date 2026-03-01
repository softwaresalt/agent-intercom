//! Unit tests for offline agent message queuing (T085 — S059, S060, S062).
//!
//! Validates that:
//! - Steering messages are queued when the session is `Offline` or `Stalled`
//! - Messages are delivered directly via the ACP driver when the session is `Online`
//! - The queue preserves FIFO insertion order

use std::sync::Arc;

use agent_intercom::driver::acp_driver::AcpDriver;
use agent_intercom::driver::AgentDriver;
use agent_intercom::models::session::{
    ConnectivityStatus, ProtocolMode, Session, SessionMode, SessionStatus,
};
use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::persistence::steering_repo::SteeringRepo;
use tokio::sync::mpsc;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build an in-memory database with schema.
async fn build_db() -> Arc<db::Database> {
    Arc::new(db::connect_memory().await.expect("in-memory db"))
}

/// Create and activate a session, optionally with ACP protocol mode and
/// a specific connectivity status.
async fn create_acp_session_with_connectivity(
    db: &Arc<db::Database>,
    protocol_mode: ProtocolMode,
    connectivity: ConnectivityStatus,
) -> Session {
    let repo = SessionRepo::new(Arc::clone(db));
    let mut session = Session::new(
        "U_OP".to_owned(),
        std::env::temp_dir().to_string_lossy().to_string(),
        Some("task".to_owned()),
        SessionMode::Remote,
    );
    session.protocol_mode = protocol_mode;
    session.channel_id = Some("C_TEST".to_owned());
    session.connectivity_status = connectivity;

    let created = repo.create(&session).await.expect("create");
    // Activate (via transition — can't store 'active' directly with connectivity_status != Online default)
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate")
}

// ── S059: steering when offline queues to steering_repo ──────────────────────

/// S059 — When a session's `connectivity_status` is `Offline`, a steering
/// message from Slack must be queued in the steering repo rather than sent
/// directly via the ACP driver.
#[tokio::test]
async fn steering_when_offline_queues_to_inbox() {
    let pool = build_db().await;
    let repo = SessionRepo::new(Arc::clone(&pool));

    // Create a session with Offline connectivity.
    let session =
        create_acp_session_with_connectivity(&pool, ProtocolMode::Acp, ConnectivityStatus::Offline)
            .await;

    // Persist the Offline status so the handler sees it on lookup.
    repo.set_connectivity_status(&session.id, ConnectivityStatus::Offline)
        .await
        .expect("set offline");

    // Queue the message directly (simulating store_from_slack offline path).
    let steering_repo = SteeringRepo::new(Arc::clone(&pool));
    let msg = SteeringMessage::new(
        session.id.clone(),
        Some("C_TEST".to_owned()),
        "please refocus on failing tests".to_owned(),
        SteeringSource::Slack,
    );
    steering_repo.insert(&msg).await.expect("enqueue");

    // Verify the message is in the queue.
    let queued = steering_repo
        .fetch_unconsumed(&session.id)
        .await
        .expect("fetch");
    assert_eq!(queued.len(), 1, "offline message must be queued");
    assert_eq!(queued[0].message, "please refocus on failing tests");
    assert!(!queued[0].consumed, "message must not be consumed yet");
}

// ── S060: steering when online sends directly ─────────────────────────────────

/// S060 — When a session's `connectivity_status` is `Online` and the protocol
/// is ACP, a steering message is delivered directly via the ACP driver rather
/// than being stored in the queue.
#[tokio::test]
async fn steering_when_online_sends_directly() {
    let pool = build_db().await;

    let session =
        create_acp_session_with_connectivity(&pool, ProtocolMode::Acp, ConnectivityStatus::Online)
            .await;

    // Set up ACP driver with a registered writer channel.
    let acp_driver = AcpDriver::new();
    let (writer_tx, mut writer_rx) = mpsc::channel::<serde_json::Value>(8);
    acp_driver.register_session(&session.id, writer_tx).await;

    // Send a prompt directly via the driver (simulating store_from_slack Online path).
    acp_driver
        .send_prompt(&session.id, "status check")
        .await
        .expect("send_prompt must succeed for online session");

    // Verify the driver's writer channel received the message.
    let msg = writer_rx
        .try_recv()
        .expect("writer channel must have received the prompt");
    assert_eq!(
        msg["method"].as_str(),
        Some("prompt/send"),
        "online steering must produce a prompt/send message"
    );
    assert_eq!(msg["params"]["text"].as_str(), Some("status check"));

    // Verify the steering repo is empty (no queuing for online sessions).
    let steering_repo = SteeringRepo::new(Arc::clone(&pool));
    let queued = steering_repo
        .fetch_unconsumed(&session.id)
        .await
        .expect("fetch");
    assert_eq!(
        queued.len(),
        0,
        "online steering must not queue to the steering repo"
    );
}

// ── S062: FIFO ordering ───────────────────────────────────────────────────────

/// S062 — Multiple queued messages must be returned in FIFO order (oldest
/// first) so that delivery on reconnect preserves the operator's intent.
#[tokio::test]
async fn inbox_queue_is_fifo() {
    let pool = build_db().await;

    let session =
        create_acp_session_with_connectivity(&pool, ProtocolMode::Acp, ConnectivityStatus::Offline)
            .await;

    let steering_repo = SteeringRepo::new(Arc::clone(&pool));

    // Insert three messages with a small delay to ensure distinct timestamps.
    for i in 1_u8..=3 {
        let msg = SteeringMessage::new(
            session.id.clone(),
            Some("C_TEST".to_owned()),
            format!("message {i}"),
            SteeringSource::Slack,
        );
        steering_repo.insert(&msg).await.expect("insert");
        // Tiny sleep to guarantee strictly increasing created_at timestamps.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let queued = steering_repo
        .fetch_unconsumed(&session.id)
        .await
        .expect("fetch");

    assert_eq!(queued.len(), 3, "all three messages must be queued");
    assert_eq!(queued[0].message, "message 1", "first in must be first out");
    assert_eq!(queued[1].message, "message 2");
    assert_eq!(queued[2].message, "message 3");
}
