//! Slack slash command router for `/acom` (MCP) and `/arc` (ACP) commands.
//!
//! Parses `/<prefix> <command> [args]` from Slack slash command events,
//! dispatches to handlers by command name, and verifies user authorization
//! (FR-013). Session-scoped commands also verify session ownership.
//!
//! ACP-only commands (`session-start`, `session-stop`, `session-restart`)
//! are gated behind `ServerMode::Acp` and rejected in MCP mode.
//!
//! Also provides remote file browsing (`list-files`, `show-file`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use slack_morphism::prelude::{
    SlackChannelId, SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector,
    SlackCommandEvent, SlackCommandEventResponse, SlackMessageContent, SlackMessageResponseType,
    SlackTs,
};
use tracing::{info, info_span, warn};

use crate::acp::handshake;
use crate::acp::spawner::SpawnConfig;
use crate::diff::path_safety::validate_path;
use crate::driver::AgentDriver;
use crate::mcp::handler::AppState;
use crate::mode::ServerMode;
use crate::models::session::{ProtocolMode, Session, SessionMode, SessionStatus};
use crate::orchestrator::{checkpoint_manager, session_manager, spawner};
use crate::persistence::checkpoint_repo::CheckpointRepo;
use crate::persistence::db::Database;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;
use crate::slack::handlers::steer as steer_handler;
use crate::slack::handlers::task as task_handler;

/// Handle incoming `/acom` or `/arc` slash commands routed via Socket Mode.
///
/// # Errors
///
/// Returns an error if the command response cannot be constructed.
pub async fn handle_command(
    event: SlackCommandEvent,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    state: SlackClientEventsUserState,
) -> slack_morphism::AnyStdResult<SlackCommandEventResponse> {
    let user_id = event.user_id.to_string();
    let raw_text = event.text.clone().unwrap_or_default();
    let parts: Vec<&str> = raw_text.split_whitespace().collect();
    let command_name = parts.first().copied().unwrap_or("help");
    let args: Vec<&str> = if parts.len() > 1 {
        parts[1..].to_vec()
    } else {
        Vec::new()
    };

    let span = info_span!("slash_command", command = command_name, user = %user_id);
    let _guard = span.enter();

    info!(command = command_name, user = %user_id, "received slash command");

    // Extract shared AppState.
    let app_state: Option<Arc<AppState>> = {
        let guard = state.read().await;
        guard.get_user_state::<Arc<AppState>>().cloned()
    };

    let response_text = if let Some(ref app) = app_state {
        // Verify authorized user.
        if let Err(err) = app.config.ensure_authorized(&user_id) {
            warn!(%err, user = %user_id, "unauthorized slash command attempt");
            "You are not authorized to use this command.".to_owned()
        } else {
            let channel = event.channel_id.to_string();
            dispatch_command(command_name, &args, &user_id, &channel, app)
                .await
                .unwrap_or_else(|err| format!("Error: {err}"))
        }
    } else {
        "Server state not available.".to_owned()
    };

    Ok(ephemeral_response(&response_text))
}

/// Dispatch a parsed command to the correct handler.
async fn dispatch_command(
    command: &str,
    args: &[&str],
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let db = &state.db;
    let prefix = slash_prefix(state.server_mode);

    match command {
        "help" => Ok(handle_help(args.first().copied(), state.server_mode)),

        "sessions" => handle_sessions(db).await,

        // ACP-only session lifecycle commands.
        "session-start" if state.server_mode == ServerMode::Acp => {
            let prompt = if args.is_empty() {
                return Err(crate::AppError::Config(
                    "usage: session-start <prompt>".into(),
                ));
            } else {
                args.join(" ")
            };
            handle_session_start(&prompt, user_id, channel_id, state).await
        }

        "session-stop" if state.server_mode == ServerMode::Acp => {
            let session_id = args.first().copied();
            handle_session_stop(session_id, user_id, channel_id, state).await
        }

        "session-restart" if state.server_mode == ServerMode::Acp => {
            let session_id = args.first().copied();
            handle_session_restart(session_id, user_id, channel_id, state).await
        }

        "session-start" | "session-stop" | "session-restart" => Ok(format!(
            "`{command}` is only available in ACP mode. Use `/{prefix} help` for commands."
        )),

        "session-pause" => {
            let session_id = args.first().copied();
            handle_session_pause(session_id, user_id, channel_id, db).await
        }

        "session-resume" => {
            let session_id = args.first().copied();
            handle_session_resume(session_id, user_id, channel_id, db).await
        }

        "session-clear" => {
            let session_id = args.first().copied();
            handle_session_clear(session_id, user_id, channel_id, state).await
        }

        "session-checkpoint" => {
            let (session_id, label) = parse_checkpoint_args(args);
            handle_session_checkpoint(session_id, label, user_id, channel_id, db).await
        }

        "session-restore" => {
            let checkpoint_id = args.first().copied().ok_or_else(|| {
                crate::AppError::Config("usage: session-restore <checkpoint_id>".into())
            })?;
            handle_session_restore(checkpoint_id, db).await
        }

        "session-checkpoints" => {
            let session_id = args.first().copied();
            handle_session_checkpoints(session_id, user_id, channel_id, db).await
        }

        "list-files" => handle_list_files(args, user_id, channel_id, state).await,

        "show-file" => handle_show_file(args, user_id, channel_id, state).await,

        "steer" => {
            let text = if args.is_empty() {
                return Err(crate::AppError::Config(
                    "usage: steer <message text>".into(),
                ));
            } else {
                args.join(" ")
            };
            steer_handler::store_from_slack(&text, Some(channel_id), state).await
        }

        "task" => {
            let text = if args.is_empty() {
                return Err(crate::AppError::Config("usage: task <message text>".into()));
            } else {
                args.join(" ")
            };
            task_handler::store_from_slack(&text, Some(channel_id), state).await
        }

        other => Ok(format!(
            "Unknown command: `{other}`. Use `/{prefix} help` for available commands."
        )),
    }
}

