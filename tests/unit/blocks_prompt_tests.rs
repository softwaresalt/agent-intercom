//! Unit tests for Block Kit prompt message builders.
//!
//! Covers `build_prompt_blocks()`, `prompt_buttons()`, `prompt_type_icon()`,
//! and `prompt_type_label()` across all four prompt types.
//!
//! Scenario references: S-T1-002 (FR-001, FR-009)

use agent_intercom::models::prompt::PromptType;
use agent_intercom::slack::blocks;

// ── prompt_buttons ────────────────────────────────────────────────────────────

/// S-T1-002a — `prompt_buttons` contains the `prompt_continue` action ID.
#[test]
fn prompt_buttons_contains_continue_action_id() {
    let blk = blocks::prompt_buttons("prompt:xyz789");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("prompt_continue"),
        "prompt_continue action_id must appear"
    );
}

/// S-T1-002b — `prompt_buttons` contains the `prompt_refine` action ID.
#[test]
fn prompt_buttons_contains_refine_action_id() {
    let blk = blocks::prompt_buttons("prompt:xyz789");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("prompt_refine"),
        "prompt_refine action_id must appear"
    );
}

/// S-T1-002c — `prompt_buttons` contains the `prompt_stop` action ID.
#[test]
fn prompt_buttons_contains_stop_action_id() {
    let blk = blocks::prompt_buttons("prompt:xyz789");
    let json = serde_json::to_string(&blk).expect("serialize block");
    assert!(
        json.contains("prompt_stop"),
        "prompt_stop action_id must appear"
    );
}

/// S-T1-002d — All three button labels appear in the serialised block.
#[test]
fn prompt_buttons_has_all_button_labels() {
    let blk = blocks::prompt_buttons("prompt:xyz789");
    let json = serde_json::to_string(&blk).expect("serialize block");
    for label in &["Continue", "Refine", "Stop"] {
        assert!(json.contains(label), "button label '{label}' must appear");
    }
}

/// S-T1-002e — The `block_id` encodes the prompt ID with the `prompt_` prefix.
#[test]
fn prompt_buttons_block_id_encodes_prompt_id() {
    let prompt_id = "prompt:xyz789";
    let blk = blocks::prompt_buttons(prompt_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    let expected = format!("prompt_{prompt_id}");
    assert!(
        json.contains(&expected),
        "block_id 'prompt_{{prompt_id}}' must appear; got: {json}"
    );
}

/// S-T1-002f — All button values equal the prompt ID.
#[test]
fn prompt_buttons_values_contain_prompt_id() {
    let prompt_id = "prompt:xyz789";
    let blk = blocks::prompt_buttons(prompt_id);
    let json = serde_json::to_string(&blk).expect("serialize block");
    // Should appear at least 3 times (one per button value) plus the block_id prefix
    let occurrences = json.matches(prompt_id).count();
    assert!(
        occurrences >= 3,
        "prompt_id must appear in all three button values; found {occurrences}"
    );
}

// ── build_prompt_blocks ───────────────────────────────────────────────────────

/// S-T1-002g — `build_prompt_blocks` includes the prompt text.
#[test]
fn build_prompt_blocks_contains_prompt_text() {
    let prompt_text = "Agent is idle and awaiting instructions";
    let blks = blocks::build_prompt_blocks(
        prompt_text,
        PromptType::Continuation,
        None,
        None,
        "prompt:xyz789",
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains(prompt_text),
        "prompt text must appear in serialised blocks"
    );
}

/// S-T1-002h — `build_prompt_blocks` includes the prompt type label.
#[test]
fn build_prompt_blocks_contains_type_label() {
    let blks = blocks::build_prompt_blocks(
        "some prompt",
        PromptType::Clarification,
        None,
        None,
        "prompt:xyz789",
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("Clarification"),
        "prompt type label must appear in header block"
    );
}

/// S-T1-002i — `build_prompt_blocks` includes the three action buttons.
#[test]
fn build_prompt_blocks_includes_action_buttons() {
    let blks =
        blocks::build_prompt_blocks("text", PromptType::Continuation, None, None, "prompt:p1");
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("prompt_continue"),
        "Continue button must appear"
    );
    assert!(json.contains("prompt_refine"), "Refine button must appear");
    assert!(json.contains("prompt_stop"), "Stop button must appear");
}

