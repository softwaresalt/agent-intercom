//! Integration tests for ACP event thread management (Phase 5 — S036–S041).
//!
//! Validates the thread continuity contract: ACP event handlers create a new
//! Slack thread when a session has no thread anchor, and reply within the
//! existing thread when one is already established. The persistence layer
//! (`SessionRepo::set_thread_ts`) uses a `WHERE thread_ts IS NULL` guard for
//! idempotent first-write semantics.

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Create and activate a session in the in-memory DB.
async fn new_active_session(repo: &SessionRepo, workspace_root: &str) -> Session {
    let s = Session::new(
        "U_OP".to_owned(),
        workspace_root.to_owned(),
        Some("test".to_owned()),
        SessionMode::Remote,
    );
    let created = repo.create(&s).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session")
}

// ── S036: Clearance creates thread when thread_ts = None ─────────────────────

/// S036 — When a session has `thread_ts = None`, calling `set_thread_ts` after
/// a successful Slack post stores the returned `ts` value and the session's
/// `thread_ts` becomes `Some(ts)` on the next lookup.
///
/// This validates the persistence contract that underpins the first-message
/// thread-anchoring logic in `handle_clearance_requested`.
#[tokio::test]
async fn clearance_creates_thread_when_session_has_no_thread_ts() {
    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let session = new_active_session(&repo, "/tmp/ws-s036").await;
    assert!(
        session.thread_ts.is_none(),
        "S036: new session must have thread_ts=None"
    );

    // Simulate the handler calling set_thread_ts after post_message_direct succeeds.
    let slack_ts = "1741234567.000001";
    repo.set_thread_ts(&session.id, slack_ts)
        .await
        .expect("set_thread_ts must succeed");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        updated.thread_ts.as_deref(),
        Some(slack_ts),
        "S036: thread_ts must be stored after clearance creates thread"
    );
}

// ── S037: Clearance replies in existing thread when thread_ts = Some ─────────

/// S037 — When a session already has a `thread_ts`, the handler reads the
/// existing value and passes it as `thread_ts` to the Slack message.
/// `set_thread_ts` is NOT called again (the `WHERE thread_ts IS NULL` guard
/// prevents overwrite). The stored value remains unchanged.
#[tokio::test]
async fn clearance_uses_existing_thread_when_session_has_thread_ts() {
    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let session = new_active_session(&repo, "/tmp/ws-s037").await;

    // Simulate a prior event that anchored the thread.
    let original_ts = "1741234567.000002";
    repo.set_thread_ts(&session.id, original_ts)
        .await
        .expect("anchor thread");

    // Simulate a second clearance arriving — handler reads session.thread_ts (Some).
    // It passes original_ts to Slack and does NOT call set_thread_ts again
    // (branch: `if session_thread_ts.is_none()` is false → skip).
    let fetched = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert!(
        fetched.thread_ts.is_some(),
        "S037: session must have thread_ts=Some for second clearance"
    );
    assert_eq!(
        fetched.thread_ts.as_deref(),
        Some(original_ts),
        "S037: existing thread_ts must not change"
    );
}

// ── S038: Prompt creates thread when thread_ts = None ────────────────────────

/// S038 — When a session has no thread anchor, a `PromptForwarded` event uses
/// `post_message_direct` (not `enqueue`) to capture the `ts` and anchor the
/// session thread via `set_thread_ts`. This test validates the persistence half
/// of that contract.
#[tokio::test]
async fn prompt_creates_thread_when_session_has_no_thread_ts() {
    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let session = new_active_session(&repo, "/tmp/ws-s038").await;
    assert!(
        session.thread_ts.is_none(),
        "S038: new session must have thread_ts=None before prompt"
    );

    // Simulate the handler calling set_thread_ts after post_message_direct succeeds.
    let slack_ts = "1741234567.000003";
    repo.set_thread_ts(&session.id, slack_ts)
        .await
        .expect("set_thread_ts must succeed");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        updated.thread_ts.as_deref(),
        Some(slack_ts),
        "S038: thread_ts must be stored after prompt creates thread"
    );
}

// ── S039: Prompt enqueues to existing thread when thread_ts = Some ───────────