// ── Slash prefix helper ──────────────────────────────────────────────

/// Return the slash command prefix for the current server mode.
fn slash_prefix(mode: ServerMode) -> &'static str {
    match mode {
        ServerMode::Mcp => "acom",
        ServerMode::Acp => "arc",
    }
}

// ── Help command (T073) ──────────────────────────────────────────────

/// Generate help text grouped by category, scoped to the active server mode.
fn handle_help(category: Option<&str>, mode: ServerMode) -> String {
    let prefix = slash_prefix(mode);
    match category {
        Some("session" | "sessions") => format_session_help(prefix, mode),
        Some("checkpoint" | "checkpoints") => format_checkpoint_help(prefix),
        Some("file" | "files") => format_files_help(prefix),
        Some("steering" | "steer" | "task" | "tasks") => format_steering_help(prefix),
        _ => format_full_help(prefix, mode),
    }
}

fn format_full_help(prefix: &str, mode: ServerMode) -> String {
    let mut text = format!("*Available `/{prefix}` commands:*\n\n");

    text.push_str(
        "*Agent Steering*\n\
         • `steer <message>` — Send a steering message to the agent (delivered on next ping)\n\
         • `task <message>` — Queue a task for the agent (delivered on next session recovery)\n\n",
    );

    text.push_str("*Session Management*\n");
    if mode == ServerMode::Acp {
        text.push_str(
            "• `session-start <prompt>` — Start a new agent session\n\
             • `session-stop [session_id]` — Gracefully stop a running session\n\
             • `session-restart [session_id]` — Restart a session with its original prompt\n",
        );
    }
    text.push_str(
        "• `session-pause [session_id]` — Pause a running session\n\
         • `session-resume [session_id]` — Resume a paused session\n\
         • `session-clear [session_id]` — Force-terminate and clean up a session\n\
         • `sessions` — List all tracked sessions\n\n",
    );

    text.push_str(
        "*Checkpoints*\n\
         • `session-checkpoint [session_id] [label]` — Create a checkpoint\n\
         • `session-restore <checkpoint_id>` — Restore a checkpoint\n\
         • `session-checkpoints [session_id]` — List checkpoints\n\n",
    );

    text.push_str(
        "*File Browsing*\n\
         • `list-files [path] [--depth N]` — List workspace directory tree (default depth: 3)\n\
         • `show-file <path> [--lines START:END]` — Display file contents with syntax \
         highlighting\n\n",
    );

    text.push_str(
        "*General*\n\
         • `help [category]` — Show this help (categories: session, checkpoint, files, steering)",
    );

    text
}

fn format_session_help(prefix: &str, mode: ServerMode) -> String {
    let mut text = String::from("*Session commands:*\n");
    if mode == ServerMode::Acp {
        text.push_str(
            "• `session-start <prompt>` — Start a new agent session with the given prompt\n\
             • `session-stop [session_id]` — Gracefully stop a running session (sends interrupt \
             first)\n\
             • `session-restart [session_id]` — Restart a session with its original prompt\n",
        );
    }
    text.push_str(
        "• `session-pause [session_id]` — Pause a running session (defaults to active session)\n\
         • `session-resume [session_id]` — Resume a paused session\n\
         • `session-clear [session_id]` — Force-terminate and clean up a session\n\
         • `sessions` — List all tracked sessions with state and timestamps",
    );
    let _ = prefix; // used by callers for consistency; format kept static
    text
}

fn format_checkpoint_help(prefix: &str) -> String {
    let _ = prefix;
    "*Checkpoint commands:*\n\
     • `session-checkpoint [session_id] [label]` — Snapshot session state and file hashes\n\
     • `session-restore <checkpoint_id>` — Restore a checkpoint (warns of diverged files)\n\
     • `session-checkpoints [session_id]` — List all checkpoints for a session"
        .to_owned()
}

fn format_files_help(prefix: &str) -> String {
    let _ = prefix;
    "*File browsing commands:*\n\
     • `list-files [path] [--depth N]` — List workspace directory tree (default depth: 3)\n\
     • `show-file <path> [--lines START:END]` — Display file contents with syntax highlighting"
        .to_owned()
}

