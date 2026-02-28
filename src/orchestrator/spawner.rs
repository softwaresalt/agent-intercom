//! Agent process spawner.
//!
//! Spawns host CLI processes for new agent sessions. Each session gets
//! its own child process with `kill_on_drop(true)` for safety. The
//! `INTERCOM_WORKSPACE_ROOT` and `INTERCOM_MCP_URL` environment variables
//! are set so the spawned agent knows its working directory and the MCP
//! endpoint to connect to.

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

    // Canonicalize workspace root to a resolved absolute path.
    let workspace_path = std::path::Path::new(workspace_root)
        .canonicalize()
        .map_err(|err| {
            AppError::Config(format!("invalid workspace root {workspace_root}: {err}"))
        })?;

    // Enforce max concurrent sessions (FR-023).
    let active_count = session_repo.count_active().await?;
    if active_count >= i64::from(config.max_concurrent_sessions) {
        return Err(AppError::Config(format!(
            "concurrent session limit reached ({}/{})",
            active_count, config.max_concurrent_sessions
        )));
    }

    // Verify user is authorized.
    config.ensure_authorized(owner_user_id)?;

    // Create session record with the canonicalized workspace path so all
    // downstream components (path safety, policy loading, IPC) use a
    // consistent, fully-resolved root.
    let canonical_root = workspace_path.display().to_string();
    let session = Session::new(
        owner_user_id.to_owned(),
        canonical_root,
        Some(prompt.to_owned()),
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await?;

    // Build the MCP endpoint URL for the spawned agent.  The `session_id`
    // query parameter is read by the middleware and passed to the
    // `IntercomServer` factory via a pending-params slot, so that
    // `on_initialized` uses Case 1 (pre-created session) rather than
    // auto-creating a new session.
    let mcp_url = format!("http://localhost:{http_port}/mcp?session_id={}", created.id);

    // Spawn the host CLI process.
    let mut cmd = Command::new(&config.host_cli);
    cmd.args(&config.host_cli_args)
        .arg(prompt)
        .env("INTERCOM_WORKSPACE_ROOT", &workspace_path)
        .env("INTERCOM_MCP_URL", &mcp_url)
        .env("INTERCOM_SESSION_ID", &created.id)
        .current_dir(&workspace_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd
        .spawn()
        .map_err(|err| AppError::Mcp(format!("failed to spawn host cli: {err}")))?;

    info!(
        session_id = created.id,
        pid = child.id(),
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
