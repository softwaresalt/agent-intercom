//! Unit tests for slash command argument parsing and session resolution (Phase 13/16).
//!
//! Covers:
//! - T113: `parse_checkpoint_args` with dash-containing single arg (S081)
//! - T114: `parse_checkpoint_args` fallback to most-recent when no arg (S082)
//! - T116: `resolve_command_session` resolves Interrupted sessions by explicit ID (S083, S084)
//! - T117: `find_interrupted_by_channel` used by session-cleanup logic (S085, S086)
//! - T155: `list_all_by_channel` returns sessions across all statuses (S113, S114)
//! - T161: `format_checkpoint_help` shows `session_id` as optional (S117, S118)
//! - T163: `list_active_or_paused` includes paused sessions for display (S119, S120)

use std::sync::Arc;

use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::{db, session_repo::SessionRepo};
use agent_intercom::slack::commands::parse_checkpoint_args;

// ── T113 / S081 ────────────────────────────────────────────────────────────────

/// A single argument that contains dashes and looks like a label (not a UUID)
/// must be treated as a `session_id` (first positional arg is ALWAYS `session_id`).
///
/// This was HITL-005: the old heuristic treated "phase-13-checkpoint" as a label
/// because it contains dashes but is not long enough to be a UUID, but the correct
/// behaviour is for 1-arg to always mean `session_id`.
#[test]
fn parse_checkpoint_args_single_dash_arg_is_session_id() {
    // Any single arg is treated as session_id — no heuristic guessing.
    let (session_id, label) = parse_checkpoint_args(&["phase-13"]);
    assert_eq!(
        session_id,
        Some("phase-13"),
        "single arg must be treated as session_id regardless of content"
    );
    assert!(
        label.is_none(),
        "label must be absent when only session_id given"
    );
}

/// A dash-heavy label-like string (the old buggy case) is also treated as `session_id`.
#[test]
fn parse_checkpoint_args_long_dash_label_is_session_id() {
    let (session_id, label) = parse_checkpoint_args(&["my-checkpoint-backup-2024"]);
    assert_eq!(session_id, Some("my-checkpoint-backup-2024"));
    assert!(label.is_none());
}

// ── T114 / S082 ────────────────────────────────────────────────────────────────

/// Zero arguments: no `session_id` and no label → fall back to most-recent session.
#[test]
fn parse_checkpoint_args_no_args_is_none_none() {
    let (session_id, label) = parse_checkpoint_args(&[]);
    assert!(session_id.is_none(), "no args → session_id must be None");
    assert!(label.is_none(), "no args → label must be None");
}

/// Two arguments: first is `session_id`, second is label.
#[test]
fn parse_checkpoint_args_two_args_session_then_label() {
    let (session_id, label) = parse_checkpoint_args(&["my-session-id", "my-label"]);
    assert_eq!(session_id, Some("my-session-id"));
    assert_eq!(label, Some("my-label"));
}

/// More than two arguments: extras after position 1 are silently ignored.
#[test]
fn parse_checkpoint_args_extra_args_ignored() {
    let (session_id, label) = parse_checkpoint_args(&["sid", "lbl", "extra"]);
    assert_eq!(session_id, Some("sid"));
    assert_eq!(label, Some("lbl"));
}

// ── T116 / S083 S084 ──────────────────────────────────────────────────────────

/// `find_interrupted_by_channel` returns Interrupted sessions in a channel.
///
/// This tests the new repo method needed to support HITL-006: managing
/// sessions that became Interrupted after a server restart.
#[tokio::test]
async fn find_interrupted_by_channel_returns_interrupted_sessions() {
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&db));

    // Create and interrupt a session in the target channel.
    let mut session = Session::new(
        "U_OWNER".into(),
        "/workspace".into(),
        Some("test".into()),
        SessionMode::Remote,
    );
    session.channel_id = Some("C_TARGET".into());
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    repo.update_status(&created.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt");

    // Create an active session in a different channel (should not appear).
    let mut other = Session::new(
        "U_OTHER".into(),
        "/workspace".into(),
        None,
        SessionMode::Remote,
    );
    other.channel_id = Some("C_OTHER".into());
    let other_created = repo.create(&other).await.expect("create other");
    repo.update_status(&other_created.id, SessionStatus::Active)
        .await
        .expect("activate other");

    let interrupted = repo
        .find_interrupted_by_channel("C_TARGET")
        .await
        .expect("find_interrupted_by_channel");

    assert_eq!(interrupted.len(), 1, "should find 1 interrupted session");
    assert_eq!(interrupted[0].id, created.id);
    assert_eq!(interrupted[0].status, SessionStatus::Interrupted);
}

/// `find_interrupted_by_channel` returns empty when no interrupted sessions exist.
#[tokio::test]
async fn find_interrupted_by_channel_empty_when_none() {
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let result = repo
        .find_interrupted_by_channel("C_EMPTY")
        .await
        .expect("find_interrupted_by_channel");

    assert!(result.is_empty());
}