fn format_steering_help(prefix: &str) -> String {
    let _ = prefix;
    "*Agent steering commands:*\n\
     • `steer <message>` — Send a steering message to the active agent session. The message is \
     queued and delivered on the agent's next `ping` call. Use this to redirect focus or provide \
     guidance without interrupting the current operation.\n\
     • `task <message>` — Queue a task item for the agent. Tasks are delivered in bulk on the \
     agent's next session recovery (`reboot` call), making them ideal for asynchronous to-do \
     items that the agent should pick up at the start of its next session."
        .to_owned()
}

// ── Session commands (T067, T072) ────────────────────────────────────

/// Resolve the session a slash command should operate on.
///
/// When `session_id` is explicitly provided it is returned directly (without
/// ownership verification — callers do that via [`spawner::verify_session_owner`]).
/// When absent, the most-recently-updated session owned by `user_id` in
/// `channel_id` is returned. If no session is found in the channel the
/// caller receives a descriptive `NotFound` error (T068 / S045).
async fn resolve_command_session(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    repo: &SessionRepo,
) -> crate::Result<crate::models::session::Session> {
    if let Some(id) = session_id {
        // Try exact match first, then fall back to prefix matching.
        if let Some(session) = repo.get_by_id(id).await? {
            return Ok(session);
        }
        return repo
            .get_by_prefix(id)
            .await?
            .ok_or_else(|| crate::AppError::NotFound(format!("session {id} not found")));
    }

    // T067: Prefer the session in the originating channel (S043).
    let channel_sessions = repo.find_active_by_channel(channel_id).await?;
    if let Some(session) = channel_sessions
        .into_iter()
        .find(|s| s.owner_user_id == user_id)
    {
        return Ok(session);
    }

    // T068: No session in this channel — return a channel-specific error (S045).
    Err(crate::AppError::NotFound(
        "no active session in this channel — use `sessions` to see all sessions".into(),
    ))
}

async fn handle_sessions(db: &Arc<Database>) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));
    let active = repo.list_active().await?;

    if active.is_empty() {
        return Ok("No active sessions.".to_owned());
    }

    let mut lines = vec!["*Active Sessions:*".to_owned()];
    for session in &active {
        let short_id: String = session.id.chars().take(8).collect();
        let protocol = match session.protocol_mode {
            ProtocolMode::Acp => "ACP",
            ProtocolMode::Mcp => "MCP",
        };
        let connectivity = format!("{:?}", session.connectivity_status);
        lines.push(format!(
            "• `{short_id}…` — {protocol} | owner: `{}` | connectivity: {connectivity}",
            session.owner_user_id
        ));
    }

    Ok(lines.join("\n"))
}

async fn handle_session_start(
    prompt: &str,
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    match state.server_mode {
        ServerMode::Acp => handle_acp_session_start(prompt, user_id, channel_id, state).await,
        ServerMode::Mcp => handle_mcp_session_start(prompt, user_id, state).await,
    }
}

// This startup sequence is inherently sequential — each step depends on the
// previous (validate → count sessions → create DB record → spawn process →
// handshake → register driver → post Slack message). The heavy work runs
// in a background task so the slash command can respond immediately.
#[allow(clippy::too_many_lines)]
async fn handle_acp_session_start(
    prompt: &str,
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    // Validate ACP configuration before attempting to spawn.
    state.config.validate_for_acp_mode()?;

    let repo = SessionRepo::new(Arc::clone(&state.db));

    // S024: enforce max concurrent ACP sessions.
    let active_count = repo.count_active().await?;
    let max = i64::try_from(state.config.acp.max_sessions).unwrap_or(i64::MAX);
    if active_count >= max {
        return Err(crate::AppError::Acp(format!(
            "max concurrent ACP sessions reached ({active_count}/{})",
            state.config.acp.max_sessions
        )));
    }

    // Resolve the workspace root and name from the incoming Slack channel.
    // Falls back to `default_workspace_root` when the channel has no mapping.
    let workspace_root = state
        .config
        .workspace_root_for_channel(channel_id)
        .to_path_buf();
    let workspace_name = state
        .config
        .resolve_workspace_by_channel_id(channel_id)
        .and_then(|m| m.label.as_deref())
        .or_else(|| workspace_root.file_name().and_then(|n| n.to_str()))
        .unwrap_or("workspace")
        .to_owned();

    // Build the session record with ACP-specific fields.
    let mut session = Session::new(
        user_id.to_owned(),
        workspace_root.to_string_lossy().to_string(),
        Some(prompt.to_owned()),
        SessionMode::Remote,
    );
    session.protocol_mode = ProtocolMode::Acp;
    session.channel_id = Some(channel_id.to_owned());

    let created = repo.create(&session).await?;
    let session_id = created.id.clone();

    // Spawn the heavy work (process launch, handshake, Slack posting) in a
    // background task so the slash command can return immediately. Slack
    // Socket Mode requires an acknowledgement within ~3 seconds; spawning
    // and waiting for the agent's ready signal can take 30+ seconds.
    let bg_state = Arc::clone(state);
    let bg_prompt = prompt.to_owned();
    let bg_channel = channel_id.to_owned();
    let bg_workspace_root = workspace_root;
    let bg_workspace_name = workspace_name.clone();
    let bg_session_id = session_id.clone();

    tokio::spawn(async move {
        if let Err(err) = finish_acp_session_start(
            &bg_session_id,
            &bg_prompt,
            &bg_channel,
            &bg_workspace_root,
            &bg_workspace_name,
            &bg_state,
        )
        .await
        {
            warn!(
                session_id = %bg_session_id,
                %err,
                "ACP session start failed in background"
            );
            // Mark the session as interrupted so it doesn't linger as pending.
            let repo = SessionRepo::new(Arc::clone(&bg_state.db));
            repo.set_terminated(&bg_session_id, SessionStatus::Interrupted)
                .await
                .ok();

            // Notify the operator of the failure via Slack.
            if let Some(ref slack) = bg_state.slack {
                let msg = SlackMessage {
                    channel: SlackChannelId(bg_channel.clone()),
                    text: Some(format!(
                        "\u{274c} ACP session `{}` failed to start: {err}",
                        bg_session_id.chars().take(8).collect::<String>()
                    )),
                    blocks: None,
                    thread_ts: None,
                };
                slack.post_message_direct(msg).await.ok();
            }
        }
    });

    Ok(format!(
        "\u{23f3} Starting ACP session `{}` in `{workspace_name}`…",
        session_id.chars().take(8).collect::<String>()
    ))
}

