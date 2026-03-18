//! Unit tests for Block Kit stall alert message builders.
//!
//! Covers `stall_alert_blocks()`, `stall_alert_message()`, and
//! `nudge_buttons()` across representative idle durations.
//!
//! Scenario references: S-T1-003 (FR-001)

use agent_intercom::slack::blocks;

// ── stall_alert_message ───────────────────────────────────────────────────────

/// S-T1-003a — Message contains the session ID.
#[test]
fn stall_alert_message_contains_session_id() {
    let msg = blocks::stall_alert_message("stall:def456", 300);
    assert!(
        msg.contains("stall:def456"),
        "stall alert message must contain session ID"
    );
}

/// S-T1-003b — Duration ≥ 60 seconds is displayed in minutes.
#[test]
fn stall_alert_message_displays_minutes_when_at_least_60_seconds() {
    let msg = blocks::stall_alert_message("sess:1", 300);
    assert!(
        msg.contains("5 min"),
        "300 seconds must be displayed as '5 min'; got: {msg}"
    );
}

/// S-T1-003c — Duration < 60 seconds is displayed with the `s` suffix.
#[test]
fn stall_alert_message_displays_seconds_when_under_60() {
    let msg = blocks::stall_alert_message("sess:2", 45);
    assert!(
        msg.contains("45s"),
        "45 seconds must be displayed as '45s'; got: {msg}"
    );
}

/// S-T1-003d — Message contains the ⚠️ warning emoji.
#[test]
fn stall_alert_message_contains_warning_emoji() {
    let msg = blocks::stall_alert_message("sess:3", 120);
    assert!(
        msg.contains('\u{26a0}'),
        "message must contain ⚠️ (U+26A0) warning emoji"
    );
}

/// S-T1-003e — Exactly 60 seconds is displayed as "1 min" (boundary condition).
#[test]
fn stall_alert_message_boundary_60_seconds_is_one_minute() {
    let msg = blocks::stall_alert_message("sess:4", 60);
    assert!(
        msg.contains("1 min"),
        "60 seconds must be '1 min'; got: {msg}"
    );
}

/// S-T1-003f — Message includes recovery instructions (ctl commands present).
#[test]
fn stall_alert_message_includes_recovery_instructions() {
    let msg = blocks::stall_alert_message("sess:5", 200);
    assert!(
        msg.contains("agent-intercom-ctl"),
        "message must include recovery command examples"
    );
}

// ── nudge_buttons ─────────────────────────────────────────────────────────────

/// S-T1-003g — `nudge_buttons` contains the `stall_nudge` action ID.
#[test]
fn nudge_buttons_contains_nudge_action_id() {
    let blk = blocks::nudge_buttons("stall:def456");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("stall_nudge"),
        "stall_nudge action_id must appear"
    );
}

/// S-T1-003h — `nudge_buttons` contains the `stall_nudge_instruct` action ID.
#[test]
fn nudge_buttons_contains_nudge_instruct_action_id() {
    let blk = blocks::nudge_buttons("stall:def456");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("stall_nudge_instruct"),
        "stall_nudge_instruct action_id must appear"
    );
}

/// S-T1-003i — `nudge_buttons` contains the `stall_stop` action ID.
#[test]
fn nudge_buttons_contains_stop_action_id() {
    let blk = blocks::nudge_buttons("stall:def456");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("stall_stop"),
        "stall_stop action_id must appear"
    );
}

/// S-T1-003j — The button labels "Nudge", "Nudge with Instructions", and "Stop" appear.
#[test]
fn nudge_buttons_has_all_button_labels() {
    let blk = blocks::nudge_buttons("stall:def456");
    let json = serde_json::to_string(&blk).expect("serialize block");
    for label in &["Nudge", "Nudge with Instructions", "Stop"] {
        assert!(json.contains(label), "button label '{label}' must appear");
    }
}

/// S-T1-003k — The `block_id` encodes the alert ID with the `stall_` prefix.
#[test]
fn nudge_buttons_block_id_encodes_alert_id() {
    let alert_id = "stall:def456";
    let blk = blocks::nudge_buttons(alert_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    let expected = format!("stall_{alert_id}");
    assert!(
        json.contains(&expected),
        "block_id 'stall_{{alert_id}}' must appear; got: {json}"
    );
}

/// S-T1-003l — All button values equal the alert ID.
#[test]
fn nudge_buttons_values_contain_alert_id() {
    let alert_id = "stall:def456";
    let blk = blocks::nudge_buttons(alert_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    // Alert ID appears at least 3 times (once per button value) + block_id prefix
    let occurrences = json.matches(alert_id).count();
    assert!(
        occurrences >= 3,
        "alert_id must appear in all three button values; found {occurrences}"
    );
}

// ── stall_alert_blocks ────────────────────────────────────────────────────────

/// S-T1-003m — `stall_alert_blocks` produces exactly two blocks.
#[test]
fn stall_alert_blocks_returns_two_blocks() {
    let blks = blocks::stall_alert_blocks("stall:def456", 300);
    assert_eq!(
        blks.len(),
        2,
        "stall_alert_blocks must return exactly 2 blocks"
    );
}

/// S-T1-003n — First block is a section with the warning emoji.
#[test]
fn stall_alert_blocks_first_block_has_warning_emoji() {
    let blks = blocks::stall_alert_blocks("stall:def456", 300);
    let json = serde_json::to_string(&blks[0]).expect("serialize first block");
    assert!(
        json.contains('\u{26a0}'),
        "first block must contain ⚠️ warning emoji"
    );
}

/// S-T1-003o — Second block contains the nudge action buttons.
#[test]
fn stall_alert_blocks_second_block_has_nudge_buttons() {
    let blks = blocks::stall_alert_blocks("stall:def456", 300);
    let json = serde_json::to_string(&blks[1]).expect("serialize second block");
    assert!(
        json.contains("stall_nudge"),
        "second block must contain stall_nudge action_id"
    );
}

/// S-T1-003p — Session ID appears in the assembled blocks.
#[test]
fn stall_alert_blocks_contains_session_id() {
    let session_id = "stall:def456";
    let blks = blocks::stall_alert_blocks(session_id, 300);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(session_id),
        "session ID must appear in stall alert blocks"
    );
}

/// S-T1-003q — Idle duration is reflected in the alert text (300 s = 5 min).
#[test]
fn stall_alert_blocks_includes_idle_duration() {
    let blks = blocks::stall_alert_blocks("sess:timeout", 300);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("5 min"),
        "stall_alert_blocks must reflect idle duration; got: {json}"
    );
}
