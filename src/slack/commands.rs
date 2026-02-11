//! Slack slash command router.

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector, SlackCommandEvent,
    SlackCommandEventResponse, SlackMessageContent, SlackMessageResponseType,
};
use tracing::info;

/// Handle incoming slash commands routed via Socket Mode.
///
/// # Errors
///
/// Returns an error if the command response cannot be constructed.
pub async fn handle_command(
    event: SlackCommandEvent,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    _state: SlackClientEventsUserState,
) -> slack_morphism::AnyStdResult<SlackCommandEventResponse> {
    info!(command = ?event.command, user = ?event.user_id, "received slash command");

    let response = SlackCommandEventResponse {
        content: SlackMessageContent {
            text: Some("acknowledged".to_string()),
            blocks: None,
            attachments: None,
            upload: None,
            files: None,
            reactions: None,
            metadata: None,
        },
        response_type: Some(SlackMessageResponseType::Ephemeral),
    };

    Ok(response)
}
