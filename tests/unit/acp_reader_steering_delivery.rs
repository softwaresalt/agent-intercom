//! Unit tests for steering delivery reliability (T001–T006, US1 — F-06).
//!
//! Verifies that `deliver_queued_messages` in `src/acp/reader.rs` only marks
//! a steering message as consumed when `send_prompt` returns `Ok`. This is
//! the F-06 correctness fix: previously, `mark_consumed` was called
//! unconditionally, causing messages to be silently dropped on delivery
//! failure.
//!
//! Scenario coverage:
//! - S001: successful delivery marks message consumed
//! - S002: failed delivery preserves unconsumed status
//! - S003: partial failure — only the failed message stays unconsumed
//! - S004: retry on next flush delivers previously failed message
//! - S006: empty queue is a no-op
//! - S007: `mark_consumed` failure after successful send — warning logged,
//!   message stays unconsumed

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_intercom::acp::reader::deliver_queued_messages;
use agent_intercom::driver::AgentDriver;
use agent_intercom::models::steering::{SteeringMessage, SteeringSource};
use agent_intercom::persistence::{db, steering_repo::SteeringRepo};
use agent_intercom::{AppError, Result};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build an in-memory database with the full schema applied.
async fn make_db() -> Arc<db::Database> {
    Arc::new(db::connect_memory().await.expect("in-memory db"))
}

/// Construct a sample steering message for the given session.
fn sample_msg(session_id: &str, text: &str) -> SteeringMessage {
    SteeringMessage::new(
        session_id.to_owned(),
        None,
        text.to_owned(),
        SteeringSource::Slack,
    )
}

// ── Mock driver ───────────────────────────────────────────────────────────────

/// Configurable mock `AgentDriver` for unit testing `deliver_queued_messages`.
///
/// Each call to `send_prompt` pops the next response from the front of
/// `responses`. When the queue is empty, subsequent calls return `Ok(())`.
struct MockDriver {
    /// Ordered responses: `true` = `Ok(())`, `false` = `Err(AppError::Acp(...))`.
    responses: Arc<Mutex<VecDeque<bool>>>,
}

impl MockDriver {
    /// Create a driver whose `send_prompt` returns the given ordered responses.
    fn with_responses(responses: impl IntoIterator<Item = bool>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into_iter().collect())),
        }
    }

    /// Create a driver that always succeeds (empty response queue → default Ok).
    fn always_succeed() -> Self {
        Self::with_responses([])
    }
}

impl AgentDriver for MockDriver {
    fn resolve_clearance(
        &self,
        _request_id: &str,
        _approved: bool,
        _reason: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn send_prompt(
        &self,
        _session_id: &str,
        _prompt: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let responses = Arc::clone(&self.responses);
        Box::pin(async move {
            let succeed = responses.lock().unwrap().pop_front().unwrap_or(true);
            if succeed {
                Ok(())
            } else {
                Err(AppError::Acp("mock: send_prompt failed".into()))
            }
        })
    }

    fn interrupt(
        &self,
        _session_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn resolve_prompt(
        &self,
        _prompt_id: &str,
        _decision: &str,
        _instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn resolve_wait(
        &self,
        _session_id: &str,
        _instruction: Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

// ── T001: successful delivery marks consumed (S001) ───────────────────────────

/// T001 — When `send_prompt` succeeds, the steering message must be marked
/// consumed in the repository (S001).
#[tokio::test]
async fn successful_delivery_marks_message_consumed() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    let driver = MockDriver::always_succeed();

    let msg = sample_msg("sess-t001", "do the thing");
    let saved = repo.insert(&msg).await.expect("insert");

    let queued = repo.fetch_unconsumed("sess-t001").await.expect("fetch");
    deliver_queued_messages("sess-t001", &queued, &driver, &repo).await;

    let remaining = repo
        .fetch_unconsumed("sess-t001")
        .await
        .expect("fetch after");
    assert!(
        remaining.is_empty(),
        "message must be marked consumed after successful send; id={}",
        saved.id
    );
}

// ── T002: failed delivery preserves unconsumed status (S002) ─────────────────

/// T002 — When `send_prompt` returns an error, the message must NOT be marked
/// consumed so that it can be retried on the next reconnect flush (S002).
#[tokio::test]
async fn failed_delivery_preserves_unconsumed_status() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    let driver = MockDriver::with_responses([false]);

    let msg = sample_msg("sess-t002", "urgent instruction");
    repo.insert(&msg).await.expect("insert");

    let queued = repo.fetch_unconsumed("sess-t002").await.expect("fetch");
    deliver_queued_messages("sess-t002", &queued, &driver, &repo).await;

    let remaining = repo
        .fetch_unconsumed("sess-t002")
        .await
        .expect("fetch after");
    assert_eq!(
        remaining.len(),
        1,
        "message must remain unconsumed after a failed send"
    );
}

// ── T003: partial failure — only failed message stays unconsumed (S003) ───────

/// T003 — When 3 messages are flushed and the middle delivery fails, only the
/// failed message must remain unconsumed; the others must be marked consumed
/// (S003).
#[tokio::test]
async fn partial_failure_only_failed_message_stays_unconsumed() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    // Responses in FIFO order: first succeeds, second fails, third succeeds.
    let driver = MockDriver::with_responses([true, false, true]);

    for text in &["first", "second", "third"] {
        let m = sample_msg("sess-t003", text);
        repo.insert(&m).await.expect("insert");
        // Small sleep ensures strictly increasing created_at for FIFO ordering.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let queued = repo.fetch_unconsumed("sess-t003").await.expect("fetch");
    assert_eq!(
        queued.len(),
        3,
        "all 3 messages must be unconsumed initially"
    );

    deliver_queued_messages("sess-t003", &queued, &driver, &repo).await;

    let remaining = repo
        .fetch_unconsumed("sess-t003")
        .await
        .expect("fetch after");
    assert_eq!(
        remaining.len(),
        1,
        "only the failed message must remain unconsumed"
    );
    assert_eq!(
        remaining[0].message, "second",
        "the failed (middle) message must be the remaining unconsumed one"
    );
}

// ── T004: retry succeeds on next flush (S004) ─────────────────────────────────

/// T004 — A message that failed delivery on the first flush must remain
/// unconsumed and be successfully delivered on the second flush (S004).
#[tokio::test]
async fn retry_succeeds_on_next_flush() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));

