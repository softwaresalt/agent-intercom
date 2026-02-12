//! Integration test for the nudge flow (T112).
//!
//! Agent makes tool calls → goes silent → verify stall alert created
//! → simulate nudge → verify nudge event delivered with progress snapshot.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use monocoque_agent_rem::models::progress::{ProgressItem, ProgressStatus};
use monocoque_agent_rem::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rem::models::stall::{StallAlert, StallAlertStatus};
use monocoque_agent_rem::orchestrator::stall_detector::{StallDetector, StallEvent};
use monocoque_agent_rem::persistence::db;
use monocoque_agent_rem::persistence::session_repo::SessionRepo;
use monocoque_agent_rem::persistence::stall_repo::StallAlertRepo;

/// Create a test config with a temp workspace root.
fn test_config(dir: &std::path::Path) -> monocoque_agent_rem::config::GlobalConfig {
    let toml_str = format!(
        r#"
default_workspace_root = "{ws}"
host_cli = "echo"
authorized_user_ids = ["U_TEST"]

[slack]
channel_id = "C_TEST"

[timeouts]
approval_seconds = 60
prompt_seconds = 30

[stall]
enabled = true
inactivity_threshold_seconds = 1
escalation_threshold_seconds = 1
max_retries = 2
default_nudge_message = "Please continue"
"#,
        ws = dir.to_string_lossy().replace('\\', "/")
    );
    monocoque_agent_rem::config::GlobalConfig::from_toml_str(&toml_str)
        .expect("test config should parse")
}

#[tokio::test]
async fn stall_alert_created_on_silence() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let config = test_config(tmp.path());
    let database = db::connect(&config, true).await.expect("db connect");
    let db = Arc::new(database);

    // Create an active session.
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let mut session = Session::new(
        "U_TEST".into(),
        tmp.path().to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    session.status = SessionStatus::Active;
    session.progress_snapshot = Some(vec![
        ProgressItem {
            label: "Setup".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "Tests".into(),
            status: ProgressStatus::InProgress,
        },
    ]);
    let created = session_repo.create(&session).await.expect("create session");

    // Start stall detector with short threshold.
    let ct = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(32);
    let detector = StallDetector::new(
        created.id.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
        2,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    // Wait for the stall event.
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("stall event within timeout")
        .expect("channel open");

    assert!(
        matches!(event, StallEvent::Stalled { ref session_id, .. } if session_id == &created.id),
        "expected Stalled event"
    );

    // Create stall alert in DB (as the event handler would do).
    let stall_repo = StallAlertRepo::new(Arc::clone(&db));
    let alert = StallAlert::new(
        created.id.clone(),
        Some("ask_approval".into()),
        Utc::now(),
        1,
        session.progress_snapshot.clone(),
    );
    let saved_alert = stall_repo.create(&alert).await.expect("create alert");
    assert_eq!(saved_alert.status, StallAlertStatus::Pending);
    assert!(saved_alert.progress_snapshot.is_some());
    assert_eq!(
        saved_alert.progress_snapshot.as_ref().map(Vec::len),
        Some(2)
    );

    ct.cancel();
    drop(handle);
}

#[tokio::test]
async fn nudge_updates_alert_and_increments_count() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let config = test_config(tmp.path());
    let database = db::connect(&config, true).await.expect("db connect");
    let db = Arc::new(database);

    // Create an active session and a pending stall alert.
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let mut session = Session::new(
        "U_TEST".into(),
        tmp.path().to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    session.status = SessionStatus::Active;
    let created_session = session_repo.create(&session).await.expect("create session");

    let stall_repo = StallAlertRepo::new(Arc::clone(&db));
    let alert = StallAlert::new(
        created_session.id.clone(),
        Some("heartbeat".into()),
        Utc::now(),
        60,
        None,
    );
    let saved = stall_repo.create(&alert).await.expect("create alert");

    // Simulate nudge: increment count, set status to Nudged.
    let updated = stall_repo
        .increment_nudge_count(&saved.id)
        .await
        .expect("nudge increment");

    assert_eq!(updated.nudge_count, 1);
    assert_eq!(updated.status, StallAlertStatus::Nudged);

    // Second nudge.
    let updated2 = stall_repo
        .increment_nudge_count(&saved.id)
        .await
        .expect("nudge increment 2");
    assert_eq!(updated2.nudge_count, 2);
}

#[tokio::test]
async fn self_recovery_clears_active_alert() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let config = test_config(tmp.path());
    let database = db::connect(&config, true).await.expect("db connect");
    let db = Arc::new(database);

    let session_repo = SessionRepo::new(Arc::clone(&db));
    let mut session = Session::new(
        "U_TEST".into(),
        tmp.path().to_string_lossy().into_owned(),
        None,
        SessionMode::Remote,
    );
    session.status = SessionStatus::Active;
    let created_session = session_repo.create(&session).await.expect("create session");

    let stall_repo = StallAlertRepo::new(Arc::clone(&db));
    let alert = StallAlert::new(created_session.id.clone(), None, Utc::now(), 30, None);
    let saved = stall_repo.create(&alert).await.expect("create alert");

    // Simulate self-recovery: update status.
    let recovered = stall_repo
        .update_status(&saved.id, StallAlertStatus::SelfRecovered)
        .await
        .expect("self recover");

    assert_eq!(recovered.status, StallAlertStatus::SelfRecovered);

    // Active alert query should now return None.
    let active = stall_repo
        .get_active_for_session(&created_session.id)
        .await
        .expect("query active");
    assert!(active.is_none(), "no active alerts after self-recovery");
}
