//! Integration tests for ACP session lifecycle (T032, T086).
//!
//! Covers:
//! - T032 (S018): ACP session start creates session with `protocol_mode = Acp`
//! - T086 (S060, S062): Queued messages are delivered in FIFO order on reconnect

use std::sync::Arc;

use agent_intercom::acp::spawner::SpawnConfig;
use agent_intercom::models::session::{ProtocolMode, Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};

// ── T032: ACP session start creates session with protocol_mode = Acp ─────────

/// S018 — starting an ACP session persists a record with `protocol_mode = acp`.
///
/// Uses an in-memory `SQLite` database to verify the persistence contract without
/// touching the file system or spawning a real agent process.
#[tokio::test]
async fn acp_session_start_creates_session_with_acp_protocol_mode() {
    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&pool));

    // SpawnConfig import proves the spawner module exists (the test is RED until
    // src/acp/spawner.rs is created).
    let _config = SpawnConfig {
        host_cli: "echo".to_owned(),
        host_cli_args: Vec::new(),
        workspace_root: std::env::temp_dir(),
        startup_timeout: std::time::Duration::from_secs(5),
    };

    // Build a Session the way the ACP session-start handler does.
    let mut session = Session::new(
        "U_OPERATOR".to_owned(),
        std::env::temp_dir().to_string_lossy().to_string(),
        Some("implement feature X".to_owned()),
        SessionMode::Remote,
    );
    session.protocol_mode = ProtocolMode::Acp;
    session.channel_id = Some("C_TEST_CHANNEL".to_owned());

    // Persist and verify.
    let created = repo.create(&session).await.expect("create session");
    assert_eq!(
        created.protocol_mode,
        ProtocolMode::Acp,
        "ACP session must be persisted with protocol_mode = Acp"
    );
    assert_eq!(
        created.channel_id.as_deref(),
        Some("C_TEST_CHANNEL"),
        "ACP session must record the originating Slack channel_id"
    );

    // Activate and re-fetch to confirm round-trip through the DB.
    let activated = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    assert_eq!(activated.status, SessionStatus::Active);
    assert_eq!(activated.protocol_mode, ProtocolMode::Acp);
}

/// S018 — the Slack `channel_id` supplied at session-start is preserved
/// after a status transition and subsequent fetch.
#[tokio::test]
async fn acp_session_channel_id_survives_status_update() {
    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&pool));

    let mut session = Session::new(
        "U_OPS".to_owned(),
        std::env::temp_dir().to_string_lossy().to_string(),
        Some("task prompt".to_owned()),
        SessionMode::Remote,
    );
    session.protocol_mode = ProtocolMode::Acp;
    session.channel_id = Some("C_ACP_CHANNEL".to_owned());

    let created = repo.create(&session).await.expect("create");
    let active = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    // Re-fetch from DB to confirm channel_id is stored correctly.
    let fetched = repo
        .get_by_id(&active.id)
        .await
        .expect("fetch")
        .expect("session present");

    assert_eq!(fetched.channel_id.as_deref(), Some("C_ACP_CHANNEL"));
    assert_eq!(fetched.protocol_mode, ProtocolMode::Acp);
}

// ── T086: queued messages are delivered on reconnect ─────────────────────────

/// S060, S062 — When an ACP agent reconnects (i.e., `run_reader` is called for
/// a session with queued steering messages), all unconsumed messages must be
/// delivered via the driver in FIFO order, and each must be marked consumed.
///
/// Uses an in-memory `SQLite` database, an `AcpDriver` with a registered writer
/// channel, and a `BytesMut`-backed fake stdout that closes immediately (EOF)
/// so `run_reader` exits after the flush without blocking.
#[tokio::test]
async fn queued_messages_delivered_on_reconnect() {
    use agent_intercom::acp::reader::{run_reader, ReconnectFlushContext};
    use agent_intercom::driver::acp_driver::AcpDriver;
    use agent_intercom::models::session::{ConnectivityStatus, Session, SessionMode};
    use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
    use agent_intercom::persistence::session_repo::SessionRepo;
    use agent_intercom::persistence::steering_repo::SteeringRepo;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    let pool = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&pool));

    // Create an ACP session that is currently Offline.
    let mut session = Session::new(
        "U_OP".to_owned(),
        std::env::temp_dir().to_string_lossy().to_string(),
        Some("task".to_owned()),
        SessionMode::Remote,
    );
    session.protocol_mode = ProtocolMode::Acp;
    session.channel_id = Some("C_RECON".to_owned());
    session.connectivity_status = ConnectivityStatus::Offline;
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    session_repo
        .set_connectivity_status(&created.id, ConnectivityStatus::Offline)
        .await
        .expect("set offline");

    // Queue two steering messages while offline.
    let steering_repo = SteeringRepo::new(Arc::clone(&pool));
    for i in 1_u8..=2 {
        let msg = SteeringMessage::new(
            created.id.clone(),
            Some("C_RECON".to_owned()),
            format!("queued message {i}"),
            SteeringSource::Slack,
        );
        steering_repo.insert(&msg).await.expect("enqueue");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    // Set up ACP driver with a registered writer channel.
    let acp_driver = Arc::new(AcpDriver::new());
    let (writer_tx, mut writer_rx) = mpsc::channel::<serde_json::Value>(16);
    acp_driver.register_session(&created.id, writer_tx).await;

    // Build reconnect flush context (simulates agent reconnecting).
    let flush_ctx = ReconnectFlushContext {
        db: Arc::clone(&pool),
        driver: acp_driver.clone(),
        slack: None,
        channel_id: Some("C_RECON".to_owned()),
        thread_ts: None,
    };

    // Run reader with an immediately-closed (empty) stdout — it will flush
    // the queue and then exit cleanly on EOF.
    let (event_tx, _event_rx) = mpsc::channel(8);
    let cancel = CancellationToken::new();
    run_reader(
        created.id.clone(),
        tokio::io::empty(),
        event_tx,
        cancel,
        Some(flush_ctx),
    )
    .await
    .expect("run_reader must not error on clean EOF");

    // Verify both queued messages were delivered in FIFO order.
    let msg1 = writer_rx
        .try_recv()
        .expect("first queued message must arrive");
    let msg2 = writer_rx
        .try_recv()
        .expect("second queued message must arrive");

    assert_eq!(msg1["method"].as_str(), Some("prompt/send"));
    assert_eq!(msg1["params"]["text"].as_str(), Some("queued message 1"));

    assert_eq!(msg2["method"].as_str(), Some("prompt/send"));
    assert_eq!(msg2["params"]["text"].as_str(), Some("queued message 2"));

    // Verify messages are now marked consumed in the DB.
    let remaining = steering_repo
        .fetch_unconsumed(&created.id)
        .await
        .expect("fetch remaining");
    assert_eq!(
        remaining.len(),
        0,
        "all queued messages must be consumed after reconnect flush"
    );
}
