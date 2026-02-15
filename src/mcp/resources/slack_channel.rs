//! `slack://channel/{id}/recent` MCP resource handler (T091).
//!
//! Exposes recent Slack channel history as an MCP resource so that
//! agents can read operator instructions posted directly in the channel.

use std::sync::Arc;

use rmcp::model::{
    Annotated, ListResourceTemplatesResult, ListResourcesResult, RawResource, RawResourceTemplate,
    ReadResourceRequestParam, ReadResourceResult, ResourceContents,
};
use serde_json::json;
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::{AppError, Result};

/// Default number of messages returned when limit is not specified.
pub const DEFAULT_LIMIT: u16 = 20;

/// Minimum allowed limit value.
const MIN_LIMIT: u16 = 1;

/// Maximum allowed limit value.
const MAX_LIMIT: u16 = 100;

/// Human-readable name for this resource.
pub const RESOURCE_NAME: &str = "Slack Channel History";

/// Description of this resource.
pub const RESOURCE_DESCRIPTION: &str = "Recent chat history from the configured Slack channel. \
     Allows the agent to read operator instructions posted directly in the channel.";

/// Parse a `slack://channel/{id}/recent` URI and return the channel ID.
///
/// Returns `None` if the URI does not match the expected pattern.
///
/// # Examples
///
/// ```
/// use monocoque_agent_rc::mcp::resources::slack_channel::parse_channel_uri;
///
/// assert_eq!(parse_channel_uri("slack://channel/C012345/recent"), Some("C012345"));
/// assert_eq!(parse_channel_uri("http://example.com"), None);
/// ```
#[must_use]
pub fn parse_channel_uri(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix("slack://channel/")?;
    let (channel_id, suffix) = rest.split_once('/')?;
    if suffix != "recent" || channel_id.is_empty() {
        return None;
    }
    Some(channel_id)
}

/// Build the `ListResourceTemplatesResult` for the Slack channel resource.
#[must_use]
pub fn resource_templates() -> ListResourceTemplatesResult {
    let template = Annotated::new(
        RawResourceTemplate {
            uri_template: "slack://channel/{id}/recent".into(),
            name: RESOURCE_NAME.into(),
            description: Some(RESOURCE_DESCRIPTION.into()),
            mime_type: Some("application/json".into()),
        },
        None,
    );

    ListResourceTemplatesResult {
        resource_templates: vec![template],
        next_cursor: None,
    }
}

/// Build the `ListResourcesResult` exposing the configured channel as a concrete resource.
#[must_use]
pub fn list_resources(channel_id: &str) -> ListResourcesResult {
    let uri = format!("slack://channel/{channel_id}/recent");
    let resource = Annotated::new(
        RawResource {
            uri,
            name: RESOURCE_NAME.into(),
            description: Some(RESOURCE_DESCRIPTION.into()),
            mime_type: Some("application/json".into()),
            size: None,
        },
        None,
    );

    ListResourcesResult {
        resources: vec![resource],
        next_cursor: None,
    }
}

/// Handle `resources/read` for the Slack channel history resource.
///
/// Fetches recent messages from the configured Slack channel using the
/// `conversations.history` API and returns them in the contract-defined
/// `{messages, has_more}` JSON format.
///
/// # Errors
///
/// Returns `AppError::Config` if the requested channel ID does not match
/// the configured channel. Returns `AppError::Slack` if the Slack service
/// is unavailable or the API call fails.
pub async fn read_resource(
    request: &ReadResourceRequestParam,
    state: &Arc<AppState>,
    effective_channel: &str,
) -> Result<ReadResourceResult> {
    let channel_id = parse_channel_uri(&request.uri).ok_or_else(|| {
        AppError::Config(format!(
            "invalid resource URI: expected slack://channel/{{id}}/recent, got '{}'",
            request.uri
        ))
    })?;

    if channel_id != effective_channel {
        return Err(AppError::Config(format!(
            "channel '{channel_id}' does not match configured channel '{effective_channel}'"
        )));
    }

    let slack = state
        .slack
        .as_ref()
        .ok_or_else(|| AppError::Slack("slack service not available (local-only mode)".into()))?;

    let limit = DEFAULT_LIMIT;
    let slack_channel = slack_morphism::prelude::SlackChannelId(channel_id.to_owned());

    info!(channel_id, limit, "reading slack channel history resource");

    let (messages, has_more) = slack.fetch_history_with_more(slack_channel, limit).await?;

    // Convert to contract schema.
    let mut output_messages = Vec::with_capacity(messages.len());
    for msg in &messages {
        let ts = msg.origin.ts.0.clone();
        let user = msg
            .sender
            .user
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |u| u.0.clone());
        let text = msg.content.text.clone().unwrap_or_default();
        let thread_ts = msg.origin.thread_ts.as_ref().map(|t| t.0.clone());

        let mut entry = json!({
            "ts": ts,
            "user": user,
            "text": text,
        });

        if let Some(thread) = thread_ts {
            entry["thread_ts"] = json!(thread);
        }

        output_messages.push(entry);
    }

    let body = json!({
        "messages": output_messages,
        "has_more": has_more,
    });

    let uri = request.uri.clone();
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::text(body.to_string(), uri)],
    })
}

/// Clamp a user-provided limit to the valid `[1, 100]` range.
#[must_use]
pub fn clamp_limit(limit: Option<u16>) -> u16 {
    match limit {
        Some(v) if v < MIN_LIMIT => {
            warn!(requested = v, clamped = MIN_LIMIT, "limit below minimum");
            MIN_LIMIT
        }
        Some(v) if v > MAX_LIMIT => {
            warn!(requested = v, clamped = MAX_LIMIT, "limit above maximum");
            MAX_LIMIT
        }
        Some(v) => v,
        None => DEFAULT_LIMIT,
    }
}