/// S-T1-002j — Elapsed seconds appear in the context line when provided.
#[test]
fn build_prompt_blocks_includes_elapsed_seconds() {
    let blks = blocks::build_prompt_blocks(
        "text",
        PromptType::Continuation,
        Some(120),
        None,
        "prompt:p1",
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("120"),
        "elapsed seconds must appear in context block"
    );
}

/// S-T1-002k — Action count appears in the context line when provided.
#[test]
fn build_prompt_blocks_includes_action_count() {
    let blks = blocks::build_prompt_blocks(
        "text",
        PromptType::Continuation,
        None,
        Some(42),
        "prompt:p1",
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("42"),
        "action count must appear in context block"
    );
}

/// S-T1-002l — When neither elapsed nor count is provided, no context block is rendered
/// (total block count = 2: header + buttons).
#[test]
fn build_prompt_blocks_no_context_when_none() {
    let blks =
        blocks::build_prompt_blocks("text", PromptType::Continuation, None, None, "prompt:p1");
    // header + prompt text + buttons = 3 blocks (no context block)
    assert_eq!(
        blks.len(),
        3,
        "expected 3 blocks when elapsed/count are both None"
    );
}

/// S-T1-002m — When elapsed and count are both provided, a context block is rendered
/// (total block count = 4: header + text + context + buttons).
#[test]
fn build_prompt_blocks_has_context_block_when_both_provided() {
    let blks = blocks::build_prompt_blocks(
        "text",
        PromptType::Continuation,
        Some(60),
        Some(10),
        "prompt:p1",
    );
    assert_eq!(
        blks.len(),
        4,
        "expected 4 blocks when elapsed and count are both provided"
    );
}

// ── prompt_type_icon ──────────────────────────────────────────────────────────

/// S-T1-002n — `prompt_type_icon` returns the 🔄 emoji for Continuation.
#[test]
fn prompt_type_icon_continuation() {
    let icon = blocks::prompt_type_icon(PromptType::Continuation);
    assert!(
        icon.contains('\u{1f504}'),
        "Continuation must use 🔄 (U+1F504)"
    );
}

/// S-T1-002o — `prompt_type_icon` returns the ❓ emoji for Clarification.
#[test]
fn prompt_type_icon_clarification() {
    let icon = blocks::prompt_type_icon(PromptType::Clarification);
    assert!(
        icon.contains('\u{2753}'),
        "Clarification must use ❓ (U+2753)"
    );
}

/// S-T1-002p — `prompt_type_icon` returns the ⚠️ emoji for `ErrorRecovery`.
#[test]
fn prompt_type_icon_error_recovery() {
    let icon = blocks::prompt_type_icon(PromptType::ErrorRecovery);
    assert!(
        icon.contains('\u{26a0}'),
        "ErrorRecovery must use ⚠️ (U+26A0)"
    );
}

/// S-T1-002q — `prompt_type_icon` returns the 📊 emoji for `ResourceWarning`.
#[test]
fn prompt_type_icon_resource_warning() {
    let icon = blocks::prompt_type_icon(PromptType::ResourceWarning);
    assert!(
        icon.contains('\u{1f4ca}'),
        "ResourceWarning must use 📊 (U+1F4CA)"
    );
}

// ── prompt_type_label ─────────────────────────────────────────────────────────

/// S-T1-002r — Labels are correct for all four prompt types.
#[test]
fn prompt_type_label_all_types() {
    assert_eq!(
        blocks::prompt_type_label(PromptType::Continuation),
        "Continuation"
    );
    assert_eq!(
        blocks::prompt_type_label(PromptType::Clarification),
        "Clarification"
    );
    assert_eq!(
        blocks::prompt_type_label(PromptType::ErrorRecovery),
        "Error Recovery"
    );
    assert_eq!(
        blocks::prompt_type_label(PromptType::ResourceWarning),
        "Resource Warning"
    );
}

/// S-T1-002s — `ErrorRecovery` prompt type icon and label both appear in built blocks.
#[test]
fn build_prompt_blocks_error_recovery_icon_and_label() {
    let blks = blocks::build_prompt_blocks(
        "An error occurred",
        PromptType::ErrorRecovery,
        None,
        None,
        "prompt:err1",
    );
    let json = serde_json::to_string(&blks).expect("serialize blocks");
    assert!(
        json.contains("Error Recovery"),
        "Error Recovery label must appear"
    );
    assert!(
        json.contains('\u{26a0}'),
        "Error Recovery icon (⚠️) must appear"
    );
}
