//! Unit tests for `blocks::instruction_modal` (T036, scenario S029).
//!
//! Verifies that the modal view builder produces a correctly structured
//! Slack modal view with the expected `callback_id`, title, submit button,
//! and text input block.

use agent_intercom::slack::blocks;
use slack_morphism::prelude::SlackView;

/// S029a — Builder returns a `SlackView::Modal` variant, not a Home tab.
#[test]
fn instruction_modal_returns_modal_variant() {
    let view = blocks::instruction_modal(
        "wait_instruct:sess-123",
        "Instructions",
        "Type your instructions\u{2026}",
    );
    assert!(
        matches!(view, SlackView::Modal(_)),
        "instruction_modal must produce SlackView::Modal"
    );
}

/// S029b — The `callback_id` is serialized into the view payload.
///
/// The `ViewSubmission` handler relies on `callback_id` to route the
/// instruction text to the correct pending oneshot channel.
#[test]
fn instruction_modal_preserves_callback_id() {
    let callback_id = "wait_instruct:sess-abc";
    let view = blocks::instruction_modal(callback_id, "Instructions", "Enter text\u{2026}");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains(callback_id),
        "callback_id '{callback_id}' must appear in serialised modal"
    );
}

/// S029c — The modal title is included in the serialised payload.
#[test]
fn instruction_modal_title_is_serialized() {
    let view =
        blocks::instruction_modal("prompt_refine:pr-1", "Provide Feedback", "Describe\u{2026}");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("Provide Feedback"),
        "modal title must appear in serialised payload"
    );
}

/// S029d — The `action_id` `instruction_text` appears in the serialised payload
/// so the `ViewSubmission` handler can extract the typed text via
/// `view.state_params.state.values["instruction_block"]["instruction_text"]`.
#[test]
fn instruction_modal_action_id_is_serialized() {
    let view = blocks::instruction_modal("wait_instruct:s", "Title", "Placeholder");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("instruction_text"),
        "action_id 'instruction_text' must appear in serialised payload"
    );
}

/// S029e — The `block_id` `instruction_block` appears in the serialised payload.
#[test]
fn instruction_modal_block_id_is_serialized() {
    let view = blocks::instruction_modal("wait_instruct:s", "Title", "Placeholder");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("instruction_block"),
        "block_id 'instruction_block' must appear in serialised payload"
    );
}

/// S029f — The submit button text `"Submit"` is included.
#[test]
fn instruction_modal_has_submit_button() {
    let view = blocks::instruction_modal("wait_instruct:s", "Title", "Placeholder");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("Submit"),
        "modal must include a 'Submit' button"
    );
}

// ── Detail level visibility (T062 — S062-S067) ────────────────────────────────

/// S062 — At "minimal" detail level, "info" messages are suppressed.
#[test]
fn minimal_detail_hides_info() {
    assert!(
        !blocks::message_visible_at_level("minimal", "info"),
        "info should not be visible at minimal detail level"
    );
}

/// S063 — At "minimal" detail level, "error" messages remain visible.
#[test]
fn minimal_detail_shows_error() {
    assert!(
        blocks::message_visible_at_level("minimal", "error"),
        "error must always be visible"
    );
}

/// S064 — At "standard" detail level, "info" messages are visible.
#[test]
fn standard_detail_shows_info() {
    assert!(
        blocks::message_visible_at_level("standard", "info"),
        "info should be visible at standard detail level"
    );
}

/// S065 — At "standard" detail level, "error" messages are visible.
#[test]
fn standard_detail_shows_error() {
    assert!(
        blocks::message_visible_at_level("standard", "error"),
        "error must always be visible"
    );
}

/// S066 — At "verbose" detail level, all message types are visible.
#[test]
fn verbose_detail_shows_all_levels() {
    for sev in &["info", "success", "warning", "error"] {
        assert!(
            blocks::message_visible_at_level("verbose", sev),
            "verbose must show {sev}"
        );
    }
}

/// S067 — An unknown detail level is treated as standard (shows info and above).
#[test]
fn unknown_detail_level_defaults_to_standard() {
    assert!(
        blocks::message_visible_at_level("unknown", "info"),
        "unknown level should fall back to standard visibility"
    );
}

// ── S-T1-007: Comprehensive instruction_modal structure ───────────────────────

/// S-T1-007a — The modal placeholder text appears in the serialised payload.
#[test]
fn instruction_modal_placeholder_is_serialized() {
    let placeholder = "Enter your instructions here…";
    let view =
        blocks::instruction_modal("refine_prompt:xyz789", "Refine Instructions", placeholder);
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("Enter your instructions here"),
        "placeholder text must appear in serialised modal; got: {json}"
    );
}

/// S-T1-007b — The modal is of type "modal" in the serialised JSON.
#[test]
fn instruction_modal_type_is_modal() {
    let view = blocks::instruction_modal("cb:1", "Title", "Placeholder");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("\"type\":\"modal\"") || json.contains("\"type\": \"modal\""),
        "modal type field must be 'modal'; got: {json}"
    );
}

/// S-T1-007c — The input block is of type "input" in the serialised JSON.
#[test]
fn instruction_modal_contains_input_block_type() {
    let view = blocks::instruction_modal("cb:2", "Title", "Placeholder");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("\"type\":\"input\"") || json.contains("\"type\": \"input\""),
        "modal must contain an input block type; got: {json}"
    );
}

/// S-T1-007d — The multiline flag is enabled on the text input.
#[test]
fn instruction_modal_multiline_enabled() {
    let view = blocks::instruction_modal("cb:3", "Title", "Placeholder");
    let json = serde_json::to_string(&view).expect("serialise SlackView");
    assert!(
        json.contains("\"multiline\":true") || json.contains("\"multiline\": true"),
        "multiline must be true on the plain_text_input element; got: {json}"
    );
}

/// S-T1-007e — Full structure: `callback_id`, title, submit, input block, `action_id`, `block_id`,
/// and placeholder are all present in a single modal built with representative inputs.
#[test]
fn instruction_modal_full_structure_comprehensive() {
    let callback_id = "refine_prompt:xyz789";
    let title = "Refine Instructions";
    let placeholder = "Enter your guidance here";
    let view = blocks::instruction_modal(callback_id, title, placeholder);
    let json = serde_json::to_string(&view).expect("serialise SlackView");

    let assertions: &[(&str, &str)] = &[
        (callback_id, "callback_id"),
        (title, "title"),
        ("Submit", "submit button"),
        ("instruction_block", "block_id"),
        ("instruction_text", "action_id"),
        (placeholder, "placeholder"),
    ];

    for (pattern, label) in assertions {
        assert!(
            json.contains(pattern),
            "{label} ('{pattern}') must appear in modal JSON"
        );
    }
}
