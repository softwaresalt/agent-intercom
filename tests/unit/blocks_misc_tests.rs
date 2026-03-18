//! Unit tests for miscellaneous Block Kit builders.
//!
//! Covers `action_buttons()`, `approval_buttons()`, `text_section()`,
//! `wait_buttons()`, `severity_section()`, `code_snippet_blocks()`,
//! `diff_section()`, `diff_applied_section()`, `diff_conflict_section()`,
//! `diff_force_warning_section()`, `auto_approve_suggestion_button()`,
//! `slack_escape()`, and `truncate_text()`.
//!
//! Scenario references: S-T1-004, S-T1-006, S-T1-007, S-T1-008 (FR-001)

use agent_intercom::slack::blocks;

// ── wait_buttons ──────────────────────────────────────────────────────────────

/// S-T1-004a — `wait_buttons` contains the `wait_resume` action ID.
#[test]
fn wait_buttons_contains_resume_action_id() {
    let blk = blocks::wait_buttons("session:ghi012");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("wait_resume"),
        "wait_resume action_id must appear"
    );
}

/// S-T1-004b — `wait_buttons` contains the `wait_resume_instruct` action ID.
#[test]
fn wait_buttons_contains_resume_instruct_action_id() {
    let blk = blocks::wait_buttons("session:ghi012");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("wait_resume_instruct"),
        "wait_resume_instruct action_id must appear"
    );
}

/// S-T1-004c — `wait_buttons` contains the `wait_stop` action ID.
#[test]
fn wait_buttons_contains_stop_action_id() {
    let blk = blocks::wait_buttons("session:ghi012");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("wait_stop"),
        "wait_stop action_id must appear"
    );
}

/// S-T1-004d — Button labels "Resume", "Resume with Instructions", and "Stop Session" appear.
#[test]
fn wait_buttons_has_all_button_labels() {
    let blk = blocks::wait_buttons("session:ghi012");
    let json = serde_json::to_string(&blk).expect("serialize block");
    for label in &["Resume", "Resume with Instructions", "Stop Session"] {
        assert!(json.contains(label), "button label '{label}' must appear");
    }
}

/// S-T1-004e — The `block_id` encodes the session ID with the `wait_` prefix.
#[test]
fn wait_buttons_block_id_encodes_session_id() {
    let session_id = "session:ghi012";
    let blk = blocks::wait_buttons(session_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    let expected = format!("wait_{session_id}");
    assert!(
        json.contains(&expected),
        "block_id 'wait_{{session_id}}' must appear; got: {json}"
    );
}

/// S-T1-004f — All button values equal the session ID.
#[test]
fn wait_buttons_values_contain_session_id() {
    let session_id = "session:ghi012";
    let blk = blocks::wait_buttons(session_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    // Should appear at least 3 times (once per button value) + block_id prefix
    let occurrences = json.matches(session_id).count();
    assert!(
        occurrences >= 3,
        "session_id must appear in all three button values; found {occurrences}"
    );
}

// ── severity_section ──────────────────────────────────────────────────────────

/// S-T1-006a — "info" level uses ℹ️ emoji (U+2139 + U+FE0F).
#[test]
fn severity_section_info_uses_info_emoji() {
    let blk = blocks::severity_section("info", "Information message");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{2139}'),
        "info level must use ℹ️ emoji (U+2139)"
    );
}

/// S-T1-006b — "success" level uses ✅ emoji (U+2705).
#[test]
fn severity_section_success_uses_check_emoji() {
    let blk = blocks::severity_section("success", "Operation succeeded");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{2705}'),
        "success level must use ✅ emoji (U+2705)"
    );
}

/// S-T1-006c — "warning" level uses ⚠️ emoji (U+26A0).
#[test]
fn severity_section_warning_uses_warning_emoji() {
    let blk = blocks::severity_section("warning", "A warning occurred");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{26a0}'),
        "warning level must use ⚠️ emoji (U+26A0)"
    );
}

