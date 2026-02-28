//! Integration tests for ACP session lifecycle (T032).
//!
//! Verifies that starting an ACP session creates a persisted `Session`
//! with `protocol_mode = Acp` and the originating Slack `channel_id`
//! recorded on the record (US3 / FR-025, FR-027, S018).

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
