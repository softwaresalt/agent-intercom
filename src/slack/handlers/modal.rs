//! Modal submission handler for instruction text input.
//!
//! Handles `ViewSubmission` events from Slack modals opened by the
//! `wait_resume_instruct` and `prompt_refine` button actions. Extracts
//! the typed instruction text and resolves the corresponding pending
//! oneshot channel.

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackActionId, SlackBlockId, SlackChannelId, SlackInteractionViewSubmissionEvent, SlackTs,
    SlackView,
};
use tracing::{info, warn};

use crate::mcp::handler::{AppState, PromptResponse, WaitResponse};
use crate::models::prompt::PromptDecision;
use crate::persistence::prompt_repo::PromptRepo;
use crate::slack::blocks;

/// Process a modal `ViewSubmission` event from Slack.
///
/// The `callback_id` on the view encodes `{source}:{entity_id}`:
/// - `wait_instruct:{session_id}` — resolves a pending `wait_for_instruction`
/// - `prompt_refine:{prompt_id}` — resolves a pending `forward_prompt`
///
/// The instruction text is read from
/// `view.state.values["instruction_block"]["instruction_text"].value`.
///
/// # Errors
///
/// Returns an error string if processing fails.
pub async fn handle_view_submission(
    event: &SlackInteractionViewSubmissionEvent,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let user_id = event.user.id.to_string();

    // ── Verify authorised user (FR-013) ──────────────────
    if !state.config.authorized_user_ids.contains(&user_id) {
        warn!(user_id, "unauthorised user attempted modal submission");
        return Err("user not authorised for modal submissions".into());
    }

    // ── Extract callback_id → route + entity_id ──────────
    let callback_id = match &event.view.view {
        SlackView::Modal(modal) => modal
            .callback_id
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default(),
        SlackView::Home(_) => {
            return Err("unexpected home view in modal submission".into());
        }
    };

    let (source, entity_id) = callback_id
        .split_once(':')
        .ok_or_else(|| format!("malformed callback_id: {callback_id}"))?;

    // ── Extract instruction text from view state ─────────
    let instruction = event
        .view
        .state_params
        .state
        .as_ref()
        .and_then(|s| s.values.get(&SlackBlockId("instruction_block".to_owned())))
        .and_then(|block| block.get(&SlackActionId("instruction_text".to_owned())))
        .and_then(|v| v.value.clone())
        .unwrap_or_default();

    if instruction.is_empty() {
        return Err("instruction text is empty".into());
    }

    info!(
        source,
        entity_id,
        user_id,
        instruction_len = instruction.len(),
        "modal instruction submitted"
    );

    match source {
        "wait_instruct" => resolve_wait(entity_id, &instruction, &user_id, state).await,
        "prompt_refine" => resolve_prompt(entity_id, &instruction, &user_id, state).await,
        _ => Err(format!("unknown modal source: {source}")),
    }
}

/// Resolve a pending `wait_for_instruction` oneshot with the operator's text.
async fn resolve_wait(
    session_id: &str,
    instruction: &str,
    user_id: &str,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let callback_id = format!("wait_instruct:{session_id}");

    let mut pending = state.pending_waits.lock().await;
    if let Some(tx) = pending.remove(session_id) {
        let response = WaitResponse {
            status: "resumed".to_owned(),
            instruction: Some(instruction.to_owned()),
        };
        if tx.send(response).is_err() {
            warn!(session_id, "wait oneshot receiver already dropped");
        }
    } else {
        warn!(
            session_id,
            "no pending wait oneshot found (may have timed out)"
        );
    }

    // FR-022: Replace the "⏳ Processing…" indicator with a final status.
    update_original_message(
        &callback_id,
        &format!("\u{25b6}\u{fe0f} *Resumed with instructions* by <@{user_id}>"),
        state,
    )
    .await;

    Ok(())
}

/// Resolve a pending `forward_prompt` oneshot with the operator's refined text.
async fn resolve_prompt(
    prompt_id: &str,
    instruction: &str,
    user_id: &str,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let callback_id = format!("prompt_refine:{prompt_id}");

    // Update DB record with the refined instruction.
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    prompt_repo
        .update_decision(
            prompt_id,
            PromptDecision::Refine,
            Some(instruction.to_owned()),
        )
        .await
        .map_err(|err| format!("failed to update prompt decision: {err}"))?;

    // Resolve the oneshot channel.
    let mut pending = state.pending_prompts.lock().await;
    if let Some(tx) = pending.remove(prompt_id) {
        let response = PromptResponse {
            decision: "refine".to_owned(),
            instruction: Some(instruction.to_owned()),
        };
        if tx.send(response).is_err() {
            warn!(prompt_id, "prompt oneshot receiver already dropped");
        }
    } else {
        warn!(
            prompt_id,
            "no pending prompt oneshot found (may have timed out)"
        );
    }

    // FR-022: Replace the "⏳ Processing…" indicator with a final status.
    update_original_message(
        &callback_id,
        &format!("\u{270f}\u{fe0f} *Refine* selected by <@{user_id}>"),
        state,
    )
    .await;

    Ok(())
}

/// Replace the "⏳ Processing…" indicator on the original Slack message
/// with a permanent status line (FR-022).
///
/// Retrieves the cached `(channel_id, message_ts)` stored when the modal
/// was opened and calls `chat.update`. Silently logs on failure — the
/// oneshot has already been resolved so the agent is not blocked.
async fn update_original_message(callback_id: &str, status_text: &str, state: &Arc<AppState>) {
    let context = {
        let mut ctx = state.pending_modal_contexts.lock().await;
        ctx.remove(callback_id)
    };

    let Some((channel_str, ts_str)) = context else {
        warn!(
            callback_id,
            "no cached modal context; cannot update original message"
        );
        return;
    };

    let Some(ref slack) = state.slack else { return };

    let channel = SlackChannelId::new(channel_str);
    let ts = SlackTs::new(ts_str);
    let replacement_blocks = vec![blocks::text_section(status_text)];

    if let Err(err) = slack.update_message(channel, ts, replacement_blocks).await {
        warn!(%err, callback_id, "failed to update message after modal submission");
    }
}
