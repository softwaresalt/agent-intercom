//! Slack Socket Mode client with a small buffered send queue.
//!
//! Includes reconnection handling (T095 / SC-003): on each WebSocket
//! hello event the client re-posts any pending interactive messages
//! (approvals, prompts) that may have been lost during a disconnect.

use std::sync::Arc;
use std::time::Duration;

use slack_morphism::prelude::{
    SlackApiChatPostMessageRequest, SlackApiChatUpdateRequest, SlackApiConversationsHistoryRequest,
    SlackApiFilesComplete, SlackApiFilesCompleteUploadExternalRequest,
    SlackApiFilesGetUploadUrlExternalRequest, SlackApiToken, SlackApiTokenType, SlackApiTokenValue,
    SlackApiViewsOpenRequest, SlackBlock, SlackChannelId, SlackClient,
    SlackClientEventsListenerEnvironment, SlackClientHyperHttpsConnector, SlackClientSession,
    SlackClientSocketModeConfig, SlackClientSocketModeListener, SlackHistoryMessage,
    SlackMessageContent, SlackSocketModeListenerCallbacks, SlackTeamId, SlackTriggerId, SlackTs,
    SlackView,
};
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};
use tracing::{error, info, warn};

use crate::mcp::handler::AppState;
use crate::models::session::SessionMode;
use crate::slack::{commands, events};
use crate::{config::SlackConfig, AppError, Result};

const QUEUE_CAPACITY: usize = 256;
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Message to be delivered to Slack via chat.postMessage.
#[derive(Debug, Clone)]
pub struct SlackMessage {
    pub channel: SlackChannelId,
    pub text: Option<String>,
    pub blocks: Option<Vec<SlackBlock>>,
    pub thread_ts: Option<SlackTs>,
}

impl SlackMessage {
    /// Create a plain-text message for a channel.
    pub fn plain(channel: SlackChannelId, text: impl Into<String>) -> Self {
        Self {
            channel,
            text: Some(text.into()),
            blocks: None,
            thread_ts: None,
        }
    }

    fn into_request(self) -> SlackApiChatPostMessageRequest {
        let content = SlackMessageContent {
            text: self.text,
            blocks: self.blocks,
            attachments: None,
            upload: None,
            files: None,
            reactions: None,
            metadata: None,
        };

        SlackApiChatPostMessageRequest {
            channel: self.channel,
            content,
            as_user: None,
            icon_emoji: None,
            icon_url: None,
            link_names: Some(true),
            parse: None,
            thread_ts: self.thread_ts,
            username: None,
            reply_broadcast: None,
            unfurl_links: None,
            unfurl_media: None,
        }
    }
}

/// Slack Socket Mode wrapper that owns a rate-limited outgoing queue.
pub struct SlackService {
    client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    bot_token: SlackApiToken,
    queue_tx: mpsc::Sender<SlackMessage>,
}

/// Join handles for Slack background tasks.
pub struct SlackRuntime {
    pub queue_task: JoinHandle<()>,
    pub socket_task: JoinHandle<()>,
}

impl SlackService {
    /// Start the Slack client and background sender task.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the HTTPS connector cannot be created.
    pub fn start(
        config: &SlackConfig,
        app_state: Option<Arc<AppState>>,
    ) -> Result<(Self, SlackRuntime)> {
        let connector = SlackClientHyperHttpsConnector::new()
            .map_err(|err| AppError::Slack(format!("failed to init slack connector: {err}")))?;
        let client = Arc::new(SlackClient::new(connector));
        let team_id = if config.team_id.is_empty() {
            None
        } else {
            Some(SlackTeamId::new(config.team_id.clone()))
        };
        let bot_token = SlackApiToken {
            token_value: SlackApiTokenValue(config.bot_token.clone()),
            cookie: None,
            team_id: team_id.clone(),
            scope: None,
            token_type: Some(SlackApiTokenType::Bot),
        };
        let app_token = SlackApiToken {
            token_value: SlackApiTokenValue(config.app_token.clone()),
            cookie: None,
            team_id,
            scope: None,
            token_type: Some(SlackApiTokenType::App),
        };

        let (queue_tx, queue_rx) = mpsc::channel(QUEUE_CAPACITY);
        let queue_task = Self::spawn_worker(client.clone(), bot_token.clone(), queue_rx);
        let socket_task = Self::spawn_socket_mode(&client, app_token.clone(), app_state);

        info!("slack service started with buffered queue and socket mode");

        Ok((
            Self {
                client,
                bot_token,
                queue_tx,
            },
            SlackRuntime {
                queue_task,
                socket_task,
            },
        ))
    }

