#![forbid(unsafe_code)]

//! `monocoque-agent-rc` — MCP remote agent server binary.
//!
//! Bootstraps configuration, starts the MCP transport (HTTP/SSE or stdio),
//! the Slack Socket Mode integration, and the IPC server for `monocoque-ctl`.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use monocoque_agent_rc::config::GlobalConfig;
use monocoque_agent_rc::mcp::handler::{
    AppState, PendingApprovals, PendingPrompts, PendingWaits, StallDetectors,
};
use monocoque_agent_rc::mcp::{sse, transport};
use monocoque_agent_rc::persistence::{db, retention};
use monocoque_agent_rc::slack::client::SlackService;
use monocoque_agent_rc::{AppError, Result};

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "monocoque-agent-rc", about = "MCP remote agent server", version, long_about = None)]
struct Cli {
    /// Path to the TOML configuration file.
    #[arg(long)]
    config: PathBuf,

    /// Log output format (text or json).
    #[arg(long, value_enum, default_value_t = LogFormat::Text)]
    log_format: LogFormat,

    /// Override the default workspace root for the primary agent.
    #[arg(long)]
    workspace: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    init_tracing(args.log_format)?;
    info!("monocoque-agent-rc server bootstrap");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::Config(format!("failed to build tokio runtime: {err}")))?
        .block_on(run(args))
}

async fn run(args: Cli) -> Result<()> {
    // ── Load configuration ──────────────────────────────
    let config_text = std::fs::read_to_string(&args.config)
        .map_err(|err| AppError::Config(format!("cannot read config: {err}")))?;
    let mut config = GlobalConfig::from_toml_str(&config_text)?;

    // Override workspace root from CLI if provided.
    if let Some(ws) = args.workspace {
        let canonical = std::path::Path::new(&ws)
            .canonicalize()
            .map_err(|err| AppError::Config(format!("invalid workspace override: {err}")))?;
        config.default_workspace_root = canonical;
    }

    // Load Slack credentials from keyring / env vars.
    config.load_credentials().await?;

    let config = Arc::new(config);
    info!("configuration loaded");

    // ── Initialize database ─────────────────────────────
    let db_path = config.db_path().to_string_lossy().to_string();
    let db = Arc::new(db::connect(&db_path).await?);
    info!("database connected");

    // ── Start retention service ──────────────────────────
    let ct = CancellationToken::new();
    let retention_handle =
        retention::spawn_retention_task(Arc::clone(&db), config.retention_days, ct.clone());
    info!("retention service started");

    // ── Build shared application state ──────────────────
    let pending_approvals: PendingApprovals = PendingApprovals::default();
    let pending_prompts: PendingPrompts = PendingPrompts::default();
    let pending_waits: PendingWaits = PendingWaits::default();

    // Start Slack client if configured.
    let (slack_service, _slack_runtime) = if config.slack.bot_token.is_empty() {
        info!("slack not configured; running in local-only mode");
        (None, None)
    } else {
        // Build a preliminary AppState without slack so we can pass it.
        // The socket mode callbacks will receive AppState via user state injection.
        // We start slack first without app_state, then rebuild with the Arc.
        let (svc, runtime) = SlackService::start(&config.slack, None).map_err(|err| {
            error!(%err, "slack service start failed");
            err
        })?;
        info!("slack service started");
        (Some(Arc::new(svc)), Some(runtime))
    };

    // Generate a random IPC auth token for this server instance.
    let ipc_auth_token = Some(uuid::Uuid::new_v4().to_string());

    let state = Arc::new(AppState {
        config: Arc::clone(&config),
        db,
        slack: slack_service,
        pending_approvals,
        pending_prompts,
        pending_waits,
        stall_detectors: Some(StallDetectors::default()),
        ipc_auth_token,
    });

    // ── Check for interrupted sessions from prior crash (T082) ──
    check_interrupted_on_startup(&state).await;

    // ── Start transports ────────────────────────────────
    let stdio_ct = ct.clone();
    let stdio_state = Arc::clone(&state);
    let stdio_handle = tokio::spawn(async move {
        if let Err(err) = transport::serve_stdio(stdio_state, stdio_ct).await {
            error!(%err, "stdio transport failed");
        }
    });

    let sse_ct = ct.clone();
    let sse_state = Arc::clone(&state);
    let sse_handle = tokio::spawn(async move {
        if let Err(err) = sse::serve_sse(sse_state, sse_ct).await {
            error!(%err, "sse transport failed");
        }
    });

    info!("MCP server ready");

    // ── Wait for shutdown signal ────────────────────────
    shutdown_signal().await;
    info!("shutdown signal received");
    ct.cancel();

    // ── Graceful shutdown: persist state (T081) ─────────
    if let Err(err) = graceful_shutdown(&state).await {
        error!(%err, "error during graceful shutdown persistence");
    }

    // ── Wait for background tasks ───────────────────────
    let _ = tokio::join!(stdio_handle, sse_handle, retention_handle);
    info!("monocoque-agent-rc shut down");

    Ok(())
}