/// Background continuation of ACP session start: spawn process, handshake,
/// wire I/O tasks, post Slack notification.
#[allow(clippy::too_many_lines)]
async fn finish_acp_session_start(
    session_id: &str,
    prompt: &str,
    channel_id: &str,
    workspace_root: &Path,
    workspace_name: &str,
    state: &Arc<AppState>,
) -> crate::Result<()> {
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Spawn the agent process (no prompt CLI arg — FR-030).
    let spawn_cfg = SpawnConfig {
        host_cli: state.config.host_cli.clone(),
        host_cli_args: state.config.host_cli_args.clone(),
        workspace_root: workspace_root.to_path_buf(),
    };

    let mut conn = crate::acp::spawner::spawn_agent(&spawn_cfg, session_id)?;

    // Perform the ACP handshake: initialize → result → initialized → session/new → prompt.
    let handshake_timeout = Duration::from_secs(state.config.acp.startup_timeout_seconds);
    let handshake_result = async {
        handshake::send_initialize(&mut conn.stdin, session_id, workspace_root, workspace_name)
            .await?;
        handshake::wait_for_initialize_result(&mut conn.stdout, session_id, handshake_timeout)
            .await?;
        handshake::send_initialized(&mut conn.stdin, session_id).await?;
        let agent_session_id = handshake::send_session_new(
            &mut conn.stdin,
            &mut conn.stdout,
            session_id,
            workspace_root,
            handshake_timeout,
        )
        .await?;
        // Persist the agent-assigned session ID for subsequent prompts.
        repo.set_agent_session_id(session_id, &agent_session_id)
            .await?;
        handshake::send_prompt(&mut conn.stdin, session_id, &agent_session_id, prompt).await
    }
    .await;

    if let Err(err) = handshake_result {
        repo.set_terminated(session_id, SessionStatus::Interrupted)
            .await
            .ok();
        return Err(err);
    }

    // Wire ACP I/O tasks for this session (T084).
    // Each session gets its own outbound message channel; inbound events are
    // routed through the shared acp_event_tx stored in AppState.
    if let (Some(ref acp_driver), Some(ref event_tx)) = (&state.acp_driver, &state.acp_event_tx) {
        use tokio_util::sync::CancellationToken;

        let (msg_tx, msg_rx) = tokio::sync::mpsc::channel(256);
        acp_driver.register_session(session_id, msg_tx).await;

        // Register the agent-assigned session ID so `send_prompt` can include
        // it in `session/prompt` messages.
        if let Ok(Some(sess)) = repo.get_by_id(session_id).await {
            if let Some(ref asid) = sess.agent_session_id {
                acp_driver
                    .register_agent_session_id(session_id, asid)
                    .await;
            }
        }

        let session_ct = CancellationToken::new();
        let reader_event_tx = event_tx.clone();
        let reader_session_id = session_id.to_owned();
        let reader_ct = session_ct.clone();
        let reader_db = Arc::clone(&state.db);
        let reader_driver: Arc<dyn crate::driver::AgentDriver> = acp_driver.clone();
        let reader_channel_id = channel_id.to_owned();
        let reader_slack = state.slack.clone();
        let flush_ctx = crate::acp::reader::ReconnectFlushContext {
            db: reader_db,
            driver: reader_driver,
            slack: reader_slack,
            channel_id: Some(reader_channel_id),
            thread_ts: None, // thread_ts not yet recorded at spawn
        };
        tokio::spawn(crate::acp::reader::run_reader(
            reader_session_id,
            conn.stdout,
            reader_event_tx,
            reader_ct,
            Some(flush_ctx),
        ));

        let writer_session_id = session_id.to_owned();
        let writer_ct = session_ct.clone();
        tokio::spawn(crate::acp::writer::run_writer(
            writer_session_id,
            conn.stdin,
            msg_rx,
            writer_ct,
        ));

        state
            .active_children
            .lock()
            .await
            .insert(session_id.to_owned(), conn.child);
    } else {
        // ACP driver not configured — store child handle only.
        state
            .active_children
            .lock()
            .await
            .insert(session_id.to_owned(), conn.child);
    }

    // Activate the session.
    let active = repo
        .update_status(session_id, SessionStatus::Active)
        .await?;

    // T058 / S036: Post "session started" as the thread root and record ts.
    // All subsequent messages for this session will be posted as thread replies.
    if let Some(ref slack) = state.slack {
        let started_blocks = blocks::session_started_blocks(&active);
        let msg = SlackMessage {
            channel: SlackChannelId(channel_id.to_owned()),
            text: Some(format!(
                "\u{1f916} ACP session `{}` started in `{workspace_name}`",
                active.id.chars().take(8).collect::<String>()
            )),
            blocks: Some(started_blocks),
            thread_ts: None,
        };
        match slack.post_message_direct(msg).await {
            Ok(ts) => {
                if let Err(err) = repo.set_thread_ts(&active.id, &ts.0).await {
                    warn!(%err, session_id = %active.id, "failed to record thread_ts");
                } else {
                    info!(session_id = %active.id, thread_ts = %ts.0, "thread_ts recorded");
                }
            }
            Err(err) => {
                warn!(%err, session_id = %active.id, "failed to post session-started message");
            }
        }
    }

    info!(
        session_id = active.id,
        channel_id, workspace = %workspace_name, "ACP session started"
    );

    Ok(())
}

