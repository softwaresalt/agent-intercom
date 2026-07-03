//! Agent process spawner.
//!
//! Spawns host CLI processes for new agent sessions. Each session gets
//! its own child process with `kill_on_drop(true)` for safety. The
//! `INTERCOM_WORKSPACE_ROOT` and `INTERCOM_MCP_URL` environment variables
//! are set so the spawned agent knows its working directory and the MCP
//! endpoint to connect to.

use std::process::Stdio;
use std::sync::Arc;

use tokio::process::{Child, Command};
use tracing::{info, info_span, warn};

use crate::config::GlobalConfig;
use crate::models::session::{Session, SessionMode, SessionStatus};
use crate::persistence::approval_repo::ApprovalRepo;
use crate::persistence::db::Database;
use crate::persistence::prompt_repo::PromptRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::persistence::steering_repo::SteeringRepo;
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

    // Canonicalize workspace root to a resolved absolute path, then strip
    // the Windows `\\?\` extended-length prefix so downstream consumers
    // (Slack messages, DB records, subprocess args) get a clean path.
    let workspace_path = crate::config::strip_unc_prefix(
        std::path::Path::new(workspace_root)
            .canonicalize()
            .map_err(|err| {
                AppError::Config(format!("invalid workspace root {workspace_root}: {err}"))
            })?,
    );

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
    let mut cmd = build_agent_command(config, &workspace_path, &mcp_url, &created.id, prompt);

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

/// Build the host CLI command for an agent session process.
///
/// Shared by [`spawn_session`] and [`respawn_session`] so both spawn paths
/// apply the same environment contract (`INTERCOM_WORKSPACE_ROOT`,
/// `INTERCOM_MCP_URL`, `INTERCOM_SESSION_ID`), working directory, stdio wiring,
/// and `kill_on_drop` behavior.
fn build_agent_command(
    config: &GlobalConfig,
    workspace_path: &std::path::Path,
    mcp_url: &str,
    session_id: &str,
    prompt: &str,
) -> Command {
    let mut cmd = Command::new(&config.host_cli);
    cmd.args(&config.host_cli_args)
        .arg(prompt)
        .env("INTERCOM_WORKSPACE_ROOT", workspace_path)
        .env("INTERCOM_MCP_URL", mcp_url)
        .env("INTERCOM_SESSION_ID", session_id)
        .current_dir(workspace_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd
}

/// Respawn a crashed agent session and rebind it as a resumed session.
///
/// Marks the `crashed` session as `Interrupted`, then creates a new session
/// that is a restart of it (`restart_of = Some(crashed.id)`), carrying the
/// owner, workspace, prompt, routing mode, protocol, Slack channel/thread, and
/// ACP `agent_session_id` forward so the resumed agent rebinds to the same
/// logical session. A fresh host CLI process is spawned and the resumed session
/// is activated before it is returned.
///
/// Per the stdio-child resume-state contract (ADR-0017, plan F.3-T4), the
/// crashed session's durable pending state is rebound to the resumed session:
/// unconsumed steering messages (F.3-T2), pending clearances, and undecided
/// prompts (F.3-T3) are carried forward, preserving correlation ids. Rebinding
/// is best-effort — a failure to move one class is logged but does not abort
/// the recovery, since a live resumed session is preferable to none.
///
/// # Errors
///
/// Returns `AppError::Mcp` if the replacement process fails to spawn, or
/// `AppError::Db` if a session record update fails.
pub async fn respawn_session(
    crashed: &Session,
    config: &GlobalConfig,
    session_repo: &SessionRepo,
    db: &Arc<Database>,
    http_port: u16,
) -> Result<(Session, Child)> {
    let span = info_span!("respawn_session", crashed_session = %crashed.id);
    let _guard = span.enter();

    // Mark the crashed session interrupted: frees its concurrency slot and
    // makes it discoverable by recovery queries. Idempotent if already set.
    if crashed.status != SessionStatus::Interrupted {
        session_repo
            .set_terminated(&crashed.id, SessionStatus::Interrupted)
            .await?;
    }

    // Build the resumed session, rebinding identity to the crashed one.
    let mut resumed = Session::new(
        crashed.owner_user_id.clone(),
        crashed.workspace_root.clone(),
        crashed.prompt.clone(),
        crashed.mode,
    );
    resumed.protocol_mode = crashed.protocol_mode;
    resumed.channel_id = crashed.channel_id.clone();
    resumed.thread_ts = crashed.thread_ts.clone();
    resumed.agent_session_id = crashed.agent_session_id.clone();
    resumed.title = crashed.title.clone();
    resumed.restart_of = Some(crashed.id.clone());

    let created = session_repo.create(&resumed).await?;

    // Rebind the crashed session's durable pending state to the resumed session
    // so mid-task work continues (ADR-0017; consumes F.3-T2 + F.3-T3).
    rebind_pending_state(db, &crashed.id, &created.id).await;

    // Spawn the replacement process bound to the resumed session id. The
    // workspace root was canonicalized at original spawn, so it is reused as-is.
    let workspace_path = std::path::PathBuf::from(&created.workspace_root);
    let mcp_url = format!("http://localhost:{http_port}/mcp?session_id={}", created.id);
    let prompt = created.prompt.clone().unwrap_or_default();
    let mut cmd = build_agent_command(config, &workspace_path, &mcp_url, &created.id, &prompt);

    let child = cmd
        .spawn()
        .map_err(|err| AppError::Mcp(format!("failed to respawn host cli: {err}")))?;

    info!(
        crashed_session = crashed.id,
        resumed_session = created.id,
        pid = child.id(),
        "agent process respawned after crash"
    );

    // Activate the resumed session now that the replacement process is running.
    let active = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await?;

    Ok((active, child))
}

/// Carry a crashed session's durable pending state forward to its resumed
/// session (ADR-0017): unconsumed steering messages, pending clearances, and
/// undecided prompts are reassigned from `from_session_id` to `to_session_id`,
/// preserving correlation ids. Best-effort — failures are logged, not fatal.
async fn rebind_pending_state(db: &Arc<Database>, from_session_id: &str, to_session_id: &str) {
    match SteeringRepo::new(Arc::clone(db))
        .reassign_unconsumed_to_session(from_session_id, to_session_id)
        .await
    {
        Ok(count) => info!(count, "rebound steering messages to resumed session"),
        Err(err) => warn!(%err, "failed to rebind steering messages to resumed session"),
    }

    match ApprovalRepo::new(Arc::clone(db))
        .reassign_pending_to_session(from_session_id, to_session_id)
        .await
    {
        Ok(count) => info!(count, "rebound pending clearances to resumed session"),
        Err(err) => warn!(%err, "failed to rebind pending clearances to resumed session"),
    }

    match PromptRepo::new(Arc::clone(db))
        .reassign_pending_to_session(from_session_id, to_session_id)
        .await
    {
        Ok(count) => info!(count, "rebound pending prompts to resumed session"),
        Err(err) => warn!(%err, "failed to rebind pending prompts to resumed session"),
    }
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
