//! Unit tests for Block Kit session lifecycle message builders.
//!
//! Covers `session_started_blocks()` and `session_ended_blocks()` for both
//! MCP and ACP protocol modes.
//!
//! Scenario references: S-T1-005 (FR-001)

use agent_intercom::models::session::{
    ConnectivityStatus, ProtocolMode, Session, SessionMode, SessionStatus,
};
use agent_intercom::slack::blocks;
use chrono::{TimeZone, Utc};

/// Construct a minimal test `Session` with the given protocol and mode.
fn make_session(protocol_mode: ProtocolMode, mode: SessionMode) -> Session {
    Session {
        id: "abc12345-0000-0000-0000-000000000000".to_owned(),
        owner_user_id: "U123".to_owned(),
        workspace_root: "D:\\projects\\myapp".to_owned(),
        status: SessionStatus::Active,
        prompt: None,
        mode,
        created_at: Utc.with_ymd_and_hms(2026, 3, 9, 10, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2026, 3, 9, 10, 0, 0).unwrap(),
        last_tool: None,
        nudge_count: 0,
        stall_paused: false,
        terminated_at: None,
        progress_snapshot: None,
        protocol_mode,
        channel_id: None,
        thread_ts: None,
        connectivity_status: ConnectivityStatus::Online,
        last_activity_at: None,
        restart_of: None,
        agent_session_id: None,
        title: None,
    }
}

// ── session_started_blocks ─────────────────────────────────────────────────────

/// S-T1-005a — Output contains the first 8 characters of the session ID.
#[test]
fn session_started_blocks_contains_short_id() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("abc12345"),
        "short ID prefix must appear; got: {json}"
    );
}

/// S-T1-005b — MCP protocol mode is labelled "MCP".
#[test]
fn session_started_blocks_mcp_protocol_label() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(json.contains("MCP"), "MCP protocol label must appear");
}

/// S-T1-005c — ACP protocol mode is labelled "ACP".
#[test]
fn session_started_blocks_acp_protocol_label() {
    let session = make_session(ProtocolMode::Acp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(json.contains("ACP"), "ACP protocol label must appear");
}

/// S-T1-005d — Operational mode "remote" appears in the block text.
#[test]
fn session_started_blocks_remote_mode_label() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("remote"),
        "operational mode 'remote' must appear"
    );
}

/// S-T1-005e — Operational mode "local" appears in the block text.
#[test]
fn session_started_blocks_local_mode_label() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Local);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("local"),
        "operational mode 'local' must appear"
    );
}

/// S-T1-005f — Operational mode "hybrid" appears in the block text.
#[test]
fn session_started_blocks_hybrid_mode_label() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Hybrid);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("hybrid"),
        "operational mode 'hybrid' must appear"
    );
}

/// S-T1-005g — The workspace root path appears in the block text.
#[test]
fn session_started_blocks_contains_workspace_root() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("D:\\\\projects\\\\myapp") || json.contains("D:\\projects\\myapp"),
        "workspace root must appear in block text"
    );
}

/// S-T1-005h — The timestamp is formatted as "YYYY-MM-DD HH:MM UTC".
#[test]
fn session_started_blocks_contains_timestamp() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    // created_at is 2026-03-09 10:00 UTC
    assert!(
        json.contains("2026-03-09 10:00 UTC"),
        "timestamp in YYYY-MM-DD HH:MM UTC format must appear; got: {json}"
    );
}

/// S-T1-005i — MCP sessions use the 🚀 emoji (U+1F680).
#[test]
fn session_started_blocks_mcp_uses_rocket_emoji() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains('\u{1f680}'),
        "MCP sessions must use 🚀 emoji (U+1F680)"
    );
}

/// S-T1-005j — ACP sessions use the 🤖 emoji (U+1F916).
#[test]
fn session_started_blocks_acp_uses_robot_emoji() {
    let session = make_session(ProtocolMode::Acp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains('\u{1f916}'),
        "ACP sessions must use 🤖 emoji (U+1F916)"
    );
}

/// S-T1-005k — `session_started_blocks` returns exactly one block.
#[test]
fn session_started_blocks_returns_one_block() {
    let session = make_session(ProtocolMode::Mcp, SessionMode::Remote);
    let blks = blocks::session_started_blocks(&session);
    assert_eq!(
        blks.len(),
        1,
        "session_started_blocks must return exactly 1 block"
    );
}