/// Start a new MCP session (existing behaviour, now refactored into its own fn).
async fn handle_mcp_session_start(
    prompt: &str,
    user_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let workspace_root = state
        .config
        .default_workspace_root()
        .to_string_lossy()
        .to_string();

    let (session, child) = spawner::spawn_session(
        prompt,
        &workspace_root,
        user_id,
        &state.config,
        &repo,
        state.config.http_port,
    )
    .await?;

    // Store the child so kill_on_drop doesn't terminate the process immediately.
    state
        .active_children
        .lock()
        .await
        .insert(session.id.clone(), child);

    Ok(format!(
        "Session `{}` started with prompt: _{}_",
        session.id, prompt
    ))
}

async fn handle_session_pause(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    db: &Arc<Database>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));

    let session = resolve_command_session(session_id, user_id, channel_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    let paused = session_manager::pause_session(&session.id, &repo).await?;
    Ok(format!("Session `{}` paused.", paused.id))
}

async fn handle_session_resume(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    db: &Arc<Database>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));

    let session = resolve_command_session(session_id, user_id, channel_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    let resumed = session_manager::resume_session(&session.id, &repo).await?;
    Ok(format!("Session `{}` resumed.", resumed.id))
}

/// Gracefully stop an ACP session.
///
/// Sends `session/interrupt` to the agent process first, giving it a chance to
/// clean up, then terminates the process and marks the session as `Terminated`.
/// Posts a "session stopped" notification to the session's Slack thread.
///
/// Unlike `session-clear`, which force-terminates immediately, `session-stop`
/// is the preferred way to close an ACP session when the agent should be given
/// the opportunity to save state or wrap up current work.
async fn handle_session_stop(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let session = resolve_command_session(session_id, user_id, channel_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    if session.protocol_mode != ProtocolMode::Acp {
        return Err(crate::AppError::Config(
            "session-stop is only supported for ACP sessions; use session-clear for MCP sessions"
                .into(),
        ));
    }

    // Send session/interrupt to give the agent a chance to wrap up.
    if let Some(ref acp_driver) = state.acp_driver {
        if let Err(err) = acp_driver.interrupt(&session.id).await {
            warn!(%err, session_id = %session.id, "session-stop: interrupt delivery failed — continuing with termination");
        }
    }

    let mut child = state.active_children.lock().await.remove(&session.id);
    let terminated = session_manager::terminate_session(&session.id, &repo, child.as_mut()).await?;

    // Deregister the ACP driver's in-memory state for this session so
    // stale writers / agent_session_ids don't linger.
    if let Some(ref acp_driver) = state.acp_driver {
        acp_driver.deregister_session(&session.id).await;
    }

    if let Some(ref slack) = state.slack {
        session_manager::notify_session_ended(&terminated, "stopped by operator", slack).await;
    }

    info!(session_id = %terminated.id, user_id, "ACP session stopped by operator");
    Ok(format!("ACP session `{}` stopped.", terminated.id))
}

async fn handle_session_clear(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let session = resolve_command_session(session_id, user_id, channel_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    // Remove the child from the registry before awaiting terminate_session
    // to avoid holding the lock guard across an await point.
    let mut child = state.active_children.lock().await.remove(&session.id);
    let terminated = session_manager::terminate_session(&session.id, &repo, child.as_mut()).await?;

    // Deregister the ACP driver's in-memory state for this session.
    if let Some(ref acp_driver) = state.acp_driver {
        acp_driver.deregister_session(&session.id).await;
    }

    // T060 / S094: Post session-ended summary as a threaded reply.
    if let Some(ref slack) = state.slack {
        session_manager::notify_session_ended(&terminated, "terminated by operator", slack).await;
    }

    Ok(format!("Session `{}` terminated.", terminated.id))
}

/// Restart an ACP session (T098 / S067).
///
/// Terminates the currently running session (marking it `Interrupted`),
/// removes the child process from the registry, and spawns a fresh ACP session
/// with the same original prompt, channel, and owner.  The new session posts
/// its "started" message to the channel so the operator can track it.
///
/// Works only in ACP mode.  Attempting to restart an MCP session returns a
/// descriptive error.
async fn handle_session_restart(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let session = resolve_command_session(session_id, user_id, channel_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    if session.protocol_mode != ProtocolMode::Acp {
        return Err(crate::AppError::Config(
            "session-restart is only supported for ACP sessions".into(),
        ));
    }

    let original_prompt = session.prompt.clone().unwrap_or_default();
    let old_session_id = session.id.clone();

    // Interrupt the old session via the driver (best-effort).
    if let Some(ref acp_driver) = state.acp_driver {
        if let Err(err) = acp_driver.interrupt(&old_session_id).await {
            warn!(%err, session_id = %old_session_id, "acp interrupt failed during restart — continuing");
        }
    }

    // Remove the child from the registry so the old process is dropped.
    let mut child = state.active_children.lock().await.remove(&old_session_id);

    // Mark old session as Interrupted.
    if let Err(err) =
        session_manager::terminate_session(&old_session_id, &repo, child.as_mut()).await
    {
        warn!(%err, session_id = %old_session_id, "failed to terminate old session during restart");
    }

    // Deregister the old session's ACP driver state.
    if let Some(ref acp_driver) = state.acp_driver {
        acp_driver.deregister_session(&old_session_id).await;
    }

    // Notify the Slack thread that the session was restarted.
    if let Some(ref slack) = state.slack {
        let msg = SlackMessage {
            channel: SlackChannelId(channel_id.to_owned()),
            text: Some(format!(
                "\u{1f504} Session `{old_session_id}` is being restarted with original prompt."
            )),
            blocks: None,
            thread_ts: session.thread_ts.as_deref().map(|s| SlackTs(s.to_owned())),
        };
        if let Err(err) = slack.enqueue(msg).await {
            warn!(%err, "failed to post restart notification");
        }
    }

    // Spawn the new ACP session with the original prompt.
    handle_acp_session_start(&original_prompt, user_id, channel_id, state).await
}

// ── Checkpoint commands (T070-T071 integration, T072) ────────────────

/// Parse checkpoint args: `[session_id] [label]` — if first arg looks like a
/// UUID it's a `session_id`, otherwise it's used as the label.
fn parse_checkpoint_args<'a>(args: &[&'a str]) -> (Option<&'a str>, Option<&'a str>) {
    match args.len() {
        0 => (None, None),
        1 => {
            // If it contains a dash and is longish, treat as session_id.
            if args[0].contains('-') && args[0].len() > 10 {
                (Some(args[0]), None)
            } else {
                (None, Some(args[0]))
            }
        }
        _ => (Some(args[0]), Some(args[1])),
    }
}

async fn handle_session_checkpoint(
    session_id: Option<&str>,
    label: Option<&str>,
    user_id: &str,
    channel_id: &str,
    db: &Arc<Database>,
) -> crate::Result<String> {
    let session_repo = SessionRepo::new(Arc::clone(db));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(db));

    let session = resolve_command_session(session_id, user_id, channel_id, &session_repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    let checkpoint =
        checkpoint_manager::create_checkpoint(&session.id, label, &session_repo, &checkpoint_repo)
            .await?;

    let label_text = checkpoint.label.as_deref().unwrap_or("(unnamed)");

    Ok(format!(
        "Checkpoint `{}` created for session `{}` (label: _{}_). {} files hashed.",
        checkpoint.id,
        session.id,
        label_text,
        checkpoint.file_hashes.len()
    ))
}

async fn handle_session_restore(checkpoint_id: &str, db: &Arc<Database>) -> crate::Result<String> {
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(db));

    let (checkpoint, divergences) =
        checkpoint_manager::restore_checkpoint(checkpoint_id, &checkpoint_repo).await?;

    if divergences.is_empty() {
        Ok(format!(
            "Checkpoint `{}` restored for session `{}`. No file divergences.",
            checkpoint.id, checkpoint.session_id
        ))
    } else {
        let mut lines = vec![format!(
            "Checkpoint `{}` loaded. *{} file(s) diverged:*",
            checkpoint.id,
            divergences.len()
        )];
        for entry in &divergences {
            let kind = match entry.kind {
                checkpoint_manager::DivergenceKind::Modified => "modified",
                checkpoint_manager::DivergenceKind::Deleted => "deleted",
                checkpoint_manager::DivergenceKind::Added => "added",
            };
            lines.push(format!("• `{}` ({})", entry.file_path, kind));
        }
        lines.push("\n_Confirm before proceeding with restore._".to_owned());
        Ok(lines.join("\n"))
    }
}

