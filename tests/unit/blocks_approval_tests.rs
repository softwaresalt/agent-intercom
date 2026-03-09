//! Unit tests for Block Kit approval message builders.
//!
//! Covers `command_approval_blocks()` (S-T1-001) and `build_approval_blocks()`
//! including risk-level emoji, diff inline/truncated rendering, and button
//! structure.
//!
//! Scenario references: S-T1-001 (FR-001, FR-009)

use agent_intercom::models::approval::RiskLevel;
use agent_intercom::slack::blocks;

// ── command_approval_blocks ───────────────────────────────────────────────────

/// S-T1-001a — `command_approval_blocks` produces exactly two blocks:
/// one section and one actions block.
#[test]
fn command_approval_blocks_returns_two_blocks() {
    let blocks = blocks::command_approval_blocks("cargo test", "req:abc123");
    assert_eq!(blocks.len(), 2, "expected exactly 2 blocks");
}

/// S-T1-001b — The first block contains the 🔐 emoji (U+1F510).
#[test]
fn command_approval_blocks_contains_lock_emoji() {
    let blks = blocks::command_approval_blocks("cargo test", "req:abc123");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    // 🔐 = U+1F510 — serialised as \u{1f510} or the raw UTF-8 character
    assert!(
        json.contains('\u{1f510}'),
        "lock emoji (🔐) must appear in first block; got: {json}"
    );
}

/// S-T1-001c — The first block contains the command text in a code fence.
#[test]
fn command_approval_blocks_contains_command_in_code_fence() {
    let command = "cargo build --release";
    let blks = blocks::command_approval_blocks(command, "req:abc123");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(command),
        "command text must appear in serialised blocks"
    );
    assert!(
        json.contains("```"),
        "command must be wrapped in a code fence"
    );
}

/// S-T1-001d — The actions block contains an `approve_accept` action ID.
#[test]
fn command_approval_blocks_has_approve_accept_action_id() {
    let blks = blocks::command_approval_blocks("ls -la", "req:abc123");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("approve_accept"),
        "approve_accept action_id must appear in serialised blocks"
    );
}

/// S-T1-001e — The actions block contains an `approve_reject` action ID.
#[test]
fn command_approval_blocks_has_approve_reject_action_id() {
    let blks = blocks::command_approval_blocks("ls -la", "req:abc123");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("approve_reject"),
        "approve_reject action_id must appear in serialised blocks"
    );
}

/// S-T1-001f — Both button values contain the request ID.
#[test]
fn command_approval_blocks_button_values_contain_request_id() {
    let request_id = "req:abc123";
    let blks = blocks::command_approval_blocks("echo hello", request_id);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    // request_id must appear at least twice — once per button value
    let occurrences = json.matches(request_id).count();
    assert!(
        occurrences >= 2,
        "request_id must appear in both button values; found {occurrences} occurrences"
    );
}

/// S-T1-001g — The actions `block_id` encodes the request ID.
#[test]
fn command_approval_blocks_block_id_encodes_request_id() {
    let request_id = "req:abc123";
    let blks = blocks::command_approval_blocks("echo hello", request_id);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    let expected_block_id = format!("approval_{request_id}");
    assert!(
        json.contains(&expected_block_id),
        "block_id 'approval_{{request_id}}' must appear in serialised blocks"
    );
}

/// S-T1-001h — "Terminal command approval requested" header text is present.
#[test]
fn command_approval_blocks_contains_header_text() {
    let blks = blocks::command_approval_blocks("git push", "req:xyz");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("Terminal command approval requested"),
        "header text must appear in first block"
    );
}

// ── build_approval_blocks ─────────────────────────────────────────────────────

/// S-T1-001i — Low risk uses the 🟢 emoji (U+1F7E2).
#[test]
fn build_approval_blocks_low_risk_uses_green_emoji() {
    let blks = blocks::build_approval_blocks(
        "My title",
        None,
        "- one\n+ two",
        "src/lib.rs",
        RiskLevel::Low,
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains('\u{1f7e2}'),
        "low risk must use 🟢 emoji (U+1F7E2)"
    );
}

