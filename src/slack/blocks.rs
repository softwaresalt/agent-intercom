//! Slack Block Kit message builders.
//!
//! Provides helpers for constructing interactive Slack messages with
//! severity-formatted text, action buttons, and diff rendering.

use slack_morphism::prelude::{
    SlackActionBlockElement, SlackActionId, SlackActionsBlock, SlackBlock, SlackBlockButtonElement,
    SlackBlockId, SlackBlockPlainTextInputElement, SlackBlockPlainTextOnly, SlackBlockText,
    SlackCallbackId, SlackInputBlock, SlackInputBlockElement, SlackModalView, SlackSectionBlock,
    SlackView,
};

/// Build a severity-formatted section block for log messages.
#[must_use]
pub fn severity_section(level: &str, message: &str) -> SlackBlock {
    let prefix = match level {
        "success" => "\u{2705}",
        "warning" => "\u{26a0}\u{fe0f}",
        "error" => "\u{274c}",
        _ => "\u{2139}\u{fe0f}",
    };
    SlackBlock::Section(SlackSectionBlock::new().with_text(SlackBlockText::MarkDown(
        format!("{prefix} {message}").into(),
    )))
}

/// Build an actions block with the given buttons.
#[must_use]
pub fn action_buttons(block_id: &str, buttons: &[(&str, &str, &str)]) -> SlackBlock {
    let elements: Vec<SlackActionBlockElement> = buttons
        .iter()
        .map(|(action_id, text, value)| {
            SlackActionBlockElement::Button(
                SlackBlockButtonElement::new(
                    (*action_id).into(),
                    SlackBlockPlainTextOnly::from(*text),
                )
                .with_value((*value).into()),
            )
        })
        .collect();
    SlackBlock::Actions(
        SlackActionsBlock::new(elements).with_block_id(SlackBlockId(block_id.into())),
    )
}

/// Build approval action buttons (Accept / Reject).
#[must_use]
pub fn approval_buttons(request_id: &str) -> SlackBlock {
    action_buttons(
        &format!("approval_{request_id}"),
        &[
            ("approve_accept", "Accept", request_id),
            ("approve_reject", "Reject", request_id),
        ],
    )
}

/// Build prompt action buttons (Continue / Refine / Stop).
#[must_use]
pub fn prompt_buttons(prompt_id: &str) -> SlackBlock {
    action_buttons(
        &format!("prompt_{prompt_id}"),
        &[
            ("prompt_continue", "Continue", prompt_id),
            ("prompt_refine", "Refine", prompt_id),
            ("prompt_stop", "Stop", prompt_id),
        ],
    )
}

/// Build stall nudge action buttons (Nudge / Nudge with Instructions / Stop).
#[must_use]
pub fn nudge_buttons(alert_id: &str) -> SlackBlock {
    action_buttons(
        &format!("stall_{alert_id}"),
        &[
            ("stall_nudge", "Nudge", alert_id),
            ("stall_nudge_instruct", "Nudge with Instructions", alert_id),
            ("stall_stop", "Stop", alert_id),
        ],
    )
}

/// Build wait-for-instruction action buttons (Resume / Resume with Instructions / Stop).
#[must_use]
pub fn wait_buttons(session_id: &str) -> SlackBlock {
    action_buttons(
        &format!("wait_{session_id}"),
        &[
            ("wait_resume", "Resume", session_id),
            (
                "wait_resume_instruct",
                "Resume with Instructions",
                session_id,
            ),
            ("wait_stop", "Stop Session", session_id),
        ],
    )
}

/// Build a plain text section block.
#[must_use]
pub fn text_section(text: &str) -> SlackBlock {
    SlackBlock::Section(SlackSectionBlock::new().with_text(SlackBlockText::MarkDown(text.into())))
}

/// Build a diff rendering section. Inline for <20 lines, marked as code block.
#[must_use]
pub fn diff_section(diff: &str) -> SlackBlock {
    let content = format!("```\n{diff}\n```");
    SlackBlock::Section(
        SlackSectionBlock::new().with_text(SlackBlockText::MarkDown(content.into())),
    )
}

