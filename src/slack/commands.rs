//! Slack slash command router for `/intercom` commands.
//!
//! Parses `/intercom <command> [args]` from Slack slash command events,
//! dispatches to handlers by command name, and verifies user authorization
//! (FR-013). Session-scoped commands also verify session ownership.
//!
//! Also provides remote file browsing (`list-files`, `show-file`) and
//! pre-approved command execution (FR-014) for User Story 8.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use slack_morphism::prelude::{
    SlackChannelId, SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector,
    SlackCommandEvent, SlackCommandEventResponse, SlackMessageContent, SlackMessageResponseType,
};
use tracing::{info, info_span, warn};

use crate::diff::path_safety::validate_path;
use crate::mcp::handler::AppState;
use crate::orchestrator::{checkpoint_manager, session_manager, spawner};
use crate::persistence::checkpoint_repo::CheckpointRepo;
use crate::persistence::db::Database;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::handlers::steer as steer_handler;

/// Handle incoming `/intercom` slash commands routed via Socket Mode.
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

        "list-files" => handle_list_files(args, user_id, state).await,

        "show-file" => handle_show_file(args, user_id, state).await,

        "steer" => {
            let text = if args.is_empty() {
                return Err(crate::AppError::Config(
                    "usage: steer <message text>".into(),
                ));
            } else {
                args.join(" ")
            };
            steer_handler::store_from_slack(&text, None, state).await
        }
        other => {
            let result = validate_command_alias(other, &state.config.commands);
            match result {
                Ok(shell_command) => {
                    handle_run_command(other, &shell_command, user_id, state).await
                }
                Err(_) => Ok(format!(
                    "Unknown command: `{other}`. Use `/intercom help` for available commands."
                )),
            }
        }
    }
}

// ── Help command (T073) ──────────────────────────────────────────────

/// Generate help text grouped by category.
fn handle_help(category: Option<&str>) -> String {
    match category {
        Some("session" | "sessions") => SESSION_HELP.to_owned(),
        Some("checkpoint" | "checkpoints") => CHECKPOINT_HELP.to_owned(),
        Some("file" | "files") => FILES_HELP.to_owned(),
        _ => FULL_HELP.to_owned(),
    }
}

const FULL_HELP: &str = "\
*Available `/intercom` commands:*

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

*File Browsing*
• `list-files [path] [--depth N]` — List workspace directory tree (default depth: 3)
• `show-file <path> [--lines START:END]` — Display file contents with syntax highlighting

*Custom Commands*
• Any registered command alias — Run a pre-approved command from config

*General*
• `help [category]` — Show this help (categories: session, checkpoint, files)";

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

const FILES_HELP: &str = "\
*File browsing and command commands:*
• `list-files [path] [--depth N]` — List workspace directory tree (default depth: 3)
• `show-file <path> [--lines START:END]` — Display file contents with syntax highlighting
• Any registered command alias — Run a pre-approved command from config";

// ── Session commands (T067, T072) ────────────────────────────────────

async fn handle_sessions(db: &Arc<Database>) -> crate::Result<String> {
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
    db: &Arc<Database>,
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
    db: &Arc<Database>,
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
    db: &Arc<Database>,
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
    db: &Arc<Database>,
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
    db: &Arc<Database>,
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

// ── File browsing commands (T076, T077) ──────────────────────────────

/// Handle the `list-files` slash command (T076).
///
/// Lists directory contents below the session's workspace root.
/// Accepts an optional path and `--depth N` flag.
async fn handle_list_files(
    args: &[&str],
    user_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let span = info_span!("list_files", user = %user_id);
    let _guard = span.enter();

    let db = &state.db;
    let session_repo = SessionRepo::new(Arc::clone(db));
    let session = session_manager::resolve_session(None, user_id, &session_repo).await?;

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
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let span = info_span!("show_file", user = %user_id);
    let _guard = span.enter();

    let db = &state.db;
    let session_repo = SessionRepo::new(Arc::clone(db));
    let session = session_manager::resolve_session(None, user_id, &session_repo).await?;

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
                .upload_file(channel, &filename, &content, None)
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

// ── Custom command execution (T078) ──────────────────────────────────

/// Handle execution of a registered command alias (T078).
///
/// Looks up the alias in `config.commands`, executes the shell command
/// in the session's workspace root, and posts output to Slack.
async fn handle_run_command(
    alias: &str,
    shell_command: &str,
    user_id: &str,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    let span = info_span!("run_command", alias, user = %user_id);
    let _guard = span.enter();

    let db = &state.db;
    let session_repo = SessionRepo::new(Arc::clone(db));
    let session = session_manager::resolve_session(None, user_id, &session_repo).await?;

    let workspace_root = PathBuf::from(&session.workspace_root);

    info!(alias, shell_command, workspace_root = %workspace_root.display(), "executing registered command");

    // Pause stall timer during execution (FR-025).
    if let Some(ref detectors) = state.stall_detectors {
        let lock = detectors.lock().await;
        if let Some(handle) = lock.get(&session.id) {
            handle.pause();
        }
    }

    let output = execute_shell_command(shell_command, &workspace_root).await;

    // Resume stall timer after execution.
    if let Some(ref detectors) = state.stall_detectors {
        let lock = detectors.lock().await;
        if let Some(handle) = lock.get(&session.id) {
            handle.resume();
        }
    }

    match output {
        Ok((stdout, stderr, exit_code)) => {
            let mut result_lines = vec![format!("*`{alias}`* exited with code `{exit_code}`")];
            if !stdout.is_empty() {
                let display_out = truncate_output(&stdout, 3000);
                result_lines.push(format!("```\n{display_out}\n```"));
            }
            if !stderr.is_empty() {
                let display_err = truncate_output(&stderr, 1000);
                result_lines.push(format!("*stderr:*\n```\n{display_err}\n```"));
            }
            Ok(result_lines.join("\n"))
        }
        Err(err) => Ok(format!("Failed to execute `{alias}`: {err}")),
    }
}

// ── Public helpers (testable) ────────────────────────────────────────

/// Validate that a command alias exists in the global allowlist (FR-014).
///
/// Returns the resolved shell command string if found.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the alias is not in the registry.
pub fn validate_command_alias<S: ::std::hash::BuildHasher>(
    alias: &str,
    commands: &HashMap<String, String, S>,
) -> crate::Result<String> {
    commands
        .get(alias)
        .cloned()
        .ok_or_else(|| crate::AppError::NotFound(format!("command not found: {alias}")))
}

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

/// Execute a shell command and capture output.
async fn execute_shell_command(
    command: &str,
    working_dir: &Path,
) -> crate::Result<(String, String, i32)> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(crate::AppError::Config("empty command".into()));
    }

    let program = parts[0];
    let args = &parts[1..];

    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(working_dir)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|err| crate::AppError::Config(format!("failed to execute command: {err}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    info!(
        command,
        exit_code,
        stdout_len = stdout.len(),
        stderr_len = stderr.len(),
        "command execution complete"
    );

    Ok((stdout, stderr, exit_code))
}

/// Truncate output to a maximum length, appending an indicator if truncated.
fn truncate_output(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_owned()
    } else {
        // Find the largest char boundary ≤ max_len to avoid splitting
        // multi-byte UTF-8 sequences.
        let boundary = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= max_len)
            .last()
            .unwrap_or(0);
        let truncated = &s[..boundary];
        format!(
            "{truncated}\n... (truncated, {total} bytes total)",
            total = s.len()
        )
    }
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
