//! Integration tests for ACP event thread management and concurrent/lifecycle
//! behaviour (Phase 5 — S036–S041, Phase 6 — S047–S054, S067–S068).
//!
//! Validates the thread continuity contract: ACP event handlers create a new
//! Slack thread when a session has no thread anchor, and reply within the
//! existing thread when one is already established. The persistence layer
//! (`SessionRepo::set_thread_ts`) uses a `WHERE thread_ts IS NULL` guard for
//! idempotent first-write semantics.
//!
//! Also validates concurrent event processing, lifecycle edge cases, and
//! DB-level ordering contracts for the ACP event pipeline.

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

// ── S047: Two clearance requests for same session — separate records ───────────

/// S047 — Two `ClearanceRequested` events for the same session with different
/// `request_id` values produce two distinct `ApprovalRequest` rows in the DB.
/// Each has a unique ID, independent status, and does not overwrite the other.
#[tokio::test]
async fn two_clearance_requests_for_same_session_create_separate_records() {
    use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;

    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));

    let session = new_active_session(&session_repo, "/tmp/ws-s047").await;

    // Simulate two rapid ClearanceRequested events with different request_ids.
    let mut req1 = ApprovalRequest::new(
        session.id.clone(),
        "Add rate limiter".to_owned(),
        None,
        "+ rate_limit: u32,".to_owned(),
        "src/config.rs".to_owned(),
        RiskLevel::Low,
        "hash-001".to_owned(),
    );
    req1.id = "req-acp-001".to_owned();

    let mut req2 = ApprovalRequest::new(
        session.id.clone(),
        "Add retry logic".to_owned(),
        None,
        "+ retry_count: u32,".to_owned(),
        "src/models.rs".to_owned(),
        RiskLevel::High,
        "hash-002".to_owned(),
    );
    req2.id = "req-acp-002".to_owned();

    approval_repo.create(&req1).await.expect("create req1");
    approval_repo.create(&req2).await.expect("create req2");

    let fetched1 = approval_repo
        .get_by_id("req-acp-001")
        .await
        .expect("db query")
        .expect("req1 must exist");
    let fetched2 = approval_repo
        .get_by_id("req-acp-002")
        .await
        .expect("db query")
        .expect("req2 must exist");

    assert_ne!(
        fetched1.id, fetched2.id,
        "S047: two clearance requests must produce distinct approval IDs"
    );
    assert_eq!(
        fetched1.status,
        ApprovalStatus::Pending,
        "S047: req1 must be Pending"
    );
    assert_eq!(
        fetched2.status,
        ApprovalStatus::Pending,
        "S047: req2 must be Pending"
    );
    assert_eq!(
        fetched1.file_path, "src/config.rs",
        "S047: req1 file_path must not be contaminated by req2"
    );
    assert_eq!(
        fetched2.file_path, "src/models.rs",
        "S047: req2 file_path must not be contaminated by req1"
    );
}

// ── S048: Clearance and prompt interleaved — independent records ──────────────

/// S048 — A `ClearanceRequested` followed by a `PromptForwarded` for the same
/// session produce independent records with no cross-contamination:
/// one `ApprovalRequest` and one `ContinuationPrompt`, each with its own ID.
#[tokio::test]
async fn interleaved_clearance_and_prompt_produce_independent_records() {
    use agent_intercom::models::approval::{ApprovalRequest, RiskLevel};
    use agent_intercom::models::prompt::{ContinuationPrompt, PromptType};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::prompt_repo::PromptRepo;

    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));
    let prompt_repo = PromptRepo::new(Arc::clone(&db));

    let session = new_active_session(&session_repo, "/tmp/ws-s048").await;

    // Clearance event.
    let mut approval = ApprovalRequest::new(
        session.id.clone(),
        "Deploy change".to_owned(),
        None,
        "+ deploy: true".to_owned(),
        "deploy.toml".to_owned(),
        RiskLevel::High,
        "hash-s048".to_owned(),
    );
    approval.id = "req-s048-clearance".to_owned();
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    // Prompt event (interleaved).
    let mut prompt = ContinuationPrompt::new(
        session.id.clone(),
        "Should I continue the deployment?".to_owned(),
        PromptType::Continuation,
        None,
        None,
    );
    prompt.id = "req-s048-prompt".to_owned();
    prompt_repo.create(&prompt).await.expect("create prompt");

    // Verify independence.
    let fetched_approval = approval_repo
        .get_by_id("req-s048-clearance")
        .await
        .expect("db query")
        .expect("approval must exist");
    let fetched_prompt = prompt_repo
        .get_by_id("req-s048-prompt")
        .await
        .expect("db query")
        .expect("prompt must exist");

    assert_eq!(
        fetched_approval.session_id, session.id,
        "S048: approval session_id must match"
    );
    assert_eq!(
        fetched_prompt.session_id, session.id,
        "S048: prompt session_id must match"
    );
    assert_ne!(
        fetched_approval.id, fetched_prompt.id,
        "S048: approval and prompt must have distinct IDs"
    );
    assert!(
        fetched_approval.diff_content.contains("deploy"),
        "S048: approval content must not be contaminated by prompt text"
    );
    assert!(
        fetched_prompt.prompt_text.contains("deployment"),
        "S048: prompt text must not be contaminated by approval diff"
    );
}