/// Format a stall alert notification message (T060 / T061 — S058, S060).
///
/// Returns a plain-text Markdown string suitable for posting to Slack when an
/// agent session has been idle past the inactivity threshold.  The message
/// includes the session ID, idle duration, and actionable recovery steps.
#[must_use]
pub fn stall_alert_message(session_id: &str, idle_seconds: u64) -> String {
    let idle_display = if idle_seconds >= 60 {
        format!("{} min", idle_seconds / 60)
    } else {
        format!("{idle_seconds}s")
    };
    format!(
        "⚠️ *Agent stalled* — session `{session_id}` has been idle for {idle_display}.\n\
         \n\
         *Recovery options:*\n\
         • Nudge agent via the buttons below\n\
         • Resume manually: `agent-intercom-ctl resume {session_id}`\n\
         • Check status: `agent-intercom-ctl status`\n\
         • Spawn a new agent: `agent-intercom-ctl spawn`"
    )
}

/// Build a stall alert message block list with recovery action buttons (T060 / T061).
///
/// Intended for posting directly to Slack when `StallEvent::Stalled` fires.
#[must_use]
pub fn stall_alert_blocks(session_id: &str, idle_seconds: u64) -> Vec<SlackBlock> {
    vec![
        severity_section("warning", &stall_alert_message(session_id, idle_seconds)),
        nudge_buttons(session_id),
    ]
}

/// T063 — Build a success section for a `check_diff` apply notification.
///
/// Used by `accept_diff` after a successful patch application.
#[must_use]
pub fn diff_applied_section(file_path: &str, bytes: usize) -> SlackBlock {
    severity_section(
        "success",
        &format!("Applied approved changes to `{file_path}` ({bytes} bytes written)"),
    )
}

/// T064 — Build an alert section for a `check_diff` patch conflict notification.
///
/// Used by `accept_diff` when the file content has changed since the proposal.
#[must_use]
pub fn diff_conflict_section(file_path: &str) -> SlackBlock {
    severity_section(
        "error",
        &format!(
            "Patch conflict: `{file_path}` has changed since the proposal was created. \
             Re-submit with `force: true` to override."
        ),
    )
}

/// T065 — Build a warning section for a `check_diff` force-apply notification.
///
/// Used by `accept_diff` when a diff is applied despite a hash mismatch.
#[must_use]
pub fn diff_force_warning_section(file_path: &str) -> SlackBlock {
    severity_section(
        "warning",
        &format!(
            "Force-applying diff to `{file_path}` \u{2014} file content has diverged since proposal"
        ),
    )
}

/// Build a Slack modal view for collecting operator instructions.
///
/// The modal contains a single multiline plain-text input. The
/// `callback_id` encodes `{source}:{entity_id}` so the `ViewSubmission`
/// handler can route the instruction to the correct pending oneshot
/// (e.g. `"wait_instruct:session-id"` or `"prompt_refine:prompt-id"`).
#[must_use]
pub fn instruction_modal(callback_id: &str, title: &str, placeholder: &str) -> SlackView {
    let input_element =
        SlackBlockPlainTextInputElement::new(SlackActionId("instruction_text".to_owned()))
            .with_multiline(true)
            .with_placeholder(SlackBlockPlainTextOnly::from(placeholder));

    let input_block = SlackInputBlock::new(
        SlackBlockPlainTextOnly::from("Instructions"),
        SlackInputBlockElement::PlainTextInput(input_element),
    )
    .with_block_id(SlackBlockId("instruction_block".to_owned()));

    SlackView::Modal(
        SlackModalView::new(
            SlackBlockPlainTextOnly::from(title),
            vec![input_block.into()],
        )
        .with_callback_id(SlackCallbackId(callback_id.to_owned()))
        .with_submit(SlackBlockPlainTextOnly::from("Submit")),
    )
}
