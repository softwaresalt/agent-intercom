//! Slack Socket Mode client with a small buffered send queue.

use std::sync::Arc;
use std::time::Duration;

use slack_morphism::prelude::{
    SlackApiChatPostMessageRequest, SlackApiChatUpdateRequest,
    SlackApiConversationsHistoryRequest, SlackApiFilesComplete,
    SlackApiFilesCompleteUploadExternalRequest, SlackApiFilesGetUploadUrlExternalRequest,
    SlackApiToken, SlackApiTokenType,
    SlackApiTokenValue, SlackApiViewsOpenRequest, SlackBlock, SlackChannelId, SlackClient,
    SlackClientEventsListenerEnvironment, SlackClientHyperHttpsConnector, SlackClientSession,
    SlackClientSocketModeConfig, SlackClientSocketModeListener, SlackHistoryMessage,
    SlackMessageContent, SlackSocketModeListenerCallbacks, SlackTriggerId, SlackTs, SlackView,
};
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};
use tracing::{error, info, warn};

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
    pub fn start(config: &SlackConfig) -> Result<(Self, SlackRuntime)> {
        let connector = SlackClientHyperHttpsConnector::new()
            .map_err(|err| AppError::Slack(format!("failed to init slack connector: {err}")))?;
        let client = Arc::new(SlackClient::new(connector));
        let bot_token = SlackApiToken {
            token_value: SlackApiTokenValue(config.bot_token.clone()),
            cookie: None,
            team_id: None,
            scope: None,
            token_type: Some(SlackApiTokenType::Bot),
        };
        let app_token = SlackApiToken {
            token_value: SlackApiTokenValue(config.app_token.clone()),
            cookie: None,
            team_id: None,
            scope: None,
            token_type: Some(SlackApiTokenType::App),
        };

        let (queue_tx, queue_rx) = mpsc::channel(QUEUE_CAPACITY);
        let queue_task = Self::spawn_worker(client.clone(), bot_token.clone(), queue_rx);
        let socket_task = Self::spawn_socket_mode(&client, app_token.clone());

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
    ) -> JoinHandle<()> {
        let listener_env = Arc::new(
            SlackClientEventsListenerEnvironment::new(Arc::clone(client)).with_error_handler(
                |err, _client, _state| {
                    error!(?err, "socket mode error");
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                },
            ),
        );
        let callbacks = SlackSocketModeListenerCallbacks::new()
            .with_hello_events(|event, _client, _state| async move {
                info!(?event, "socket hello");
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
            .map(|response| response.messages)
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
        let url_request = SlackApiFilesGetUploadUrlExternalRequest::new(
            filename.into(),
            content.len(),
        );
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
        let mut complete_request =
            SlackApiFilesCompleteUploadExternalRequest::new(vec![file_ref]);
        complete_request.channel_id = Some(channel);
        complete_request.thread_ts = thread_ts;
        session
            .files_complete_upload_external(&complete_request)
            .await
            .map_err(|err| {
                AppError::Slack(format!("failed to complete upload: {err}"))
            })?;

        Ok(())
    }

    /// Open a Slack modal dialog.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Slack` if the API call fails.
    pub async fn open_modal(
        &self,
        trigger_id: SlackTriggerId,
        view: SlackView,
    ) -> Result<()> {
        let request = SlackApiViewsOpenRequest::new(trigger_id, view);
        self.http_session()
            .views_open(&request)
            .await
            .map_err(|err| AppError::Slack(format!("failed to open modal: {err}")))?;
        Ok(())
    }
}
