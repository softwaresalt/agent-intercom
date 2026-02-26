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
