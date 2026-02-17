use std::sync::Arc;

use monocoque_agent_rc::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rc::persistence::{db, session_repo::SessionRepo};

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

    let fetched = repo.get_by_id(&created.id).await.expect("fetch session");
    assert_eq!(fetched.status, SessionStatus::Active);
}
