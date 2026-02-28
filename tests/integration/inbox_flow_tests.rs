//! Integration tests for the task inbox end-to-end flow (T031).
//!
//! Covers scenarios S013-S022: task insertion, delivery at session start,
//! channel scoping, `mark_consumed` idempotency, IPC source, purge.

use std::sync::Arc;

use agent_intercom::models::inbox::{InboxSource, TaskInboxItem};
use agent_intercom::persistence::inbox_repo::InboxRepo;

use super::test_helpers::{test_app_state, test_config};

// ─── S013: Insert and retrieve task ──────────────────────────────────

#[tokio::test]
async fn inbox_task_inserted_and_retrieved() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let item = TaskInboxItem::new(
        Some("C_INBOX".to_owned()),
        "write docs".to_owned(),
        InboxSource::Slack,
    );
    let saved = repo.insert(&item).await.expect("insert");

    let fetched = repo
        .fetch_unconsumed_by_channel(Some("C_INBOX"))
        .await
        .expect("fetch");
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].id, saved.id);
    assert_eq!(fetched[0].message, "write docs");
}

// ─── S014: mark_consumed removes from pending ────────────────────────

#[tokio::test]
async fn mark_consumed_removes_from_pending() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let item = TaskInboxItem::new(None, "test task".to_owned(), InboxSource::Slack);
    let saved = repo.insert(&item).await.expect("insert");
    repo.mark_consumed(&saved.id).await.expect("consume");

    let pending = repo.fetch_unconsumed_by_channel(None).await.expect("fetch");
    assert!(pending.is_empty());
}

// ─── S015: empty inbox returns empty vec ─────────────────────────────

#[tokio::test]
async fn empty_inbox_returns_empty_vec() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let items = repo
        .fetch_unconsumed_by_channel(Some("C_EMPTY"))
        .await
        .expect("fetch");
    assert!(items.is_empty());
}

// ─── S017: channel scoping ───────────────────────────────────────────

#[tokio::test]
async fn channel_scoped_delivery() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let ca = TaskInboxItem::new(
        Some("CA".to_owned()),
        "task for A".to_owned(),
        InboxSource::Slack,
    );
    let cb = TaskInboxItem::new(
        Some("CB".to_owned()),
        "task for B".to_owned(),
        InboxSource::Slack,
    );
    repo.insert(&ca).await.expect("insert ca");
    repo.insert(&cb).await.expect("insert cb");

    let a_items = repo
        .fetch_unconsumed_by_channel(Some("CA"))
        .await
        .expect("fetch A");
    let b_items = repo
        .fetch_unconsumed_by_channel(Some("CB"))
        .await
        .expect("fetch B");

    assert_eq!(a_items.len(), 1);
    assert_eq!(a_items[0].message, "task for A");
    assert_eq!(b_items.len(), 1);
    assert_eq!(b_items[0].message, "task for B");
}

// ─── S017b: global tasks visible to all channels ─────────────────────

#[tokio::test]
async fn global_task_visible_to_any_channel() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let global = TaskInboxItem::new(None, "global test".to_owned(), InboxSource::Ipc);
    repo.insert(&global).await.expect("insert global");

    let items_cx = repo
        .fetch_unconsumed_by_channel(Some("CX"))
        .await
        .expect("fetch CX");
    let items_cy = repo
        .fetch_unconsumed_by_channel(Some("CY"))
        .await
        .expect("fetch CY");

    assert_eq!(items_cx.len(), 1, "CX should see global task");
    assert_eq!(items_cy.len(), 1, "CY should see global task");
}

// ─── S019: IPC source preserved ──────────────────────────────────────

#[tokio::test]
async fn ipc_source_preserved() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let item = TaskInboxItem::new(None, "from ctl".to_owned(), InboxSource::Ipc);
    let saved = repo.insert(&item).await.expect("insert");
    assert_eq!(saved.source, InboxSource::Ipc);
}

// ─── S020: purge removes old items ──────────────────────────────────

#[tokio::test]
async fn purge_removes_old_items() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let item = TaskInboxItem::new(None, "old task".to_owned(), InboxSource::Slack);
    repo.insert(&item).await.expect("insert");

    let cutoff = chrono::Utc::now() + chrono::Duration::seconds(1);
    let deleted = repo.purge(cutoff).await.expect("purge");
    assert_eq!(deleted, 1);

    let remaining = repo.fetch_unconsumed_by_channel(None).await.expect("fetch");
    assert!(remaining.is_empty());
}

// ─── S022: mark_consumed is idempotent ──────────────────────────────

#[tokio::test]
async fn mark_consumed_is_idempotent() {
    let root = std::env::temp_dir();
    let config = test_config(root.to_str().unwrap_or("/tmp"));
    let state = test_app_state(config).await;
    let repo = InboxRepo::new(Arc::clone(&state.db));

    let item = TaskInboxItem::new(None, "test".to_owned(), InboxSource::Slack);
    let saved = repo.insert(&item).await.expect("insert");
    repo.mark_consumed(&saved.id).await.expect("first consume");
    repo.mark_consumed(&saved.id)
        .await
        .expect("second consume — idempotent");
}