    let msg = sample_msg("sess-t004", "please continue");
    repo.insert(&msg).await.expect("insert");

    // First flush: delivery fails → message stays unconsumed.
    let driver_fail = MockDriver::with_responses([false]);
    let queued = repo.fetch_unconsumed("sess-t004").await.expect("fetch1");
    deliver_queued_messages("sess-t004", &queued, &driver_fail, &repo).await;

    let after_first = repo
        .fetch_unconsumed("sess-t004")
        .await
        .expect("fetch after first");
    assert_eq!(
        after_first.len(),
        1,
        "message must still be unconsumed after first (failed) flush"
    );

    // Second flush: delivery succeeds → message is marked consumed.
    let driver_ok = MockDriver::always_succeed();
    let queued2 = repo.fetch_unconsumed("sess-t004").await.expect("fetch2");
    deliver_queued_messages("sess-t004", &queued2, &driver_ok, &repo).await;

    let after_second = repo
        .fetch_unconsumed("sess-t004")
        .await
        .expect("fetch after second");
    assert!(
        after_second.is_empty(),
        "message must be consumed after second (successful) flush"
    );
}

// ── T005: empty queue is no-op (S006) ─────────────────────────────────────────

/// T005 — When the queue is empty, `deliver_queued_messages` must complete
/// without error or panic, and `send_prompt` must never be called (S006).
#[tokio::test]
async fn empty_queue_is_no_op() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    // Driver configured to fail: if send_prompt were called, the test would
    // surface that as an Err — but it must never be called for an empty queue.
    let driver = MockDriver::with_responses([false]);

    // Deliver with an empty slice.
    deliver_queued_messages("sess-t005", &[], &driver, &repo).await;

    // No messages were inserted, so fetch returns empty.
    let remaining = repo.fetch_unconsumed("sess-t005").await.expect("fetch");
    assert!(
        remaining.is_empty(),
        "empty queue flush must be a true no-op"
    );

    // Verify send_prompt was never called: the one configured response remains.
    let remaining_responses = driver.responses.lock().unwrap().len();
    assert_eq!(
        remaining_responses, 1,
        "send_prompt must not be called for an empty queue"
    );
}

// ── T006: mark_consumed failure after successful send (S007) ──────────────────

/// T006 — When `send_prompt` succeeds but `mark_consumed` encounters a DB
/// error, the function must log a warning and continue without panicking.
/// The message remains unconsumed because the DB update did not execute (S007).
#[tokio::test]
async fn mark_consumed_failure_after_successful_send_is_handled() {
    let db = make_db().await;
    let repo = SteeringRepo::new(Arc::clone(&db));
    let driver = MockDriver::always_succeed();

    let msg = sample_msg("sess-t006", "handle gracefully");
    repo.insert(&msg).await.expect("insert");

    let queued = repo.fetch_unconsumed("sess-t006").await.expect("fetch");
    assert_eq!(queued.len(), 1);

    // Drop the table so that mark_consumed fails with a DB error.
    sqlx::query("DROP TABLE steering_message")
        .execute(db.as_ref())
        .await
        .expect("drop table");

    // deliver_queued_messages must complete without panicking even when
    // mark_consumed fails because the table no longer exists.
    deliver_queued_messages("sess-t006", &queued, &driver, &repo).await;

    // Reaching this assertion confirms graceful error handling (no panic).
    // The message "stays unconsumed" because the DB UPDATE did not execute;
    // we cannot query to verify since the table was intentionally dropped.
}