    /// Enqueue a message for async delivery.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the message queue is full.
    pub async fn enqueue(&self, message: SlackMessage) -> Result<()> {
        self.queue_tx
            .send(message)
            .await
            .map_err(|err| AppError::Slack(format!("failed to enqueue slack message: {err}")))
    }

    /// Post a message directly and return the Slack message timestamp.
    ///
    /// Unlike [`enqueue`], this bypasses the background queue so that
    /// the caller can capture the message `ts` for threading.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the Slack API call fails.
    pub async fn post_message_direct(&self, message: SlackMessage) -> Result<SlackTs> {
        let request = message.into_request();
        let session = self.http_session();
        let response = session
            .chat_post_message(&request)
            .await
            .map_err(|err| AppError::Slack(format!("failed to post message: {err}")))?;
        Ok(response.ts)
    }

    fn spawn_worker(
        client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
        token: SlackApiToken,
        mut queue_rx: mpsc::Receiver<SlackMessage>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let session = client.open_session(&token);
            while let Some(message) = queue_rx.recv().await {
                let request = message.into_request();
                let mut backoff = INITIAL_RETRY_DELAY;
                loop {
                    match session.chat_post_message(&request).await {
                        Ok(_) => {
                            info!("sent slack message");
                            break;
                        }
                        Err(error) => {
                            let delay = match &error {
                                slack_morphism::errors::SlackClientError::RateLimitError(rate) => {
                                    rate.retry_after.unwrap_or(backoff)
                                }
                                _ => backoff,
                            };
                            warn!(?error, delay=?delay, "slack post failed; retrying");
                            sleep(delay).await;
                            backoff = (backoff * 2).min(MAX_RETRY_DELAY);
                        }
                    }
                }
            }
            info!("slack sender task exiting");
        })
    }

    fn spawn_socket_mode(
        client: &Arc<SlackClient<SlackClientHyperHttpsConnector>>,
        app_token: SlackApiToken,
        app_state: Option<Arc<AppState>>,
    ) -> JoinHandle<()> {
        let mut listener_env = SlackClientEventsListenerEnvironment::new(Arc::clone(client))
            .with_error_handler(|err, _client, _state| {
                error!(?err, "socket mode error");
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            });

        // Inject shared AppState so interaction callbacks can access it.
        if let Some(state) = app_state {
            listener_env = listener_env.with_user_state(state);
        }
        let listener_env = Arc::new(listener_env);

        let callbacks = SlackSocketModeListenerCallbacks::new()
            .with_hello_events(|event, _client, state| async move {
                // T095: On each hello (including reconnections), re-post
                // any pending interactive messages that may have been lost.
                info!(?event, "socket hello (connection established)");
                let app: Option<Arc<AppState>> = {
                    let guard = state.read().await;
                    guard.get_user_state::<Arc<AppState>>().cloned()
                };
                if let Some(app) = app {
                    repost_pending_messages(&app).await;
                }
            })
            .with_command_events(commands::handle_command)
            .with_interaction_events(events::handle_interaction)
            .with_push_events(|event, _client, _state| async move {
                info!(?event, "push event ignored");
                Ok(())
            });
        let config = SlackClientSocketModeConfig {
            max_connections_count: SlackClientSocketModeConfig::DEFAULT_CONNECTIONS_COUNT,
            debug_connections: SlackClientSocketModeConfig::DEFAULT_DEBUG_CONNECTIONS,
            initial_backoff_in_seconds:
                SlackClientSocketModeConfig::DEFAULT_INITIAL_BACKOFF_IN_SECONDS,
            reconnect_timeout_in_seconds:
                SlackClientSocketModeConfig::DEFAULT_RECONNECT_TIMEOUT_IN_SECONDS,
            ping_interval_in_seconds: SlackClientSocketModeConfig::DEFAULT_PING_INTERVAL_IN_SECONDS,
            ping_failure_threshold_times:
                SlackClientSocketModeConfig::DEFAULT_PING_FAILURE_THRESHOLD_TIMES,
        };

        let listener = SlackClientSocketModeListener::new(&config, listener_env, callbacks);
        tokio::spawn(async move {
            if let Err(error) = listener.listen_for(&app_token).await {
                error!(?error, "socket mode listen failed");
                return;
            }

            listener.serve().await;
            info!("socket mode listener exited");
        })
    }

    /// Create an HTTP session for direct API calls using the bot token.
    #[must_use]
    pub fn http_session(&self) -> SlackClientSession<'_, SlackClientHyperHttpsConnector> {
        self.client.open_session(&self.bot_token)
    }

    /// Fetch recent channel history.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the Slack API call fails.
    pub async fn fetch_recent_history(
        &self,
        channel: SlackChannelId,
        limit: u16,
    ) -> Result<Vec<SlackHistoryMessage>> {
        let (messages, _has_more) = self.fetch_history_with_more(channel, limit).await?;
        Ok(messages)
    }

    /// Fetch recent channel history including the `has_more` pagination flag.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the Slack API call fails.
    pub async fn fetch_history_with_more(
        &self,
        channel: SlackChannelId,
        limit: u16,
    ) -> Result<(Vec<SlackHistoryMessage>, bool)> {
        let request = SlackApiConversationsHistoryRequest {
            channel: Some(channel),
            cursor: None,
            latest: None,
            limit: Some(limit),
            oldest: None,
            inclusive: None,
            include_all_metadata: None,
        };

        self.http_session()
            .conversations_history(&request)
            .await
            .map(|response| {
                let has_more = response.has_more.unwrap_or(false);
                (response.messages, has_more)
            })
            .map_err(|err| AppError::Slack(format!("failed to read history: {err}")))
    }

    /// Update an existing Slack message (e.g., replace buttons with static text).
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the Slack API call fails.
    pub async fn update_message(
        &self,
        channel: SlackChannelId,
        ts: SlackTs,
        blocks: Vec<SlackBlock>,
    ) -> Result<()> {
        let request = SlackApiChatUpdateRequest::new(
            channel,
            SlackMessageContent {
                text: None,
                blocks: Some(blocks),
                attachments: None,
                upload: None,
                files: None,
                reactions: None,
                metadata: None,
            },
            ts,
        );
        self.http_session()
            .chat_update(&request)
            .await
            .map_err(|err| AppError::Slack(format!("failed to update message: {err}")))?;
        Ok(())
    }

    /// Upload a file to a Slack channel using the external upload flow.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the upload fails.
    pub async fn upload_file(
        &self,
        channel: SlackChannelId,
        filename: &str,
        content: &str,
        thread_ts: Option<SlackTs>,
    ) -> Result<()> {
        let session = self.http_session();

        // Step 1: Get upload URL.
        let url_request =
            SlackApiFilesGetUploadUrlExternalRequest::new(filename.into(), content.len());
        let url_response = session
            .get_upload_url_external(&url_request)
            .await
            .map_err(|err| AppError::Slack(format!("failed to get upload url: {err}")))?;

        // Step 2: Upload content to the URL.
        let http_client = reqwest::Client::new();
        http_client
            .post(url_response.upload_url.0.to_string())
            .body(content.to_owned())
            .send()
            .await
            .map_err(|err| AppError::Slack(format!("failed to upload file: {err}")))?;

        // Step 3: Complete the upload.
        let file_ref = SlackApiFilesComplete {
            id: url_response.file_id,
            title: Some(filename.into()),
        };
        let mut complete_request = SlackApiFilesCompleteUploadExternalRequest::new(vec![file_ref]);
        complete_request.channel_id = Some(channel);
        complete_request.thread_ts = thread_ts;
        session
            .files_complete_upload_external(&complete_request)
            .await
            .map_err(|err| AppError::Slack(format!("failed to complete upload: {err}")))?;

        Ok(())
    }

    /// Open a Slack modal dialog.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the API call fails.
    pub async fn open_modal(&self, trigger_id: SlackTriggerId, view: SlackView) -> Result<()> {
        let request = SlackApiViewsOpenRequest::new(trigger_id, view);
        self.http_session()
            .views_open(&request)
            .await
            .map_err(|err| AppError::Slack(format!("failed to open modal: {err}")))?;
        Ok(())
    }
}