async fn handle_session_checkpoints(
    session_id: Option<&str>,
    user_id: &str,
    channel_id: &str,
    db: &Arc<Database>,
) -> crate::Result<String> {
    let session_repo = SessionRepo::new(Arc::clone(db));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(db));

    let resolved_session_id = if let Some(id) = session_id {
        id.to_owned()
    } else {
        let session = resolve_command_session(None, user_id, channel_id, &session_repo).await?;
        session.id
    };

    let checkpoints = checkpoint_repo
        .list_for_session(&resolved_session_id)
        .await?;

    if checkpoints.is_empty() {
        return Ok(format!(
            "No checkpoints for session `{resolved_session_id}`."
        ));
    }

    let mut lines = vec![format!(
        "*Checkpoints for session `{}`* ({} total):",
        resolved_session_id,
        checkpoints.len()
    )];
    for cp in &checkpoints {
        let label = cp.label.as_deref().unwrap_or("(unnamed)");
        lines.push(format!(
            "• `{}` — _{}_  (created: {})",
            cp.id, label, cp.created_at
        ));
    }

    Ok(lines.join("\n"))
}

// ── File browsing commands (T076, T077) ──────────────────────────────

/// Handle the `list-files` slash command (T076).
///
/// Lists directory contents below the session's workspace root.
/// Accepts an optional path and `--depth N` flag.
async fn handle_list_files(
    args: &[&str],
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let span = info_span!("list_files", user = %user_id);
    let _guard = span.enter();

    let db = &state.db;
    let session_repo = SessionRepo::new(Arc::clone(db));
    let session = resolve_command_session(None, user_id, channel_id, &session_repo).await?;

    let workspace_root = PathBuf::from(&session.workspace_root);

    // Parse optional path and --depth flag.
    let (target_path, max_depth) = parse_list_files_args(args);

    let resolved = validate_listing_path(target_path, &workspace_root)?;

    let tree = build_directory_tree(&resolved, &workspace_root, max_depth, 0)?;

    if tree.is_empty() {
        Ok("_(empty directory)_".to_owned())
    } else {
        Ok(format!("```\n{tree}```"))
    }
}