// ── session_ended_blocks ───────────────────────────────────────────────────────

/// Construct a terminated session with `terminated_at` set for duration tests.
fn make_terminated_session(duration_secs: i64) -> Session {
    let created = Utc.with_ymd_and_hms(2026, 3, 9, 10, 0, 0).unwrap();
    let ended = created + chrono::Duration::seconds(duration_secs);
    Session {
        id: "abc12345-0000-0000-0000-000000000000".to_owned(),
        owner_user_id: "U123".to_owned(),
        workspace_root: "/workspace".to_owned(),
        status: SessionStatus::Terminated,
        prompt: None,
        mode: SessionMode::Remote,
        created_at: created,
        updated_at: ended,
        last_tool: None,
        nudge_count: 0,
        stall_paused: false,
        terminated_at: Some(ended),
        progress_snapshot: None,
        protocol_mode: ProtocolMode::Mcp,
        channel_id: None,
        thread_ts: None,
        connectivity_status: ConnectivityStatus::Online,
        last_activity_at: None,
        restart_of: None,
        agent_session_id: None,
        title: None,
    }
}

/// S-T1-005l — `session_ended_blocks` contains the short session ID prefix.
#[test]
fn session_ended_blocks_contains_short_id() {
    let session = make_terminated_session(300);
    let blks = blocks::session_ended_blocks(&session, "user request");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("abc12345"),
        "short ID prefix must appear in session ended blocks"
    );
}

/// S-T1-005m — "terminated" status label appears for a terminated session.
#[test]
fn session_ended_blocks_contains_status_label() {
    let session = make_terminated_session(300);
    let blks = blocks::session_ended_blocks(&session, "completed normally");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("terminated"),
        "status label 'terminated' must appear"
    );
}

/// S-T1-005n — The termination reason appears in the block text.
#[test]
fn session_ended_blocks_contains_reason() {
    let session = make_terminated_session(300);
    let reason = "user request";
    let blks = blocks::session_ended_blocks(&session, reason);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(reason),
        "termination reason must appear in session ended blocks"
    );
}

/// S-T1-005o — Duration < 60 seconds is displayed as seconds.
#[test]
fn session_ended_blocks_duration_seconds() {
    let session = make_terminated_session(45);
    let blks = blocks::session_ended_blocks(&session, "done");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("45s"),
        "45-second duration must be displayed as '45s'; got: {json}"
    );
}

/// S-T1-005p — Duration ≥ 60 s and < 3600 s is displayed as "Xm Ys".
#[test]
fn session_ended_blocks_duration_minutes() {
    let session = make_terminated_session(125); // 2m 5s
    let blks = blocks::session_ended_blocks(&session, "done");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("2m 5s"),
        "125-second duration must be '2m 5s'; got: {json}"
    );
}

/// S-T1-005q — Duration ≥ 3600 s is displayed as "Xh Ym".
#[test]
fn session_ended_blocks_duration_hours() {
    let session = make_terminated_session(3_900); // 1h 5m
    let blks = blocks::session_ended_blocks(&session, "done");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("1h 5m"),
        "3900-second duration must be '1h 5m'; got: {json}"
    );
}

/// S-T1-005r — When `terminated_at` is None, "unknown" duration is displayed.
#[test]
fn session_ended_blocks_unknown_duration_when_no_terminated_at() {
    let mut session = make_terminated_session(0);
    session.terminated_at = None;
    let blks = blocks::session_ended_blocks(&session, "done");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("unknown"),
        "duration must be 'unknown' when terminated_at is None"
    );
}

/// S-T1-005s — "interrupted" status label for an interrupted session.
#[test]
fn session_ended_blocks_interrupted_status_label() {
    let mut session = make_terminated_session(60);
    session.status = SessionStatus::Interrupted;
    let blks = blocks::session_ended_blocks(&session, "crash");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("interrupted"),
        "status label 'interrupted' must appear for interrupted sessions"
    );
}

/// S-T1-005t — `session_ended_blocks` returns exactly one block.
#[test]
fn session_ended_blocks_returns_one_block() {
    let session = make_terminated_session(300);
    let blks = blocks::session_ended_blocks(&session, "done");
    assert_eq!(
        blks.len(),
        1,
        "session_ended_blocks must return exactly 1 block"
    );
}
