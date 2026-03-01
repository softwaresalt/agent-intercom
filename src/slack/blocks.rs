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

use crate::models::session::{ProtocolMode, Session, SessionMode, SessionStatus};

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

/// T063 — Build an "Add to auto-approve?" action button for manual approval suggestions.
///
/// Intended for posting after an operator manually approves a command, giving
/// them a one-click shortcut to persist the pattern to the workspace policy.
#[must_use]
pub fn auto_approve_suggestion_button(command: &str) -> SlackBlock {
    action_buttons(
        &format!("auto_approve_{}", command.len()),
        &[
            ("auto_approve_add", "Add to auto-approve?", command),
            ("auto_approve_dismiss", "No thanks", command),
        ],
    )
}

/// Build terminal command approval blocks.
///
/// Presents the command in a code fence with Approve / Reject buttons.
/// Used by the blocking `check_auto_approve` flow when `kind` is `"terminal_command"`
/// and the command is not already covered by the workspace auto-approve policy.
#[must_use]
pub fn command_approval_blocks(command: &str, request_id: &str) -> Vec<SlackBlock> {
    vec![
        text_section(&format!(
            "\u{1f510} *Terminal command approval requested*\n```\n{command}\n```"
        )),
        approval_buttons(request_id),
    ]
}

/// Determine whether a Slack message at `severity` should be posted at `detail_level`.
///
/// | `detail_level` | visible severities |
/// |---|---|
/// | `"minimal"` | `"warning"`, `"error"` only |
/// | `"standard"` (default) | all standard severities |
/// | `"verbose"` | all messages |
/// | unknown | treated as `"standard"` |
#[must_use]
pub fn message_visible_at_level(detail_level: &str, severity: &str) -> bool {
    if detail_level == "minimal" {
        matches!(severity, "warning" | "error")
    } else {
        // "standard", "verbose", and unknown values all show every severity.
        true
    }
}

/// T063 — Build a `check_diff` apply notification.
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

/// Build blocks for a threaded snippet review reply.
///
/// Each entry in `snippets` is `(label, language, content)`.  The content
/// is truncated at `max_chars` if it would exceed Slack's 3,000-character
/// per-block limit.  Returns a flat `Vec<SlackBlock>` suitable for a
/// `SlackMessage` payload — one header + code-block section per snippet,
/// separated by dividers.
#[must_use]
pub fn code_snippet_blocks(snippets: &[(&str, &str, &str)]) -> Vec<SlackBlock> {
    const MAX_CHARS: usize = 2_600;

    let mut blocks: Vec<SlackBlock> = vec![text_section(
        "\u{1f4dd} *Code snippets for review*\n_Curated by the agent \u{2014} most relevant sections_",
    )];

    for &(label, language, content) in snippets {
        let truncated;
        let body = if content.len() > MAX_CHARS {
            truncated = format!(
                "{}\n\u{2026} _(truncated \u{2014} {} chars omitted)_",
                &content[..MAX_CHARS],
                content.len() - MAX_CHARS,
            );
            truncated.as_str()
        } else {
            content
        };
        let fence = if language.is_empty() {
            format!("*{label}*\n```\n{body}\n```")
        } else {
            format!("*{label}*\n```{language}\n{body}\n```")
        };
        blocks.push(SlackBlock::Divider(
            slack_morphism::prelude::SlackDividerBlock::new(),
        ));
        blocks.push(text_section(&fence));
    }

    blocks
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

/// Build the initial "Session started" Block Kit message for a new session.
///
/// Posts as a top-level channel message whose Slack timestamp becomes the
/// session's `thread_ts`.  All subsequent messages for this session are
/// posted as replies to this thread (S036).
///
/// Includes: session ID prefix, protocol mode (MCP/ACP), operational mode,
/// workspace root, and the session creation timestamp.
#[must_use]
pub fn session_started_blocks(session: &Session) -> Vec<SlackBlock> {
    let short_id: String = session.id.chars().take(8).collect();
    let protocol = match session.protocol_mode {
        ProtocolMode::Mcp => "MCP",
        ProtocolMode::Acp => "ACP",
    };
    let mode = match session.mode {
        SessionMode::Remote => "remote",
        SessionMode::Local => "local",
        SessionMode::Hybrid => "hybrid",
    };
    let started = session.created_at.format("%Y-%m-%d %H:%M UTC");
    let text = format!(
        "\u{1f680} *Session started*\n\
         *ID:* `{short_id}\u{2026}` | *Protocol:* {protocol} | *Mode:* {mode}\n\
         *Workspace:* `{workspace}`\n\
         *Started:* {started}",
        workspace = session.workspace_root,
    );
    vec![text_section(&text)]
}

/// Build a "Session ended" Block Kit summary message for a thread reply (T060).
///
/// Posted as a reply to the session thread when the session transitions to
/// `Terminated` or `Interrupted`.  Includes session ID prefix, final status,
/// termination reason, and wall-clock duration.
#[must_use]
pub fn session_ended_blocks(session: &Session, reason: &str) -> Vec<SlackBlock> {
    let short_id: String = session.id.chars().take(8).collect();
    let status_label = match session.status {
        SessionStatus::Terminated => "terminated",
        SessionStatus::Interrupted => "interrupted",
        _ => "ended",
    };
    let duration_text = if let Some(ended_at) = session.terminated_at {
        let secs = ended_at
            .signed_duration_since(session.created_at)
            .num_seconds()
            .max(0);
        if secs >= 3600 {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        } else if secs >= 60 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{secs}s")
        }
    } else {
        "unknown".to_owned()
    };
    let text = format!(
        "\u{1f3c1} *Session ended* \u{2014} `{short_id}\u{2026}`\n\
         *Status:* {status_label} | *Reason:* {reason}\n\
         *Duration:* {duration_text}",
    );
    vec![text_section(&text)]
}