/// Mark all in-flight state as interrupted on graceful shutdown (T081).
///
/// - Marks pending approval requests and prompts as `Interrupted`.
/// - Marks active/paused sessions as `Interrupted` with `terminated_at`.
/// - Posts a final notification to Slack.
///
/// # Errors
///
/// Returns `AppError` if any persistence or Slack operation fails.
async fn graceful_shutdown(state: &AppState) -> Result<()> {
    use monocoque_agent_rc::models::approval::ApprovalStatus;
    use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
    use monocoque_agent_rc::persistence::prompt_repo::PromptRepo;
    use monocoque_agent_rc::persistence::session_repo::SessionRepo;

    let _span = tracing::info_span!("graceful_shutdown").entered();

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));

    // Mark all pending approval requests as Interrupted.
    let pending_approvals = approval_repo.list_pending().await.unwrap_or_default();
    for approval in &pending_approvals {
        if let Err(err) = approval_repo
            .update_status(&approval.id, ApprovalStatus::Interrupted)
            .await
        {
            error!(request_id = %approval.id, %err, "failed to interrupt approval");
        }
    }

    // Mark all pending prompts as Interrupted (set decision to Stop).
    let pending_prompts = prompt_repo.list_pending().await.unwrap_or_default();
    for prompt in &pending_prompts {
        if let Err(err) = prompt_repo
            .update_decision(
                &prompt.id,
                monocoque_agent_rc::models::prompt::PromptDecision::Stop,
                Some("server shutdown".into()),
            )
            .await
        {
            error!(prompt_id = %prompt.id, %err, "failed to interrupt prompt");
        }
    }

    // Mark all active/paused sessions as Interrupted.
    let live_sessions = session_repo
        .list_active_or_paused()
        .await
        .unwrap_or_default();
    for session in &live_sessions {
        if let Err(err) = session_repo
            .set_terminated(
                &session.id,
                monocoque_agent_rc::models::session::SessionStatus::Interrupted,
            )
            .await
        {
            error!(session_id = %session.id, %err, "failed to interrupt session");
        }
    }

    // Post final notification to Slack.
    if let Some(ref slack) = state.slack {
        let channel =
            slack_morphism::prelude::SlackChannelId(state.config.slack.channel_id.clone());
        let msg = monocoque_agent_rc::slack::client::SlackMessage::plain(
            channel,
            format!(
                "⚠️ Server shutting down. {} session(s), {} approval(s), {} prompt(s) interrupted.",
                live_sessions.len(),
                pending_approvals.len(),
                pending_prompts.len(),
            ),
        );
        if let Err(err) = slack.enqueue(msg).await {
            error!(%err, "failed to post shutdown notification to slack");
        }
        // Brief sleep to let the queue drain.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    info!(
        sessions = live_sessions.len(),
        approvals = pending_approvals.len(),
        prompts = pending_prompts.len(),
        "graceful shutdown persistence complete"
    );

    Ok(())
}

/// Check for interrupted sessions on startup and optionally re-post
/// pending requests to Slack (T082).
async fn check_interrupted_on_startup(state: &AppState) {
    use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
    use monocoque_agent_rc::persistence::prompt_repo::PromptRepo;
    use monocoque_agent_rc::persistence::session_repo::SessionRepo;

    let _span = tracing::info_span!("startup_recovery_check").entered();

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));

    let interrupted = session_repo.list_interrupted().await.unwrap_or_default();

    if interrupted.is_empty() {
        info!("no interrupted sessions found on startup");
        return;
    }

    info!(
        count = interrupted.len(),
        "found interrupted sessions on startup"
    );

    // Count and report pending requests across all interrupted sessions.
    let mut total_approvals = 0usize;
    let mut total_prompts = 0usize;

    for session in &interrupted {
        if let Ok(Some(_)) = approval_repo.get_pending_for_session(&session.id).await {
            total_approvals += 1;
        }
        if let Ok(Some(_)) = prompt_repo.get_pending_for_session(&session.id).await {
            total_prompts += 1;
        }
    }

    // Post recovery summary to Slack.
    if let Some(ref slack) = state.slack {
        let channel =
            slack_morphism::prelude::SlackChannelId(state.config.slack.channel_id.clone());
        let msg = monocoque_agent_rc::slack::client::SlackMessage::plain(
            channel,
            format!(
                "🔄 Server restarted. Found {} interrupted session(s) \
                 with {} pending approval(s) and {} pending prompt(s). \
                 Agents can use `recover_state` to resume.",
                interrupted.len(),
                total_approvals,
                total_prompts,
            ),
        );
        if let Err(err) = slack.enqueue(msg).await {
            error!(%err, "failed to post startup recovery notification");
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = ctrl_c => {}
                    _ = sigterm.recv() => {}
                }
            }
            Err(err) => {
                tracing::warn!(%err, "failed to register SIGTERM handler, using ctrl-c only");
                let _ = ctrl_c.await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(err) = ctrl_c.await {
            tracing::error!(%err, "ctrl-c signal handler failed");
        }
    }
}

fn init_tracing(log_format: LogFormat) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = fmt().with_env_filter(env_filter);

    match log_format {
        LogFormat::Text => subscriber
            .try_init()
            .map_err(|err| AppError::Config(format!("failed to init tracing: {err}")))?,
        LogFormat::Json => subscriber
            .json()
            .try_init()
            .map_err(|err| AppError::Config(format!("failed to init tracing: {err}")))?,
    }

    Ok(())
}
