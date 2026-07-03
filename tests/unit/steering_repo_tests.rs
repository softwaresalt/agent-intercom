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

// ─── F.3-T2: durable steering-queue persistence across restarts ──────

/// New messages have no origin session until reassigned to a resumed session.
#[tokio::test]
async fn insert_defaults_origin_session_id_to_none() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let saved = repo
        .insert(&sample_msg("sess-origin", None, "no origin yet"))
        .await
        .expect("insert");
    assert_eq!(saved.origin_session_id, None);

    let fetched = repo.fetch_unconsumed("sess-origin").await.expect("fetch");
    assert_eq!(fetched[0].origin_session_id, None);
}

/// Reassigning carries only the *unconsumed* messages of a crashed session to
/// the resumed session, preserving FIFO order and recording the crashed
/// session id as the durable origin.
#[tokio::test]
async fn reassign_unconsumed_carries_messages_to_resumed_session() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    let m1 = sample_msg("sess-crashed", Some("C1"), "pending 1");
    let m2 = sample_msg("sess-crashed", Some("C1"), "pending 2");
    let m3 = sample_msg("sess-crashed", Some("C1"), "already delivered");
    repo.insert(&m1).await.expect("insert m1");
    repo.insert(&m2).await.expect("insert m2");
    let saved3 = repo.insert(&m3).await.expect("insert m3");
    repo.mark_consumed(&saved3.id).await.expect("consume m3");

    let moved = repo
        .reassign_unconsumed_to_session("sess-crashed", "sess-resumed")
        .await
        .expect("reassign");
    assert_eq!(moved, 2, "only the 2 unconsumed messages move");

    // The crashed session has no unconsumed messages left.
    let crashed = repo
        .fetch_unconsumed("sess-crashed")
        .await
        .expect("fetch crashed");
    assert!(crashed.is_empty());

    // The resumed session inherits them in FIFO order with origin recorded.
    let resumed = repo
        .fetch_unconsumed("sess-resumed")
        .await
        .expect("fetch resumed");
    assert_eq!(resumed.len(), 2);
    assert_eq!(resumed[0].message, "pending 1");
    assert_eq!(resumed[1].message, "pending 2");
    assert_eq!(
        resumed[0].origin_session_id.as_deref(),
        Some("sess-crashed")
    );
    assert_eq!(
        resumed[1].origin_session_id.as_deref(),
        Some("sess-crashed")
    );
}

/// A message reassigned across multiple restarts keeps its *original* origin,
/// not the intermediate session id.
#[tokio::test]
async fn reassign_preserves_original_origin_across_chained_restarts() {
    let db = db::connect_memory().await.expect("db");
    let repo = SteeringRepo::new(Arc::new(db));

    repo.insert(&sample_msg("sess-a", None, "chained"))
        .await
        .expect("insert");

    repo.reassign_unconsumed_to_session("sess-a", "sess-b")
        .await
        .expect("a->b");
    repo.reassign_unconsumed_to_session("sess-b", "sess-c")
        .await
        .expect("b->c");

    let c = repo.fetch_unconsumed("sess-c").await.expect("fetch c");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].origin_session_id.as_deref(), Some("sess-a"));
}

/// Steering queue contents survive a full DB restart (close pool, reopen the
/// same file-backed database).
#[tokio::test]
async fn steering_messages_survive_db_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("steer-restart.db");
    let path_str = path.to_str().expect("utf8");

    let saved_id = {
        let db = db::connect(path_str).await.expect("connect");
        let repo = SteeringRepo::new(Arc::new(db));
        let saved = repo
            .insert(&sample_msg("sess-restart", Some("C1"), "survive restart"))
            .await
            .expect("insert");
        saved.id
    }; // pool dropped == server shutdown

    // Reopen the same file (simulated restart).
    let db2 = db::connect(path_str).await.expect("reconnect");
    let repo2 = SteeringRepo::new(Arc::new(db2));
    let msgs = repo2
        .fetch_unconsumed("sess-restart")
        .await
        .expect("fetch after restart");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, saved_id);
    assert_eq!(msgs[0].message, "survive restart");
    assert!(!msgs[0].consumed);
}

/// A legacy database whose `steering_message` table predates the
/// `origin_session_id` column migrates additively on reconnect: existing rows
/// survive and the new column becomes available.
#[tokio::test]
async fn legacy_db_without_origin_column_migrates_additively() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("legacy-steer.db");
    let path_str = path.to_str().expect("utf8");

    // Build a legacy DB whose steering_message table lacks origin_session_id.
    {
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("legacy pool");
        sqlx::raw_sql(
            "CREATE TABLE steering_message (
                id TEXT PRIMARY KEY NOT NULL,
                session_id TEXT NOT NULL,
                channel_id TEXT,
                message TEXT NOT NULL,
                source TEXT NOT NULL CHECK(source IN ('slack','ipc')),
                created_at TEXT NOT NULL,
                consumed INTEGER NOT NULL DEFAULT 0
            );",
        )
        .execute(&pool)
        .await
        .expect("legacy ddl");
        sqlx::query(
            "INSERT INTO steering_message
             (id, session_id, channel_id, message, source, created_at, consumed)
             VALUES ('steer:legacy', 'sess-legacy', NULL, 'legacy message', 'slack', ?1, 0)",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .expect("legacy insert");
        pool.close().await;
    }

    // Reopen with the production connect path, which runs the additive migration.
    let db = db::connect(path_str)
        .await
        .expect("connect migrates legacy db");
    let repo = SteeringRepo::new(Arc::new(db));

    // Legacy row survived and reads back with the new column defaulting to None.
    let msgs = repo
        .fetch_unconsumed("sess-legacy")
        .await
        .expect("fetch legacy");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].message, "legacy message");
    assert_eq!(msgs[0].origin_session_id, None);

    // Reassignment works against the migrated legacy row.
    let moved = repo
        .reassign_unconsumed_to_session("sess-legacy", "sess-resumed")
        .await
        .expect("reassign legacy");
    assert_eq!(moved, 1);
    let resumed = repo
        .fetch_unconsumed("sess-resumed")
        .await
        .expect("fetch resumed");
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].origin_session_id.as_deref(), Some("sess-legacy"));
}