// ── S049: Events from multiple sessions — independent records ─────────────────

/// S049 — `ClearanceRequested` events from two different sessions produce
/// independent `ApprovalRequest` rows with separate session contexts.
/// There must be no shared state leakage between sessions.
#[tokio::test]
async fn clearance_events_from_multiple_sessions_are_independent() {
    use agent_intercom::models::approval::{ApprovalRequest, RiskLevel};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;

    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));

    let session1 = new_active_session(&session_repo, "/tmp/ws-s049-a").await;
    let session2 = new_active_session(&session_repo, "/tmp/ws-s049-b").await;

    let mut req1 = ApprovalRequest::new(
        session1.id.clone(),
        "Session-1 change".to_owned(),
        None,
        "+ s1_field: bool".to_owned(),
        "s1.rs".to_owned(),
        RiskLevel::Low,
        "hash-s049-1".to_owned(),
    );
    req1.id = "req-s049-s1".to_owned();

    let mut req2 = ApprovalRequest::new(
        session2.id.clone(),
        "Session-2 change".to_owned(),
        None,
        "+ s2_field: bool".to_owned(),
        "s2.rs".to_owned(),
        RiskLevel::Low,
        "hash-s049-2".to_owned(),
    );
    req2.id = "req-s049-s2".to_owned();

    approval_repo.create(&req1).await.expect("create req1");
    approval_repo.create(&req2).await.expect("create req2");

    let fetched1 = approval_repo
        .get_by_id("req-s049-s1")
        .await
        .expect("db query")
        .expect("req1 must exist");
    let fetched2 = approval_repo
        .get_by_id("req-s049-s2")
        .await
        .expect("db query")
        .expect("req2 must exist");

    assert_eq!(
        fetched1.session_id, session1.id,
        "S049: req1 must be bound to session1"
    );
    assert_eq!(
        fetched2.session_id, session2.id,
        "S049: req2 must be bound to session2"
    );
    assert_ne!(
        fetched1.session_id, fetched2.session_id,
        "S049: sessions must not share approval records"
    );
}

// ── S050: DB persistence before driver registration — ordering contract ────────

/// S050 — The handler persists the `ApprovalRequest` to the DB before
/// registering with `AcpDriver`. If DB persistence fails, driver registration
/// is skipped (no unaudited pending state).
///
/// This test validates the persistence-first ordering by confirming that:
/// (a) a successfully created record is retrievable, and
/// (b) the returned record ID matches the `request_id` used for driver registration.
#[tokio::test]
async fn clearance_db_persistence_precedes_driver_registration() {
    use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;

    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));

    let session = new_active_session(&session_repo, "/tmp/ws-s050").await;

    let request_id = "req-s050-ordering";
    let mut approval = ApprovalRequest::new(
        session.id.clone(),
        "Config update".to_owned(),
        None,
        "+new_key = true".to_owned(),
        "config.toml".to_owned(),
        RiskLevel::Low,
        "hash-s050".to_owned(),
    );
    approval.id = request_id.to_owned();

    // Step 1: persist (simulates DB write before driver registration).
    approval_repo
        .create(&approval)
        .await
        .expect("DB must succeed before driver step");

    // Step 2: verify the record is retrievable with the same ID used for driver lookup.
    let persisted = approval_repo
        .get_by_id(request_id)
        .await
        .expect("db query")
        .expect("record must exist after persistence step");

    assert_eq!(
        persisted.id, request_id,
        "S050: persisted ID must match the request_id used for driver registration"
    );
    assert_eq!(
        persisted.status,
        ApprovalStatus::Pending,
        "S050: persisted approval must be Pending before driver registration"
    );
}

// ── S052: mpsc sender dropped — consumer exits on channel close ───────────────

/// S052 — When all `mpsc::Sender` handles are dropped, `rx.recv()` returns
/// `None`, which signals the event consumer to exit its loop gracefully.
///
/// **Scope note**: `run_acp_event_consumer` is private and requires full
/// `AppState` + `SlackService`. These tests validate the Tokio primitives the
/// consumer relies on. A separate integration test that spawns the actual
/// consumer function would require extracting it to `pub(crate)` — deferred.
#[tokio::test]
async fn mpsc_sender_dropped_causes_recv_to_return_none() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<u32>(1);
    drop(tx);
    let result = rx.recv().await;
    assert!(
        result.is_none(),
        "S052: dropped sender must cause rx.recv() to return None (consumer exits)"
    );
}

