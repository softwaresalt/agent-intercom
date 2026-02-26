//! Unit tests for `InboxRepo` CRUD operations (T028, T029).
//!
//! Covers scenarios S013-S014, S017, S019-S020:
//! - Insert and retrieve unconsumed items
//! - `mark_consumed` marks delivered items
//! - Channel-scoped delivery: items route to correct channel
//! - Boundary: purge removes old items

use std::sync::Arc;

use agent_intercom::models::inbox::{InboxSource, TaskInboxItem};
use agent_intercom::persistence::{db, inbox_repo::InboxRepo};

fn sample_item(channel_id: Option<&str>, text: &str) -> TaskInboxItem {
    TaskInboxItem::new(
        channel_id.map(str::to_owned),
        text.to_owned(),
        InboxSource::Slack,
    )
}

// ─── S013: insert stores item ─────────────────────────────────────────

#[tokio::test]
async fn insert_persists_all_fields() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    let item = sample_item(Some("C1"), "implement auth tests");
    let saved = repo.insert(&item).await.expect("insert");

    assert_eq!(saved.channel_id, Some("C1".to_owned()));
    assert_eq!(saved.message, "implement auth tests");
    assert_eq!(saved.source, InboxSource::Slack);
    assert!(!saved.consumed);
}

// ─── S014: fetch_unconsumed_by_channel returns unconsumed items ───────

#[tokio::test]
async fn fetch_unconsumed_returns_pending_items() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    let i1 = sample_item(Some("C2"), "first task");
    let i2 = sample_item(Some("C2"), "second task");
    repo.insert(&i1).await.expect("insert i1");
    repo.insert(&i2).await.expect("insert i2");

    let items = repo
        .fetch_unconsumed_by_channel(Some("C2"))
        .await
        .expect("fetch");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].message, "first task");
    assert_eq!(items[1].message, "second task");
}

// ─── fetch_unconsumed excludes consumed items ─────────────────────────

#[tokio::test]
async fn fetch_unconsumed_excludes_consumed_items() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    let item = sample_item(Some("C3"), "already done");
    let saved = repo.insert(&item).await.expect("insert");
    repo.mark_consumed(&saved.id).await.expect("consume");

    let items = repo
        .fetch_unconsumed_by_channel(Some("C3"))
        .await
        .expect("fetch");
    assert!(items.is_empty());
}

// ─── S017: channel-scoped delivery ───────────────────────────────────

#[tokio::test]
async fn channel_scoped_fetch_does_not_cross_channels() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    // Channel C4 item should NOT appear in C5 fetch.
    let c4 = sample_item(Some("C4"), "channel four task");
    let c5 = sample_item(Some("C5"), "channel five task");
    repo.insert(&c4).await.expect("insert c4");
    repo.insert(&c5).await.expect("insert c5");

    let items = repo
        .fetch_unconsumed_by_channel(Some("C5"))
        .await
        .expect("fetch");
    assert_eq!(items.len(), 1, "C5 should only see its own task");
    assert_eq!(items[0].message, "channel five task");
}

// ─── S017b: null channel items appear in any channel fetch ───────────

#[tokio::test]
async fn null_channel_item_visible_to_channel_fetch() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    // An item with no channel should appear in any channel's fetch.
    let global = sample_item(None, "global task");
    let channel_specific = sample_item(Some("C6"), "c6 task");
    repo.insert(&global).await.expect("insert global");
    repo.insert(&channel_specific).await.expect("insert c6");

    let items = repo
        .fetch_unconsumed_by_channel(Some("C6"))
        .await
        .expect("fetch");
    assert_eq!(
        items.len(),
        2,
        "should see both global and channel-specific"
    );
}

// ─── S019: IPC source stored correctly ───────────────────────────────

#[tokio::test]
async fn insert_ipc_source_stored() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    let item = TaskInboxItem::new(None, "ipc task".to_owned(), InboxSource::Ipc);
    let saved = repo.insert(&item).await.expect("insert");
    assert_eq!(saved.source, InboxSource::Ipc);
}

// ─── S020: purge removes old items ───────────────────────────────────

#[tokio::test]
async fn purge_removes_old_items() {
    let db = db::connect_memory().await.expect("db");
    let repo = InboxRepo::new(Arc::new(db));

    let old = sample_item(None, "old task");
    repo.insert(&old).await.expect("insert");

    let cutoff = chrono::Utc::now() + chrono::Duration::seconds(1);
    let deleted = repo.purge(cutoff).await.expect("purge");
    assert_eq!(deleted, 1);

    let remaining = repo.fetch_unconsumed_by_channel(None).await.expect("fetch");
    assert!(remaining.is_empty());
}