/// S-T1-006d — "error" level uses ❌ emoji (U+274C).
#[test]
fn severity_section_error_uses_cross_emoji() {
    let blk = blocks::severity_section("error", "An error occurred");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{274c}'),
        "error level must use ❌ emoji (U+274C)"
    );
}

/// S-T1-006e — The message text appears in the serialised block.
#[test]
fn severity_section_contains_message_text() {
    let message = "Unique test message content";
    let blk = blocks::severity_section("info", message);
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains(message),
        "message text must appear in serialised severity section"
    );
}

/// S-T1-006f — Unknown severity level falls back to the ℹ️ emoji.
#[test]
fn severity_section_unknown_level_uses_info_emoji() {
    let blk = blocks::severity_section("debug", "Debug message");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{2139}'),
        "unknown level must fall back to ℹ️ emoji"
    );
}

// ── code_snippet_blocks ────────────────────────────────────────────────────────

/// S-T1-008a — `code_snippet_blocks` returns at least one block (the header).
#[test]
fn code_snippet_blocks_returns_at_least_one_block() {
    let blks = blocks::code_snippet_blocks(&[("label", "rust", "fn main() {}")]);
    assert!(
        !blks.is_empty(),
        "code_snippet_blocks must return at least one block"
    );
}

/// S-T1-008b — Header block contains the "Code snippets for review" text.
#[test]
fn code_snippet_blocks_header_text_present() {
    let blks = blocks::code_snippet_blocks(&[("label", "rust", "fn main() {}")]);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("Code snippets for review"),
        "header must contain 'Code snippets for review'"
    );
}

/// S-T1-008c — The snippet label appears as a bold header in the block text.
#[test]
fn code_snippet_blocks_contains_label() {
    let blks = blocks::code_snippet_blocks(&[("my_module.rs", "rust", "fn main() {}")]);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("my_module.rs"),
        "snippet label must appear in serialised blocks"
    );
}

/// S-T1-008d — The snippet content appears in a code fence.
#[test]
fn code_snippet_blocks_contains_content_in_code_fence() {
    let content = "fn main() { println!(\"hello\"); }";
    let blks = blocks::code_snippet_blocks(&[("label", "rust", content)]);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("fn main"),
        "snippet content must appear in serialised blocks"
    );
    assert!(
        json.contains("```"),
        "content must be wrapped in code fence delimiters"
    );
}

/// S-T1-008e — Two snippets produce a divider block between them.
#[test]
fn code_snippet_blocks_two_snippets_have_divider() {
    let blks = blocks::code_snippet_blocks(&[
        ("file_a.rs", "rust", "let x = 1;"),
        ("file_b.rs", "rust", "let y = 2;"),
    ]);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("divider"),
        "multiple snippets must be separated by divider blocks"
    );
}

/// S-T1-008f — Language hint is included in the code fence when non-empty.
#[test]
fn code_snippet_blocks_includes_language_hint() {
    let blks = blocks::code_snippet_blocks(&[("label", "python", "print('hello')")]);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("```python"),
        "language hint 'python' must appear in code fence"
    );
}

/// S-T1-008g — Content exceeding 2600 characters is truncated with "(truncated)" notice.
#[test]
fn code_snippet_blocks_truncates_long_content() {
    let long_content: String = "x".repeat(3_000);
    let blks = blocks::code_snippet_blocks(&[("large_file.rs", "rust", &long_content)]);
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("truncated"),
        "long content must be truncated with a notice"
    );
}

// ── diff_section ──────────────────────────────────────────────────────────────

/// S-T1-008h — `diff_section` wraps the diff in a code fence.
#[test]
fn diff_section_wraps_content_in_code_fence() {
    let diff = "- old line\n+ new line";
    let blk = blocks::diff_section(diff);
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(json.contains("```"), "diff must be wrapped in code fence");
    assert!(
        json.contains("old line"),
        "diff content must appear in serialised block"
    );
}

// ── diff_applied_section ──────────────────────────────────────────────────────

/// S-T1-008i — `diff_applied_section` uses the success emoji (U+2705).
#[test]
fn diff_applied_section_uses_success_emoji() {
    let blk = blocks::diff_applied_section("src/main.rs", 1_024);
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{2705}'),
        "diff_applied_section must use ✅ emoji"
    );
}

