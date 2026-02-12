//! Agent process spawner.
//!
//! Spawns host CLI processes for new agent sessions. Each session gets
//! its own child process with `kill_on_drop(true)` for safety. The
//! `MONOCOQUE_WORKSPACE_ROOT` environment variable is set so the agent
//! knows its working directory.

use std::process::Stdio;

use tokio::process::{Child, Command};
use tracing::{info, info_span};

use crate::config::GlobalConfig;
use crate::models::session::{Session, SessionMode, SessionStatus};
use crate::persistence::session_repo::SessionRepo;
use crate::{AppError, Result};

/// Spawn a new agent session process and persist the session record.
///
/// Creates a `Session` in the database with `Created` status, then
/// spawns the host CLI process. The session is activated only after
/// the process starts successfully.
///
/// # Errors
///
/// Returns `AppError::Config` if the concurrent session limit is exceeded,
/// or `AppError::Mcp` if the process fails to spawn.
pub async fn spawn_session(
    prompt: &str,
    workspace_root: &str,
    owner_user_id: &str,
    config: &GlobalConfig,
    session_repo: &SessionRepo,
    http_port: u16,
) -> Result<(Session, Child)> {
    let span = info_span!(
        "spawn_session",
        owner = owner_user_id,
        workspace = workspace_root
    );
    let _guard = span.enter();

    // Enforce max concurrent sessions (FR-023).
    let active_count = session_repo.count_active().await?;
    if active_count >= u64::from(config.max_concurrent_sessions) {
        return Err(AppError::Config(format!(
            "concurrent session limit reached ({}/{})",
            active_count, config.max_concurrent_sessions
        )));
    }

    // Verify user is authorized.
    config.ensure_authorized(owner_user_id)?;

    // Create session record.
    let session = Session::new(
        owner_user_id.to_owned(),
        workspace_root.to_owned(),
        Some(prompt.to_owned()),
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await?;

    // Build the SSE endpoint URL for the spawned agent.
    let sse_url = format!("http://localhost:{http_port}/mcp");

    // Spawn the host CLI process.
    let mut cmd = Command::new(&config.host_cli);
    cmd.args(&config.host_cli_args)
        .arg(prompt)
        .env("MONOCOQUE_WORKSPACE_ROOT", workspace_root)
        .env("MONOCOQUE_SSE_URL", &sse_url)
        .env("MONOCOQUE_SESSION_ID", &created.id)
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd
        .spawn()
        .map_err(|err| AppError::Mcp(format!("failed to spawn host cli: {err}")))?;

    info!(
        session_id = created.id,
        pid = child.id().unwrap_or(0),
        host_cli = config.host_cli,
        "agent process spawned"
    );

    // Activate the session now that the process is running.
    let active_session = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await?;

    Ok((active_session, child))
}

/// Verify that a user is the owner of the given session.
///
/// # Errors
///
/// Returns `AppError::Unauthorized` if the user is not the session owner.
pub fn verify_session_owner(session: &Session, user_id: &str) -> Result<()> {
    if session.owner_user_id == user_id {
        Ok(())
    } else {
        Err(AppError::Unauthorized(
            "session belongs to a different operator".into(),
        ))
    }
}
