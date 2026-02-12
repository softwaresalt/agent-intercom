//! Slack Block Kit message builders.
//!
//! Provides helpers for constructing interactive Slack messages with
//! severity-formatted text, action buttons, and diff rendering.

use slack_morphism::prelude::{
    SlackActionBlockElement, SlackActionsBlock, SlackBlock, SlackBlockButtonElement, SlackBlockId,
    SlackBlockPlainTextOnly, SlackBlockText, SlackSectionBlock,
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
