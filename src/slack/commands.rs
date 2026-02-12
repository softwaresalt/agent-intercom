//! Slack slash command router for `/monocoque` commands.
//!
//! Parses `/monocoque <command> [args]` from Slack slash command events,
//! dispatches to handlers by command name, and verifies user authorization
//! (FR-013). Session-scoped commands also verify session ownership.

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector, SlackCommandEvent,
    SlackCommandEventResponse, SlackMessageContent, SlackMessageResponseType,
};
use tracing::{info, info_span, warn};

use crate::mcp::handler::AppState;
use crate::orchestrator::{checkpoint_manager, session_manager, spawner};
use crate::persistence::checkpoint_repo::CheckpointRepo;
use crate::persistence::session_repo::SessionRepo;

/// Handle incoming `/monocoque` slash commands routed via Socket Mode.
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
            dispatch_command(command_name, &args, &user_id, app)
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
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let db = &state.db;

    match command {
        "help" => Ok(handle_help(args.first().copied())),

        "sessions" => handle_sessions(db).await,

        "session-start" => {
            let prompt = if args.is_empty() {
                return Err(crate::AppError::Config(
                    "usage: session-start <prompt>".into(),
                ));
            } else {
                args.join(" ")
            };
            handle_session_start(&prompt, user_id, state).await
        }

        "session-pause" => {
            let session_id = args.first().copied();
            handle_session_pause(session_id, user_id, db).await
        }

        "session-resume" => {
            let session_id = args.first().copied();
            handle_session_resume(session_id, user_id, db).await
        }

        "session-clear" => {
            let session_id = args.first().copied();
            handle_session_clear(session_id, user_id, db).await
        }

        "session-checkpoint" => {
            let (session_id, label) = parse_checkpoint_args(args);
            handle_session_checkpoint(session_id, label, user_id, db).await
        }

        "session-restore" => {
            let checkpoint_id = args.first().copied().ok_or_else(|| {
                crate::AppError::Config("usage: session-restore <checkpoint_id>".into())
            })?;
            handle_session_restore(checkpoint_id, db).await
        }

        "session-checkpoints" => {
            let session_id = args.first().copied();
            handle_session_checkpoints(session_id, user_id, db).await
        }

        _ => Ok(format!(
            "Unknown command: `{command}`. Use `/monocoque help` for available commands."
        )),
    }
}

// ── Help command (T073) ──────────────────────────────────────────────

/// Generate help text grouped by category.
fn handle_help(category: Option<&str>) -> String {
    match category {
        Some("session" | "sessions") => SESSION_HELP.to_owned(),
        Some("checkpoint" | "checkpoints") => CHECKPOINT_HELP.to_owned(),
        _ => FULL_HELP.to_owned(),
    }
}

const FULL_HELP: &str = "\
*Available `/monocoque` commands:*

*Session Management*
• `session-start <prompt>` — Start a new agent session
• `session-pause [session_id]` — Pause a running session
• `session-resume [session_id]` — Resume a paused session
• `session-clear [session_id]` — Terminate and clean up a session
• `sessions` — List all tracked sessions

*Checkpoints*
• `session-checkpoint [session_id] [label]` — Create a checkpoint
• `session-restore <checkpoint_id>` — Restore a checkpoint
• `session-checkpoints [session_id]` — List checkpoints

*General*
• `help [category]` — Show this help (categories: session, checkpoint)";

const SESSION_HELP: &str = "\
*Session commands:*
• `session-start <prompt>` — Start a new agent session with the given prompt
• `session-pause [session_id]` — Pause a running session (defaults to active session)
• `session-resume [session_id]` — Resume a paused session
• `session-clear [session_id]` — Terminate and clean up a session
• `sessions` — List all tracked sessions with state and timestamps";

const CHECKPOINT_HELP: &str = "\
*Checkpoint commands:*
• `session-checkpoint [session_id] [label]` — Snapshot session state and file hashes
• `session-restore <checkpoint_id>` — Restore a checkpoint (warns of diverged files)
• `session-checkpoints [session_id]` — List all checkpoints for a session";

// ── Session commands (T067, T072) ────────────────────────────────────

async fn handle_sessions(
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));
    let active = repo.list_active().await?;

    if active.is_empty() {
        return Ok("No active sessions.".to_owned());
    }

    let mut lines = vec!["*Active Sessions:*".to_owned()];
    for session in &active {
        let last_tool = session.last_tool.as_deref().unwrap_or("none");
        lines.push(format!(
            "• `{}` — owner: `{}`, status: `{:?}`, last tool: `{}`",
            session.id, session.owner_user_id, session.status, last_tool
        ));
    }

    Ok(lines.join("\n"))
}

async fn handle_session_start(
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

    let (session, _child) = spawner::spawn_session(
        prompt,
        &workspace_root,
        user_id,
        &state.config,
        &repo,
        state.config.http_port,
    )
    .await?;

    Ok(format!(
        "Session `{}` started with prompt: _{}_",
        session.id, prompt
    ))
}

async fn handle_session_pause(
    session_id: Option<&str>,
    user_id: &str,
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));

    let session = session_manager::resolve_session(session_id, user_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    let paused = session_manager::pause_session(&session.id, &repo).await?;
    Ok(format!("Session `{}` paused.", paused.id))
}

async fn handle_session_resume(
    session_id: Option<&str>,
    user_id: &str,
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));

    let session = session_manager::resolve_session(session_id, user_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    let resumed = session_manager::resume_session(&session.id, &repo).await?;
    Ok(format!("Session `{}` resumed.", resumed.id))
}

async fn handle_session_clear(
    session_id: Option<&str>,
    user_id: &str,
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
    let repo = SessionRepo::new(Arc::clone(db));

    let session = session_manager::resolve_session(session_id, user_id, &repo).await?;
    spawner::verify_session_owner(&session, user_id)?;

    // No child handle available from slash commands — the child is managed by
    // the orchestrator. Pass None to just update the DB status.
    let terminated = session_manager::terminate_session(&session.id, &repo, None).await?;
    Ok(format!("Session `{}` terminated.", terminated.id))
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
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
    let session_repo = SessionRepo::new(Arc::clone(db));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(db));

    let session = session_manager::resolve_session(session_id, user_id, &session_repo).await?;
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

async fn handle_session_restore(
    checkpoint_id: &str,
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
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
    db: &Arc<surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> crate::Result<String> {
    let session_repo = SessionRepo::new(Arc::clone(db));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(db));

    let resolved_session_id = if let Some(id) = session_id {
        id.to_owned()
    } else {
        let session = session_manager::resolve_session(None, user_id, &session_repo).await?;
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
