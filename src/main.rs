#![forbid(unsafe_code)]

//! `agent-intercom` — MCP remote agent server binary.
//!
//! Bootstraps configuration, starts the MCP transport (HTTP/SSE or stdio),
//! the Slack Socket Mode integration, and the IPC server for `agent-intercom-ctl`.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use agent_intercom::config::GlobalConfig;
use agent_intercom::mcp::handler::{
    AppState, PendingApprovals, PendingPrompts, PendingWaits, StallDetectors,
};
use agent_intercom::mcp::{sse, transport};
use agent_intercom::persistence::{db, retention};
use agent_intercom::slack::client::{SlackRuntime, SlackService};
use agent_intercom::{AppError, Result};

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

/// Which MCP transport(s) to start.
#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
enum Transport {
    /// Stdio only (for direct agent invocation).
    Stdio,
    /// HTTP/SSE only (for remote clients).
    Sse,
    /// Both stdio and HTTP/SSE.
    Both,
}

#[derive(Debug, Parser)]
#[command(name = "agent-intercom", about = "MCP remote agent server", version, long_about = None)]
struct Cli {
    /// Path to the TOML configuration file.
    ///
    /// Defaults to `config.toml` in the current working directory, which is
    /// the expected layout for a portable installation (binary + config.toml
    /// in the same folder).
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,

    /// Log output format (text or json).
    #[arg(long, value_enum, default_value_t = LogFormat::Text)]
    log_format: LogFormat,

    /// Override the default workspace root for the primary agent.
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Override the HTTP port for the SSE transport.
    #[arg(long)]
    port: Option<u16>,

    /// Which MCP transport(s) to start: stdio, sse, or both.
    ///
    /// Use `sse` to run as an HTTP/SSE endpoint for remote clients.
    /// Use `stdio` for direct agent invocation (e.g., from an IDE).
    /// Defaults to `both`.
    #[arg(long, value_enum, default_value_t = Transport::Both)]
    transport: Transport,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    init_tracing(args.log_format)?;
    info!("agent-intercom server bootstrap");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::Config(format!("failed to build tokio runtime: {err}")))?
        .block_on(run(args))
}