/// S-T1-008j — `diff_applied_section` includes the file path and byte count.
#[test]
fn diff_applied_section_contains_file_path_and_bytes() {
    let blk = blocks::diff_applied_section("src/main.rs", 512);
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(json.contains("src/main.rs"), "file path must appear");
    assert!(json.contains("512"), "byte count must appear");
}

// ── diff_conflict_section ─────────────────────────────────────────────────────

/// S-T1-008k — `diff_conflict_section` uses the error emoji (U+274C).
#[test]
fn diff_conflict_section_uses_error_emoji() {
    let blk = blocks::diff_conflict_section("src/lib.rs");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{274c}'),
        "diff_conflict_section must use ❌ emoji"
    );
}

/// S-T1-008l — `diff_conflict_section` includes the file path.
#[test]
fn diff_conflict_section_contains_file_path() {
    let blk = blocks::diff_conflict_section("src/lib.rs");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(json.contains("src/lib.rs"), "file path must appear");
}

/// S-T1-008m — `diff_conflict_section` mentions "conflict" or "changed".
#[test]
fn diff_conflict_section_describes_conflict() {
    let blk = blocks::diff_conflict_section("src/lib.rs");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("conflict") || json.contains("changed"),
        "diff_conflict_section must describe the conflict"
    );
}

// ── diff_force_warning_section ────────────────────────────────────────────────

/// S-T1-008n — `diff_force_warning_section` uses the warning emoji (U+26A0).
#[test]
fn diff_force_warning_section_uses_warning_emoji() {
    let blk = blocks::diff_force_warning_section("src/main.rs");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains('\u{26a0}'),
        "diff_force_warning_section must use ⚠️ emoji"
    );
}

/// S-T1-008o — `diff_force_warning_section` includes the file path.
#[test]
fn diff_force_warning_section_contains_file_path() {
    let blk = blocks::diff_force_warning_section("src/main.rs");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(json.contains("src/main.rs"), "file path must appear");
}

// ── auto_approve_suggestion_button ────────────────────────────────────────────

/// S-T1-008p — `auto_approve_suggestion_button` contains the `auto_approve_add` action ID.
#[test]
fn auto_approve_suggestion_button_has_add_action_id() {
    let blk = blocks::auto_approve_suggestion_button("cargo test");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("auto_approve_add"),
        "auto_approve_add action_id must appear"
    );
}

/// S-T1-008q — `auto_approve_suggestion_button` contains the `auto_approve_dismiss` action ID.
#[test]
fn auto_approve_suggestion_button_has_dismiss_action_id() {
    let blk = blocks::auto_approve_suggestion_button("cargo test");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("auto_approve_dismiss"),
        "auto_approve_dismiss action_id must appear"
    );
}

/// S-T1-008r — Both buttons carry the command as their value.
#[test]
fn auto_approve_suggestion_button_values_contain_command() {
    let command = "cargo test";
    let blk = blocks::auto_approve_suggestion_button(command);
    let json = serde_json::to_string(&blk).expect("serialize block");
    let occurrences = json.matches(command).count();
    assert!(
        occurrences >= 2,
        "command must appear in both button values; found {occurrences}"
    );
}

// ── slack_escape ──────────────────────────────────────────────────────────────

/// Ampersands are escaped to `&amp;`.
#[test]
fn slack_escape_ampersand() {
    assert_eq!(blocks::slack_escape("a & b"), "a &amp; b");
}

/// Less-than is escaped to `&lt;`.
#[test]
fn slack_escape_less_than() {
    assert_eq!(blocks::slack_escape("a < b"), "a &lt; b");
}

/// Greater-than is escaped to `&gt;`.
#[test]
fn slack_escape_greater_than() {
    assert_eq!(blocks::slack_escape("a > b"), "a &gt; b");
}

/// Plain text without special characters is returned unchanged.
#[test]
fn slack_escape_plain_text_unchanged() {
    let plain = "hello world 123";
    assert_eq!(blocks::slack_escape(plain), plain);
}