// ── T155 / S113 S114 ──────────────────────────────────────────────────────────

/// `list_all_by_channel` returns sessions with every status in a channel.
///
/// This supports HITL-002 / FR-048: `/arc sessions --all` should show all
/// historical sessions (active, paused, terminated, interrupted) for the
/// current channel so operators can review session history.
#[tokio::test]
async fn list_all_by_channel_returns_sessions_across_statuses() {
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&db));
    let channel = "C_HIST";

    // Active session.
    let mut s1 = Session::new(
        "U1".into(),
        "/ws".into(),
        Some("active prompt".into()),
        SessionMode::Remote,
    );
    s1.channel_id = Some(channel.into());
    let s1 = repo.create(&s1).await.expect("create s1");
    repo.update_status(&s1.id, SessionStatus::Active)
        .await
        .expect("activate s1");

    // Terminated session.
    let mut s2 = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    s2.channel_id = Some(channel.into());
    let s2 = repo.create(&s2).await.expect("create s2");
    repo.update_status(&s2.id, SessionStatus::Active)
        .await
        .expect("activate s2");
    repo.set_terminated(&s2.id, SessionStatus::Terminated)
        .await
        .expect("terminate s2");

    // Interrupted session.
    let mut s3 = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    s3.channel_id = Some(channel.into());
    let s3 = repo.create(&s3).await.expect("create s3");
    repo.update_status(&s3.id, SessionStatus::Active)
        .await
        .expect("activate s3");
    repo.set_terminated(&s3.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt s3");

    // Session in a different channel — must not appear.
    let mut other = Session::new("U2".into(), "/ws".into(), None, SessionMode::Remote);
    other.channel_id = Some("C_OTHER".into());
    let other = repo.create(&other).await.expect("create other");
    repo.update_status(&other.id, SessionStatus::Active)
        .await
        .expect("activate other");

    let all = repo
        .list_all_by_channel(channel)
        .await
        .expect("list_all_by_channel");
    assert_eq!(
        all.len(),
        3,
        "must return all 3 channel sessions regardless of status"
    );
}

/// `list_all_by_channel` returns empty for a channel with no sessions.
#[tokio::test]
async fn list_all_by_channel_empty_for_unknown_channel() {
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&db));

    let all = repo
        .list_all_by_channel("C_UNKNOWN")
        .await
        .expect("list_all_by_channel");
    assert!(all.is_empty());
}

// ── T161 / S117 S118 ──────────────────────────────────────────────────────────

/// `format_checkpoint_help` shows `[session_id]` as optional (bracket notation).
///
/// Verifies HITL-004 / FR-050: the help text must accurately describe that
/// `session_id` is optional (falls back to most-recent when omitted).
#[test]
fn checkpoint_help_shows_session_id_as_optional_brackets() {
    use agent_intercom::slack::commands::format_checkpoint_help;
    let help = format_checkpoint_help("/arc");
    assert!(
        help.contains("[session_id]"),
        "help text must show session_id as optional using bracket notation: got: {help}"
    );
    assert!(
        !help.contains("<session_id>"),
        "help text must NOT mark session_id as required with angle brackets"
    );
}

// ── T163 / S119 S120 ──────────────────────────────────────────────────────────

/// `list_active_or_paused` returns paused sessions alongside active ones.
///
/// Verifies HITL-008 / FR-051: the sessions listing must show paused sessions
/// with a visual indicator. `handle_sessions` uses `list_active_or_paused`
/// (after T164) so paused sessions appear in the default listing.
#[tokio::test]
async fn list_active_or_paused_includes_paused_sessions() {
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let repo = SessionRepo::new(Arc::clone(&db));

    // Create and activate a session, then pause it.
    let mut s = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    s.channel_id = Some("C_PAUSE".into());
    let s = repo.create(&s).await.expect("create");
    repo.update_status(&s.id, SessionStatus::Active)
        .await
        .expect("activate");
    repo.update_status(&s.id, SessionStatus::Paused)
        .await
        .expect("pause");

    // Create a terminated session — must NOT appear.
    let mut t = Session::new("U1".into(), "/ws".into(), None, SessionMode::Remote);
    t.channel_id = Some("C_PAUSE".into());
    let t = repo.create(&t).await.expect("create terminated");
    repo.update_status(&t.id, SessionStatus::Active)
        .await
        .expect("activate terminated");
    repo.set_terminated(&t.id, SessionStatus::Terminated)
        .await
        .expect("terminate");

    let visible = repo
        .list_active_or_paused()
        .await
        .expect("list_active_or_paused");
    assert_eq!(visible.len(), 1, "must include the paused session");
    assert_eq!(visible[0].status, SessionStatus::Paused);
}