/// S-T1-001j — High risk uses the 🟡 emoji (U+1F7E1).
#[test]
fn build_approval_blocks_high_risk_uses_yellow_emoji() {
    let blks = blocks::build_approval_blocks(
        "My title",
        None,
        "- one\n+ two",
        "src/lib.rs",
        RiskLevel::High,
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains('\u{1f7e1}'),
        "high risk must use 🟡 emoji (U+1F7E1)"
    );
}

/// S-T1-001k — Critical risk uses the 🔴 emoji (U+1F534).
#[test]
fn build_approval_blocks_critical_risk_uses_red_emoji() {
    let blks = blocks::build_approval_blocks(
        "My title",
        None,
        "- one\n+ two",
        "src/lib.rs",
        RiskLevel::Critical,
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains('\u{1f534}'),
        "critical risk must use 🔴 emoji (U+1F534)"
    );
}

/// S-T1-001l — The title appears in the header block.
#[test]
fn build_approval_blocks_includes_title() {
    let title = "Add error handling to parser";
    let blks = blocks::build_approval_blocks(title, None, "diff", "src/lib.rs", RiskLevel::Low);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(title),
        "title must appear in serialised approval blocks"
    );
}

/// S-T1-001m — The file path appears in the header block.
#[test]
fn build_approval_blocks_includes_file_path() {
    let file_path = "src/parser/mod.rs";
    let blks = blocks::build_approval_blocks("My change", None, "diff", file_path, RiskLevel::Low);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(file_path),
        "file path must appear in serialised approval blocks"
    );
}

/// S-T1-001n — Short diffs (≤ `INLINE_DIFF_THRESHOLD` lines) are rendered inline.
#[test]
fn build_approval_blocks_short_diff_rendered_inline() {
    let short_diff = "- old line\n+ new line";
    let blks = blocks::build_approval_blocks("title", None, short_diff, "file.rs", RiskLevel::Low);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("old line"),
        "short diff must be inlined in the block payload"
    );
}

/// S-T1-001o — Long diffs (> `INLINE_DIFF_THRESHOLD` lines) show a line-count indicator.
#[test]
fn build_approval_blocks_long_diff_shows_line_count() {
    // Create a diff with more than INLINE_DIFF_THRESHOLD lines
    let long_diff: String = (0..=blocks::INLINE_DIFF_THRESHOLD)
        .map(|i| format!("+ line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let blks = blocks::build_approval_blocks("title", None, &long_diff, "file.rs", RiskLevel::Low);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("lines"),
        "long diff must show a line-count indicator"
    );
    // Diff content should NOT be inlined
    assert!(
        !json.contains("line 0"),
        "long diff content must not be inlined"
    );
}

/// S-T1-001p — Optional description is included when `Some`.
#[test]
fn build_approval_blocks_description_included_when_some() {
    let description = "This change fixes the bug reported in issue #42";
    let blks = blocks::build_approval_blocks(
        "title",
        Some(description),
        "diff",
        "file.rs",
        RiskLevel::Low,
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(description),
        "description must appear in serialised blocks when Some"
    );
}

/// S-T1-001q — Block count is 2 when description is None and diff is short (header + diff).
#[test]
fn build_approval_blocks_no_description_yields_two_blocks() {
    let blks = blocks::build_approval_blocks("title", None, "- a\n+ b", "file.rs", RiskLevel::Low);
    assert_eq!(
        blks.len(),
        2,
        "expected 2 blocks when description is None and diff is short"
    );
}

/// S-T1-001r — Block count is 3 when description is Some and diff is short (header + desc + diff).
#[test]
fn build_approval_blocks_with_description_yields_three_blocks() {
    let blks = blocks::build_approval_blocks(
        "title",
        Some("description"),
        "- a\n+ b",
        "file.rs",
        RiskLevel::Low,
    );
    assert_eq!(
        blks.len(),
        3,
        "expected 3 blocks when description is Some and diff is short"
    );
}

/// S-T1-001s — `slack_escape` is applied to the title so Slack entities are safe.
#[test]
fn build_approval_blocks_title_is_escaped() {
    let blks = blocks::build_approval_blocks(
        "Changes <script> & more",
        None,
        "diff",
        "file.rs",
        RiskLevel::Low,
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    // The raw characters should be escaped in the output
    assert!(
        !json.contains("<script>"),
        "raw HTML characters in title must be escaped"
    );
    assert!(
        json.contains("&lt;script&gt;"),
        "title angle brackets must be escaped to &lt;/&gt;"
    );
}
