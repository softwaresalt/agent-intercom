//! Slack interaction dispatch handler.
//!
//! Receives interactive payloads (button presses, modal submissions)
//! via Socket Mode, verifies the acting user, dispatches to the
//! appropriate handler by `action_id`, and replaces buttons with
//! static status text after first action (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector, SlackInteractionEvent,
};
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::slack::handlers;

/// Handle interactive payloads (buttons, modals) delivered via Socket Mode.
///
/// Dispatches to the correct handler by `action_id` prefix. Verifies
/// the acting user belongs to `authorized_user_ids` before processing.
///
/// # Errors
///
/// Returns an error if the interaction cannot be processed.
pub async fn handle_interaction(
    event: SlackInteractionEvent,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    state: SlackClientEventsUserState,
) -> slack_morphism::UserCallbackResult<()> {
    // Extract shared AppState from the user state storage.
    let app_state: Option<Arc<AppState>> = {
        let guard = state.read().await;
        guard.get_user_state::<Arc<AppState>>().cloned()
    };

    match &event {
        SlackInteractionEvent::BlockActions(block_event) => {
            let user_id = block_event
                .user
                .as_ref()
                .map(|u| u.id.to_string())
                .unwrap_or_default();

            if let Some(actions) = &block_event.actions {
                for action in actions {
                    let action_id = action.action_id.to_string();
                    info!(action_id, user_id, "dispatching block action");

                    // Route by action_id prefix to the correct handler.
                    if action_id.starts_with("approve_") {
                        if let Some(ref app) = app_state {
                            if let Err(err) = handlers::approval::handle_approval_action(
                                action,
                                &user_id,
                                block_event.channel.as_ref(),
                                block_event.message.as_ref(),
                                app,
                            )
                            .await
                            {
                                warn!(%err, action_id, "approval action failed");
                            }
                        } else {
                            warn!(
                                action_id,
                                "app state not available; cannot process approval"
                            );
                        }
                    } else if action_id.starts_with("prompt_") {
                        if let Some(ref app) = app_state {
                            if let Err(err) = handlers::prompt::handle_prompt_action(
                                action,
                                &user_id,
                                block_event.channel.as_ref(),
                                block_event.message.as_ref(),
                                app,
                            )
                            .await
                            {
                                warn!(%err, action_id, "prompt action failed");
                            }
                        } else {
                            warn!(action_id, "app state not available; cannot process prompt");
                        }
                    } else if action_id.starts_with("stall_") {
                        if let Some(ref app) = app_state {
                            if let Err(err) = handlers::nudge::handle_nudge_action(
                                action,
                                &user_id,
                                block_event.channel.as_ref(),
                                block_event.message.as_ref(),
                                app,
                            )
                            .await
                            {
                                warn!(%err, action_id, "nudge action failed");
                            }
                        } else {
                            warn!(action_id, "app state not available; cannot process nudge");
                        }
                    } else {
                        warn!(action_id, "unknown action_id prefix");
                    }
                }
            }
        }
        _ => {
            info!(?event, "unhandled interaction event type");
        }
    }
    Ok(())
}