/// Handle the `show-file` slash command (T077).
///
/// Displays file contents. Accepts a path and optional `--lines START:END`.
async fn handle_show_file(
    args: &[&str],
    user_id: &str,
    channel_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let span = info_span!("show_file", user = %user_id);
    let _guard = span.enter();

    let db = &state.db;
    let session_repo = SessionRepo::new(Arc::clone(db));
    let session = resolve_command_session(None, user_id, channel_id, &session_repo).await?;

    let workspace_root = PathBuf::from(&session.workspace_root);

    let (file_path, line_range) = parse_show_file_args(args)?;

    let resolved = validate_listing_path(Some(file_path), &workspace_root)?;

    if !resolved.is_file() {
        return Err(crate::AppError::NotFound(format!(
            "not a file: {}",
            resolved.display()
        )));
    }

    let raw = std::fs::read_to_string(&resolved)
        .map_err(|err| crate::AppError::Diff(format!("failed to read file: {err}")))?;

    let content = match line_range {
        Some((start, end)) => extract_line_range(&raw, start, end),
        None => raw.clone(),
    };

    let lang = file_extension_language(&resolved.to_string_lossy());

    // If the content is short enough, return inline; otherwise indicate
    // it should be uploaded as a snippet (Slack 4000-char limit).
    if content.len() < 3500 {
        Ok(format!("```{lang}\n{content}\n```"))
    } else {
        // Post via Slack file upload when Slack client is available.
        if let Some(ref slack) = state.slack {
            let ch = &state.config.slack.channel_id;
            if ch.is_empty() {
                // No global channel configured; fall through to truncated inline response.
                let truncated = &content[..3400];
                return Ok(format!(
                    "```{lang}\n{truncated}\n```\n_(truncated — {total} bytes total)_",
                    total = content.len()
                ));
            }
            let channel = SlackChannelId::new(ch.clone());
            let filename = resolved
                .file_name()
                .map_or("file.txt".to_owned(), |n| n.to_string_lossy().to_string());
            slack
                .upload_file(channel, &filename, &content, None, Some(lang))
                .await?;
            Ok(format!(
                "File `{}` uploaded as snippet.",
                resolved.display()
            ))
        } else {
            // Truncate for ephemeral response.
            let truncated = &content[..3400];
            Ok(format!(
                "```{lang}\n{truncated}\n```\n_(truncated — {total} bytes total)_",
                total = content.len()
            ))
        }
    }
}

// ── Public helpers (testable) ────────────────────────────────────────

