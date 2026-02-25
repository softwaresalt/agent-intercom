use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};

/// T005: In-memory `connect_memory()` creates pool with all 5 tables.
#[tokio::test]
async fn in_memory_connect_creates_five_tables() {
    let pool = db::connect_memory()
        .await
        .expect("in-memory connect should succeed");

    let tables = [
        "session",
        "approval_request",
        "checkpoint",
        "continuation_prompt",
        "stall_alert",
    ];

    for table in tables {
        let query = format!("SELECT COUNT(*) AS cnt FROM {table}");
        let row: (i64,) = sqlx::query_as(&query)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("table '{table}' should be queryable: {e}"));
        assert_eq!(row.0, 0, "table '{table}' should start empty");
    }
}

#[tokio::test]
async fn create_and_update_session() {
    let db = db::connect_memory().await.expect("db connect");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new(
        "U123".into(),
        "/test/workspace".into(),
        Some("hello".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    assert_eq!(created.owner_user_id, "U123");

    let activated = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");
    assert_eq!(activated.status, SessionStatus::Active);

    let count = repo.count_active().await.expect("count active");
    assert_eq!(count, 1);

    let fetched = repo
        .get_by_id(&created.id)
        .await
        .expect("fetch session")
        .expect("session should exist");
    assert_eq!(fetched.status, SessionStatus::Active);
}

#[tokio::test]
async fn get_by_id_returns_none_for_missing() {
    let db = db::connect_memory().await.expect("db");
    let repo = SessionRepo::new(Arc::new(db));

    let result = repo.get_by_id("nonexistent").await.expect("query");
    assert!(result.is_none());
}

#[tokio::test]
async fn list_active_returns_only_active_sessions() {
    let db = db::connect_memory().await.expect("db");
    let repo = SessionRepo::new(Arc::new(db));

    let s1 = Session::new("U1".into(), "/ws1".into(), None, SessionMode::Remote);
    let s2 = Session::new("U2".into(), "/ws2".into(), None, SessionMode::Remote);
    let c1 = repo.create(&s1).await.expect("create1");
    let c2 = repo.create(&s2).await.expect("create2");

    repo.update_status(&c1.id, SessionStatus::Active)
        .await
        .expect("activate1");
    // s2 stays in Created status.

    let active = repo.list_active().await.expect("list");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, c1.id);
    drop(c2);
}

#[tokio::test]
async fn set_terminated_records_terminated_at() {
    let db = db::connect_memory().await.expect("db");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let terminated = repo
        .set_terminated(&created.id, SessionStatus::Terminated)
        .await
        .expect("terminate");
    assert_eq!(terminated.status, SessionStatus::Terminated);
    assert!(terminated.terminated_at.is_some());
}

#[tokio::test]
async fn update_last_activity_sets_tool() {
    let db = db::connect_memory().await.expect("db");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create");

    repo.update_last_activity(&created.id, Some("heartbeat".into()))
        .await
        .expect("update activity");

    let fetched = repo
        .get_by_id(&created.id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(fetched.last_tool, Some("heartbeat".to_owned()));
}

#[tokio::test]
async fn update_mode_changes_session_mode() {
    let db = db::connect_memory().await.expect("db");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create");

    repo.update_mode(&created.id, SessionMode::Local)
        .await
        .expect("update mode");

    let fetched = repo
        .get_by_id(&created.id)
        .await
        .expect("query")
        .expect("exists");
    assert_eq!(fetched.mode, SessionMode::Local);
}

#[tokio::test]
async fn invalid_transition_rejected() {
    let db = db::connect_memory().await.expect("db");
    let repo = SessionRepo::new(Arc::new(db));

    let session = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create");

    // Created → Paused is not a valid transition.
    let result = repo.update_status(&created.id, SessionStatus::Paused).await;
    assert!(result.is_err());
}