#[allow(clippy::too_many_lines)] // Startup sequence is inherently sequential.
async fn run(args: Cli) -> Result<()> {
    // ── Load configuration ──────────────────────────────
    let config_text = std::fs::read_to_string(&args.config).map_err(|err| {
        AppError::Config(format!(
            "cannot read config file '{}': {err} — copy config.toml from the release \
             archive to the same directory as the binary, or pass --config <path>",
            args.config.display()
        ))
    })?;
    let mut config = GlobalConfig::from_toml_str(&config_text)?;

    // Override workspace root from CLI if provided.
    if let Some(ws) = args.workspace {
        let canonical = std::path::Path::new(&ws)
            .canonicalize()
            .map_err(|err| AppError::Config(format!("invalid workspace override: {err}")))?;
        config.default_workspace_root = canonical;
    }

    // Override HTTP port from CLI if provided.
    if let Some(port) = args.port {
        config.http_port = port;
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
    // NOTE: Socket mode is wired in a second phase (below) after AppState
    // is fully constructed so that the interaction callbacks get the live
    // pending_prompts / pending_approvals maps.
    let (slack_service, mut slack_runtime) = if config.slack.bot_token.is_empty() {
        info!("slack not configured; running in local-only mode");
        (None, None)
    } else {
        let (svc, runtime) = SlackService::start(&config.slack).map_err(|err| {
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
        pending_modal_contexts: Arc::default(),
        stall_detectors: Some(StallDetectors::default()),
        ipc_auth_token,
    });

    // ── Check for interrupted sessions from prior crash (T082) ──
    check_interrupted_on_startup(&state).await;

    // ── Wire socket mode with the live AppState (T093/T094 fix) ────
    // Start socket mode AFTER AppState is built so the interaction
    // callbacks share the same pending_prompts/approvals/waits maps
    // as the MCP transport and can resolve oneshot channels correctly.
    if let (Some(ref svc), Some(ref mut rt)) = (&state.slack, slack_runtime.as_mut()) {
        rt.socket_task = Some(svc.start_socket_mode(Arc::clone(&state)));
        info!("slack socket mode started with live app state");
    }

    // ── Start transports ────────────────────────────────
    let start_stdio = matches!(args.transport, Transport::Stdio | Transport::Both);
    let start_sse = matches!(args.transport, Transport::Sse | Transport::Both);

    let stdio_handle = if start_stdio {
        let stdio_ct = ct.clone();
        let stdio_state = Arc::clone(&state);
        let stdio_shutdown_ct = ct.clone();
        Some(tokio::spawn(async move {
            if let Err(err) = transport::serve_stdio(stdio_state, stdio_ct).await {
                error!(%err, "stdio transport failed — initiating shutdown");
                stdio_shutdown_ct.cancel();
            }
        }))
    } else {
        info!(
            "stdio transport disabled (--transport {})",
            match args.transport {
                Transport::Sse => "sse",
                _ => "unknown",
            }
        );
        None
    };

    let sse_handle = if start_sse {
        let sse_ct = ct.clone();
        let sse_state = Arc::clone(&state);
        let sse_shutdown_ct = ct.clone();
        Some(tokio::spawn(async move {
            if let Err(err) = sse::serve_http(sse_state, sse_ct).await {
                error!(%err, "http transport failed — initiating shutdown");
                sse_shutdown_ct.cancel();
            }
        }))
    } else {
        info!("SSE transport disabled (--transport stdio)");
        None
    };

    info!(transport = ?args.transport, "MCP server ready");

    // ── Wait for first shutdown signal ──────────────────
    shutdown_signal().await;
    info!("shutdown signal received — starting graceful shutdown");
    ct.cancel();

    // Spawn a background listener for a second Ctrl+C (force-exit).
    tokio::spawn(async {
        shutdown_signal().await;
        error!("second shutdown signal received — forcing exit");
        std::process::exit(1);
    });

    // ── Graceful shutdown with timeout ───────────────────
    shutdown_with_timeout(
        &state,
        slack_runtime,
        stdio_handle,
        sse_handle,
        retention_handle,
    )
    .await;

    info!("agent-intercom shut down");

    Ok(())
}

/// Maximum time to wait for graceful shutdown before force-exiting.
const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Run the graceful shutdown sequence with a timeout.
///
/// Persists interrupted state, aborts Slack runtime tasks, and waits for
/// transport and retention handles.  If the sequence exceeds
/// [`SHUTDOWN_TIMEOUT`], it logs an error and returns immediately.
async fn shutdown_with_timeout(
    state: &AppState,
    slack_runtime: Option<SlackRuntime>,
    stdio_handle: Option<tokio::task::JoinHandle<()>>,
    sse_handle: Option<tokio::task::JoinHandle<()>>,
    retention_handle: tokio::task::JoinHandle<()>,
) {
    let shutdown_fut = async {
        // 1. Persist interrupted state.
        if let Err(err) = graceful_shutdown(state).await {
            error!(%err, "error during graceful shutdown persistence");
        }

        // 2. Abort Slack runtime tasks (they have no cancellation token).
        if let Some(ref rt) = slack_runtime {
            if let Some(ref socket) = rt.socket_task {
                socket.abort();
            }
            rt.queue_task.abort();
            info!("slack runtime tasks aborted");
        }

        // 3. Wait for transport handles.
        if let Some(h) = stdio_handle {
            let _ = h.await;
        }
        if let Some(h) = sse_handle {
            let _ = h.await;
        }
        let _ = retention_handle.await;
    };

    if tokio::time::timeout(SHUTDOWN_TIMEOUT, shutdown_fut)
        .await
        .is_err()
    {
        error!(
            timeout_secs = SHUTDOWN_TIMEOUT.as_secs(),
            "graceful shutdown timed out — exiting"
        );
    }
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
    use agent_intercom::models::approval::ApprovalStatus;
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::prompt_repo::PromptRepo;
    use agent_intercom::persistence::session_repo::SessionRepo;

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
                agent_intercom::models::prompt::PromptDecision::Stop,
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
                agent_intercom::models::session::SessionStatus::Interrupted,
            )
            .await
        {
            error!(session_id = %session.id, %err, "failed to interrupt session");
        }
    }

    // Post final notification to Slack.
    if let Some(ref slack) = state.slack {
        let ch = &state.config.slack.channel_id;
        if ch.is_empty() {
            info!("no global Slack channel configured; skipping shutdown notification");
        } else {
            let channel = slack_morphism::prelude::SlackChannelId(ch.clone());
            let msg = agent_intercom::slack::client::SlackMessage::plain(
                channel,
                format!(
                    "\u{26a0}\u{fe0f} Server shutting down. {} session(s), {} approval(s), {} prompt(s) interrupted.",
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
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::prompt_repo::PromptRepo;
    use agent_intercom::persistence::session_repo::SessionRepo;

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
        let ch = &state.config.slack.channel_id;
        if ch.is_empty() {
            info!("no global Slack channel configured; skipping startup recovery notification");
        } else {
            let channel = slack_morphism::prelude::SlackChannelId(ch.clone());
            let msg = agent_intercom::slack::client::SlackMessage::plain(
                channel,
                format!(
                    "\u{1f504} Server restarted. Found {} interrupted session(s) \
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