// ── truncate_text ─────────────────────────────────────────────────────────────

/// Text at or below `max_len` is returned unchanged.
#[test]
fn truncate_text_short_text_unchanged() {
    assert_eq!(blocks::truncate_text("hello", 10), "hello");
}

/// Text equal to `max_len` is returned unchanged.
#[test]
fn truncate_text_exact_length_unchanged() {
    assert_eq!(blocks::truncate_text("hello", 5), "hello");
}

/// Text exceeding `max_len` is truncated with ellipsis.
#[test]
fn truncate_text_long_text_truncated_with_ellipsis() {
    let result = blocks::truncate_text("hello world", 8);
    assert!(
        result.ends_with("..."),
        "truncated text must end with '...'"
    );
    assert!(
        result.len() <= 8,
        "truncated text must not exceed max_len bytes"
    );
}

// ── text_section — SC-001 direct coverage ─────────────────────────────────────

/// `text_section` produces a section block containing the supplied text.
#[test]
fn text_section_contains_supplied_text() {
    let content = "Hello, operator!";
    let blk = blocks::text_section(content);
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains(content),
        "text_section must contain the supplied text; got: {json}"
    );
}

/// `text_section` serialises as a `"section"` type block.
#[test]
fn text_section_is_section_type() {
    let blk = blocks::text_section("test");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("\"type\":\"section\"") || json.contains("\"type\": \"section\""),
        "text_section must produce a section block; got: {json}"
    );
}

// ── action_buttons — SC-001 direct coverage ───────────────────────────────────

/// `action_buttons` serialises with the supplied `block_id`.
#[test]
fn action_buttons_encodes_block_id() {
    let blk = blocks::action_buttons(
        "my_block",
        &[("act_1", "Label 1", "val1"), ("act_2", "Label 2", "val2")],
    );
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("my_block"),
        "action_buttons must encode the block_id; got: {json}"
    );
}

/// `action_buttons` serialises all supplied `action_id` values.
#[test]
fn action_buttons_encodes_all_action_ids() {
    let blk = blocks::action_buttons(
        "blk",
        &[("id_alpha", "Alpha", "v1"), ("id_beta", "Beta", "v2")],
    );
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("id_alpha"),
        "action_id 'id_alpha' must appear"
    );
    assert!(json.contains("id_beta"), "action_id 'id_beta' must appear");
}

/// `action_buttons` serialises all supplied button labels.
#[test]
fn action_buttons_encodes_all_labels() {
    let blk = blocks::action_buttons(
        "blk",
        &[("a1", "Accept All", "v1"), ("a2", "Reject All", "v2")],
    );
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("Accept All"),
        "label 'Accept All' must appear"
    );
    assert!(
        json.contains("Reject All"),
        "label 'Reject All' must appear"
    );
}

/// `action_buttons` serialises all supplied button values.
#[test]
fn action_buttons_encodes_all_values() {
    let blk = blocks::action_buttons(
        "blk",
        &[("a1", "L1", "value_one"), ("a2", "L2", "value_two")],
    );
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(json.contains("value_one"), "value 'value_one' must appear");
    assert!(json.contains("value_two"), "value 'value_two' must appear");
}

// ── approval_buttons — SC-001 direct coverage ─────────────────────────────────

/// `approval_buttons` serialises with the `approve_accept` and `approve_reject` action IDs.
#[test]
fn approval_buttons_has_correct_action_ids() {
    let blk = blocks::approval_buttons("req:direct-001");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("approve_accept"),
        "approve_accept must appear in approval_buttons"
    );
    assert!(
        json.contains("approve_reject"),
        "approve_reject must appear in approval_buttons"
    );
}

/// `approval_buttons` encodes the `block_id` as `"approval_{{request_id}}"`.
#[test]
fn approval_buttons_block_id_format() {
    let request_id = "req:direct-001";
    let blk = blocks::approval_buttons(request_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    let expected = format!("approval_{request_id}");
    assert!(
        json.contains(&expected),
        "approval_buttons block_id must be 'approval_{{request_id}}'"
    );
}
