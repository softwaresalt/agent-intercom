//! Unit tests for `StallAlertRepo` CRUD operations (T022).
//!
//! Validates:
//! - Create stall alert and verify all fields persisted
//! - `get_by_id` returns `None` for missing records
//! - `get_active_for_session` returns `pending` / `nudged` alerts only
//! - `update_status` changes lifecycle state
//! - `increment_nudge_count` atomically bumps count and sets `nudged`
//! - `dismiss` marks alert as dismissed

use std::sync::Arc;

use chrono::Utc;

use monocoque_agent_rc::models::progress::{ProgressItem, ProgressStatus};
use monocoque_agent_rc::models::stall::{StallAlert, StallAlertStatus};
use monocoque_agent_rc::persistence::{db, stall_repo::StallAlertRepo};

fn sample_alert(session_id: &str) -> StallAlert {
    StallAlert::new(
        session_id.to_owned(),
        Some("heartbeat".into()),
        Utc::now(),
        60,
        Some(vec![ProgressItem {
            label: "Setup".into(),
            status: ProgressStatus::Done,
        }]),
    )
}

#[tokio::test]
async fn create_persists_all_fields() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-1");
    let id = alert.id.clone();
    let created = repo.create(&alert).await.expect("create");

    assert_eq!(created.id, id);
    assert_eq!(created.session_id, "sess-1");
    assert_eq!(created.last_tool, Some("heartbeat".to_owned()));
    assert_eq!(created.idle_seconds, 60);
    assert_eq!(created.nudge_count, 0);
    assert_eq!(created.status, StallAlertStatus::Pending);
    assert!(created.progress_snapshot.is_some());
}

#[tokio::test]
async fn get_by_id_returns_none_for_missing() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let result = repo.get_by_id("nonexistent").await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn get_by_id_round_trips() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-2");
    let id = alert.id.clone();
    repo.create(&alert).await.expect("create");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.last_tool, Some("heartbeat".to_owned()));
    assert_eq!(fetched.progress_snapshot.as_ref().map(Vec::len), Some(1));
}

#[tokio::test]
async fn get_active_for_session_returns_pending() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-3");
    repo.create(&alert).await.expect("create");

    let active = repo.get_active_for_session("sess-3").await.expect("query");
    assert!(active.is_some());
    assert_eq!(
        active.as_ref().map(|a| a.status),
        Some(StallAlertStatus::Pending)
    );
}

#[tokio::test]
async fn get_active_for_session_returns_none_after_dismiss() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-4");
    let id = alert.id.clone();
    repo.create(&alert).await.expect("create");
    repo.dismiss(&id).await.expect("dismiss");

    let active = repo.get_active_for_session("sess-4").await.expect("query");
    assert!(active.is_none());
}

#[tokio::test]
async fn update_status_transitions() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-5");
    let id = alert.id.clone();
    repo.create(&alert).await.expect("create");

    repo.update_status(&id, StallAlertStatus::Escalated)
        .await
        .expect("escalate");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.status, StallAlertStatus::Escalated);
}

#[tokio::test]
async fn increment_nudge_count_bumps_and_sets_nudged() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-6");
    let id = alert.id.clone();
    repo.create(&alert).await.expect("create");

    repo.increment_nudge_count(&id).await.expect("nudge1");
    let after1 = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(after1.nudge_count, 1);
    assert_eq!(after1.status, StallAlertStatus::Nudged);

    repo.increment_nudge_count(&id).await.expect("nudge2");
    let after2 = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(after2.nudge_count, 2);
}

#[tokio::test]
async fn dismiss_sets_dismissed_status() {
    let db = db::connect_memory().await.expect("db");
    let repo = StallAlertRepo::new(Arc::new(db));

    let alert = sample_alert("sess-7");
    let id = alert.id.clone();
    repo.create(&alert).await.expect("create");

    repo.dismiss(&id).await.expect("dismiss");

    let fetched = repo.get_by_id(&id).await.expect("query").expect("exists");
    assert_eq!(fetched.status, StallAlertStatus::Dismissed);
}