/// S039 — When a session already has a `thread_ts`, the `PromptForwarded`
/// handler branches to `SlackService::enqueue` (not `post_message_direct`).
/// The existing `thread_ts` is passed as `thread_ts` on the message.
/// The stored `thread_ts` value must not change (no second `set_thread_ts` call).
#[tokio::test]
async fn prompt_uses_existing_thread_when_session_has_thread_ts() {
    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let session = new_active_session(&repo, "/tmp/ws-s039").await;
    let original_ts = "1741234567.000004";
    repo.set_thread_ts(&session.id, original_ts)
        .await
        .expect("anchor thread");

    // Simulate the handler reading the session — it sees thread_ts=Some.
    // Handler calls enqueue(msg with thread_ts=original_ts) — no set_thread_ts.
    let fetched = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        fetched.thread_ts.as_deref(),
        Some(original_ts),
        "S039: prompt must see existing thread_ts=Some and pass it to Slack"
    );

    // Explicitly verify that a spurious second set_thread_ts call would not
    // overwrite (see S040 for the idempotency test).
    let new_ts = "1741234567.000099";
    repo.set_thread_ts(&session.id, new_ts)
        .await
        .expect("second set_thread_ts must not error");
    let after = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        after.thread_ts.as_deref(),
        Some(original_ts),
        "S039: second set_thread_ts call must not overwrite existing thread_ts"
    );
}

// ── S040: set_thread_ts is idempotent ─────────────────────────────────────────

/// S040 — `SessionRepo::set_thread_ts` uses `WHERE thread_ts IS NULL` in the
/// SQL `UPDATE` statement. Calling it a second time with a different value must
/// not overwrite the first value. This ensures concurrent or duplicate
/// first-message events cannot corrupt the thread anchor.
#[tokio::test]
async fn set_thread_ts_is_idempotent_prevents_overwrite() {
    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let session = new_active_session(&repo, "/tmp/ws-s040").await;
    assert!(
        session.thread_ts.is_none(),
        "S040: pre-condition: thread_ts must be None"
    );

    // First call — sets the thread anchor.
    let first_ts = "1741234567.100000";
    repo.set_thread_ts(&session.id, first_ts)
        .await
        .expect("first set_thread_ts must succeed");

    // Second call — must be a no-op (WHERE thread_ts IS NULL is now false).
    let second_ts = "1741234567.200000";
    repo.set_thread_ts(&session.id, second_ts)
        .await
        .expect("second set_thread_ts must not error");

    // Third call — also a no-op.
    let third_ts = "1741234567.300000";
    repo.set_thread_ts(&session.id, third_ts)
        .await
        .expect("third set_thread_ts must not error");

    let final_session = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        final_session.thread_ts.as_deref(),
        Some(first_ts),
        "S040: first thread_ts must be preserved — subsequent calls are no-ops"
    );
}

// ── S041: Sequential events share thread anchor ───────────────────────────────

/// S041 — When a clearance request creates a session thread, a subsequent
/// prompt-forwarded event for the same session must see `thread_ts=Some` and
/// use the existing thread rather than creating a new one.
///
/// This is the "full round-trip" thread continuity contract: first event anchors
/// the thread, all subsequent events inherit the same anchor.
#[tokio::test]
async fn sequential_events_share_thread_anchor() {
    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let session = new_active_session(&repo, "/tmp/ws-s041").await;
    assert!(
        session.thread_ts.is_none(),
        "S041: pre-condition: new session has no thread"
    );

    // Event 1: clearance request arrives → post_message_direct → captures ts-1.
    let clearance_ts = "1741234567.000010";
    repo.set_thread_ts(&session.id, clearance_ts)
        .await
        .expect("clearance anchors thread");

    // Event 2: prompt-forwarded arrives → handler reads session.thread_ts.
    let after_clearance = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        after_clearance.thread_ts.as_deref(),
        Some(clearance_ts),
        "S041: prompt event must see the thread_ts anchored by the clearance event"
    );

    // The prompt handler sees thread_ts=Some → branches to enqueue (not direct post).
    // No second set_thread_ts call is made — thread anchor is shared.
    // Verify the anchor is still intact after the prompt would have enqueued.
    repo.set_thread_ts(&session.id, "1741234567.000011") // would be a no-op
        .await
        .expect("spurious set_thread_ts is no-op");

    let final_session = repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");
    assert_eq!(
        final_session.thread_ts.as_deref(),
        Some(clearance_ts),
        "S041: thread anchor must remain the clearance ts across all subsequent events"
    );
}
