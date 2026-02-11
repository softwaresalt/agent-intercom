//! Slack interaction handlers.

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector, SlackInteractionEvent,
};
use tracing::info;

/// Handle interactive payloads (buttons, modals) delivered via Socket Mode.
///
/// # Errors
///
/// Returns an error if the interaction cannot be processed.
pub async fn handle_interaction(
    event: SlackInteractionEvent,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    _state: SlackClientEventsUserState,
) -> slack_morphism::UserCallbackResult<()> {
    info!(?event, "received interaction event");
    Ok(())
}