// ── S053: Cancellation token signals consumer exit ────────────────────────────

/// S053 — When the `CancellationToken` is cancelled, `.cancelled()` becomes
/// ready. In the `tokio::select!` loop, this is the `biased` first branch,
/// causing the consumer to exit cleanly.
///
/// **Scope note**: Same as S052 — tests Tokio primitives that the production
/// consumer loop depends on. The `biased` priority ordering is verified here.
#[tokio::test]
async fn cancellation_token_becomes_ready_when_cancelled() {
    use tokio_util::sync::CancellationToken;

    let ct = CancellationToken::new();
    assert!(
        !ct.is_cancelled(),
        "S053: pre-condition: token must not be cancelled"
    );
    ct.cancel();
    assert!(
        ct.is_cancelled(),
        "S053: cancelled token must be ready — signals consumer loop exit"
    );
    // Also verify the future resolves immediately (biased first-branch behavior).
    tokio::select! {
        biased;
        () = ct.cancelled() => {},
        // Timeout branch — should NOT be reached.
        () = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
            panic!("S053: ct.cancelled() must resolve immediately after cancel()");
        }
    }
}

// ── S054: Approval remains Pending after session terminated ───────────────────

/// S054 — If a session terminates after its `ApprovalRequest` was persisted,
/// the approval record remains in `Pending` state in the DB. The Slack handler
/// is responsible for returning an error to the operator when `resolve_clearance`
/// fails (session writer gone). This test validates the persistence contract.
#[tokio::test]
async fn approval_remains_pending_after_session_terminated() {
    use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
    use agent_intercom::models::session::SessionStatus;
    use agent_intercom::persistence::approval_repo::ApprovalRepo;

    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));

    let session = new_active_session(&session_repo, "/tmp/ws-s054").await;

    let mut approval = ApprovalRequest::new(
        session.id.clone(),
        "Post-termination clearance".to_owned(),
        None,
        "+ field: bool".to_owned(),
        "src/lib.rs".to_owned(),
        RiskLevel::Low,
        "hash-s054".to_owned(),
    );
    approval.id = "req-s054".to_owned();
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    // Session terminates (e.g., ACP process exits).
    session_repo
        .update_status(&session.id, SessionStatus::Terminated)
        .await
        .expect("terminate session");

    // The approval must still be queryable and in Pending state.
    let fetched = approval_repo
        .get_by_id("req-s054")
        .await
        .expect("db query")
        .expect("approval must still exist after session termination");

    assert_eq!(
        fetched.status,
        ApprovalStatus::Pending,
        "S054: approval must remain Pending after session terminates — Slack handler logs warning"
    );
}

// ── S068: Slack post succeeds but thread_ts DB persistence fails — self-healing

/// S068 — When `post_message_direct` succeeds (returns `ts`) but the subsequent
/// `set_thread_ts` DB write fails, the approval record still has `slack_ts` set
/// (via `update_slack_ts`) while `session.thread_ts` remains `None`.
///
/// On the next event for the same session, the handler sees `thread_ts=None`
/// and again uses `post_message_direct` — self-healing behaviour.
///
/// This test validates the independence of `slack_ts` on the approval record
/// and `thread_ts` on the session record.
#[tokio::test]
async fn slack_ts_and_thread_ts_are_independent_on_failure() {
    use agent_intercom::models::approval::{ApprovalRequest, RiskLevel};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;

    let db = Arc::new(db::connect_memory().await.expect("in-memory db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));

    let session = new_active_session(&session_repo, "/tmp/ws-s068").await;

    let mut approval = ApprovalRequest::new(
        session.id.clone(),
        "Thread_ts failure test".to_owned(),
        None,
        "+field".to_owned(),
        "src/lib.rs".to_owned(),
        RiskLevel::Low,
        "hash-s068".to_owned(),
    );
    approval.id = "req-s068".to_owned();
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    // Simulate: post_message_direct returns ts → update_slack_ts succeeds.
    let posted_ts = "1741234567.999000";
    approval_repo
        .update_slack_ts("req-s068", posted_ts)
        .await
        .expect("update_slack_ts must succeed");

    // Simulate: set_thread_ts fails (e.g., DB constraint or connection error).
    // We verify this by NOT calling set_thread_ts and checking session.thread_ts=None.

    let updated_approval = approval_repo
        .get_by_id("req-s068")
        .await
        .expect("db query")
        .expect("approval must exist");
    let session_after = session_repo
        .get_by_id(&session.id)
        .await
        .expect("db query")
        .expect("session must exist");

    assert_eq!(
        updated_approval.slack_ts.as_deref(),
        Some(posted_ts),
        "S068: approval.slack_ts must be set even when thread_ts write is skipped"
    );
    assert!(
        session_after.thread_ts.is_none(),
        "S068: session.thread_ts must remain None when set_thread_ts fails — self-healing on next event"
    );
}