/// Validate a listing path against the workspace root (FR-006).
///
/// If `path` is `None`, returns the canonical workspace root.
/// Absolute paths are canonicalized and checked directly.
/// Relative paths are resolved via the standard `validate_path` helper.
///
/// # Errors
///
/// Returns `AppError::PathViolation` if the path escapes the workspace.
pub fn validate_listing_path(path: Option<&str>, workspace_root: &Path) -> crate::Result<PathBuf> {
    let root = workspace_root
        .canonicalize()
        .map_err(|err| crate::AppError::PathViolation(format!("workspace root invalid: {err}")))?;

    match path {
        Some(p) => {
            let candidate = Path::new(p);
            if candidate.is_absolute() {
                // For absolute paths, canonicalize (if exists) and verify
                // the result is within the workspace root.
                let resolved = if candidate.exists() {
                    candidate.canonicalize().map_err(|err| {
                        crate::AppError::PathViolation(format!("cannot resolve path: {err}"))
                    })?
                } else {
                    candidate.to_path_buf()
                };
                if resolved.starts_with(&root) {
                    Ok(resolved)
                } else {
                    Err(crate::AppError::PathViolation(
                        "path outside workspace".into(),
                    ))
                }
            } else {
                // Relative paths delegate to the standard path validator.
                validate_path(workspace_root, p)
            }
        }
        None => Ok(root),
    }
}

/// Infer the syntax-highlighting language from a file name's extension.
#[must_use]
pub fn file_extension_language(filename: &str) -> &'static str {
    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "java" => "java",
        "cs" => "csharp",
        "cpp" | "cc" | "cxx" => "cpp",
        "c" | "h" => "c",
        "sh" | "bash" => "bash",
        "ps1" => "powershell",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "md" | "markdown" => "markdown",
        "tf" => "hcl",
        "dockerfile" | "Dockerfile" => "dockerfile",
        _ => "text",
    }
}

// ── Internal helpers ─────────────────────────────────────────────────

/// Parse `list-files` arguments: optional path and `--depth N`.
fn parse_list_files_args<'a>(args: &[&'a str]) -> (Option<&'a str>, usize) {
    let mut path: Option<&str> = None;
    let mut depth: usize = 3;
    let mut i = 0;

    while i < args.len() {
        if args[i] == "--depth" {
            if let Some(next) = args.get(i + 1) {
                depth = next.parse().unwrap_or(3);
                i += 2;
                continue;
            }
        }
        if path.is_none() {
            path = Some(args[i]);
        }
        i += 1;
    }

    (path, depth)
}

/// Parse `show-file` arguments: `<path> [--lines START:END]`.
fn parse_show_file_args<'a>(args: &[&'a str]) -> crate::Result<(&'a str, Option<(usize, usize)>)> {
    if args.is_empty() {
        return Err(crate::AppError::Config(
            "usage: show-file <path> [--lines START:END]".into(),
        ));
    }

    let file_path = args[0];
    let mut line_range: Option<(usize, usize)> = None;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--lines" {
            if let Some(range_str) = args.get(i + 1) {
                line_range = parse_line_range(range_str);
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    Ok((file_path, line_range))
}

/// Parse a `START:END` range string into 1-based line numbers.
fn parse_line_range(s: &str) -> Option<(usize, usize)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let start = parts[0].parse::<usize>().ok()?;
        let end = parts[1].parse::<usize>().ok()?;
        if start > 0 && end >= start {
            return Some((start, end));
        }
    }
    None
}

/// Extract lines `start..=end` (1-based) from text.
fn extract_line_range(text: &str, start: usize, end: usize) -> String {
    text.lines()
        .enumerate()
        .filter(|(i, _)| {
            let line_num = i + 1;
            line_num >= start && line_num <= end
        })
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build a recursive directory tree string.
#[allow(clippy::only_used_in_recursion)] // `workspace_root` is threaded through for future use.
fn build_directory_tree(
    dir: &Path,
    workspace_root: &Path,
    max_depth: usize,
    current_depth: usize,
) -> crate::Result<String> {
    use std::fmt::Write;

    if current_depth >= max_depth {
        return Ok(String::new());
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|err| crate::AppError::Diff(format!("failed to read directory: {err}")))?;

    let mut items: Vec<(String, bool)> = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|err| crate::AppError::Diff(format!("failed to read entry: {err}")))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().is_ok_and(|ft| ft.is_dir());

        // Skip hidden directories and common large dirs.
        if is_dir && (name.starts_with('.') || name == "node_modules" || name == "target") {
            continue;
        }
        items.push((name, is_dir));
    }

    items.sort_by(|a, b| {
        // Directories first, then alphabetical.
        b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
    });

    let mut result = String::new();
    let indent = "  ".repeat(current_depth);

    for (name, is_dir) in &items {
        if *is_dir {
            let _ = writeln!(result, "{indent}{name}/");
            let child_path = dir.join(name);
            let subtree =
                build_directory_tree(&child_path, workspace_root, max_depth, current_depth + 1)?;
            result.push_str(&subtree);
        } else {
            let _ = writeln!(result, "{indent}{name}");
        }
    }

    Ok(result)
}

/// Build an ephemeral Slack command response.
fn ephemeral_response(text: &str) -> SlackCommandEventResponse {
    SlackCommandEventResponse {
        content: SlackMessageContent {
            text: Some(text.to_owned()),
            blocks: None,
            attachments: None,
            upload: None,
            files: None,
            reactions: None,
            metadata: None,
        },
        response_type: Some(SlackMessageResponseType::Ephemeral),
    }
}