// ── Reconnection: re-post pending interactive messages (T095) ────────

/// Re-post pending approvals and prompts after a Socket Mode reconnection.
///
/// When the WebSocket drops and reconnects, any interactive messages that
/// were in-flight may not be delivered. This function queries the DB for
/// pending records and re-posts their interactive messages to Slack so
/// the operator can still act on them.
async fn repost_pending_messages(state: &AppState) {
    use crate::persistence::approval_repo::ApprovalRepo;
    use crate::persistence::prompt_repo::PromptRepo;
    use crate::slack::blocks;

    let Some(ref slack) = state.slack else { return };

    let channel = SlackChannelId(state.config.slack.channel_id.clone());

    // Re-post pending approval requests.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    match approval_repo.list_pending().await {
        Ok(pending) if !pending.is_empty() => {
            info!(
                count = pending.len(),
                "re-posting pending approval requests after reconnect"
            );
            for req in pending {
                let diff_preview = if req.diff_content.lines().count() < 20 {
                    format!("```\n{}\n```", req.diff_content)
                } else {
                    format!("_(large diff, {} lines)_", req.diff_content.lines().count())
                };
                let text = format!(
                    "\u{1f504} *Re-posted after reconnect*\n\
                     *Approval:* {}\n\
                     *File:* `{}`\n\
                     *Risk:* {:?}\n\n{}",
                    req.title, req.file_path, req.risk_level, diff_preview
                );
                let msg_blocks = vec![
                    blocks::text_section(&text),
                    blocks::approval_buttons(&req.id),
                ];
                let message = SlackMessage {
                    channel: channel.clone(),
                    text: Some(format!("[Re-posted] Approval: {}", req.title)),
                    blocks: Some(msg_blocks),
                    thread_ts: None,
                };
                if let Err(err) = slack.enqueue(message).await {
                    warn!(%err, request_id = %req.id, "failed to re-post approval");
                }
            }
        }
        Ok(_) => { /* no pending approvals */ }
        Err(err) => {
            warn!(%err, "failed to query pending approvals for reconnect re-post");
        }
    }

    // Re-post pending continuation prompts.
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    match prompt_repo.list_pending().await {
        Ok(pending) if !pending.is_empty() => {
            info!(
                count = pending.len(),
                "re-posting pending prompts after reconnect"
            );
            for prompt in pending {
                let text = format!(
                    "\u{1f504} *Re-posted after reconnect*\n\
                     *Prompt:* {:?}\n\n{}",
                    prompt.prompt_type, prompt.prompt_text
                );
                let msg_blocks = vec![
                    blocks::text_section(&text),
                    blocks::prompt_buttons(&prompt.id),
                ];
                let message = SlackMessage {
                    channel: channel.clone(),
                    text: Some(format!("[Re-posted] Prompt: {:?}", prompt.prompt_type)),
                    blocks: Some(msg_blocks),
                    thread_ts: None,
                };
                if let Err(err) = slack.enqueue(message).await {
                    warn!(%err, prompt_id = %prompt.id, "failed to re-post prompt");
                }
            }
        }
        Ok(_) => { /* no pending prompts */ }
        Err(err) => {
            warn!(%err, "failed to query pending prompts for reconnect re-post");
        }
    }
}

// ── Mode-aware routing helpers ───────────────────────────────────────

/// Whether a message should be posted to Slack for the given mode.
#[must_use]
pub fn should_post_to_slack(mode: SessionMode) -> bool {
    matches!(mode, SessionMode::Remote | SessionMode::Hybrid)
}

/// Whether a message should be routed to IPC for the given mode.
#[must_use]
pub fn should_post_to_ipc(mode: SessionMode) -> bool {
    matches!(mode, SessionMode::Local | SessionMode::Hybrid)
}
