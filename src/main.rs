#![forbid(unsafe_code)]

//! `agent-intercom` — MCP remote agent server binary.
//!
//! Bootstraps configuration, starts the MCP transport (HTTP/SSE or stdio),
//! the Slack Socket Mode integration, and the IPC server for `agent-intercom-ctl`.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use agent_intercom::audit::writer::JsonlAuditWriter;
use agent_intercom::audit::AuditLogger;
use agent_intercom::config::GlobalConfig;
use agent_intercom::config_watcher::ConfigWatcher;
use agent_intercom::driver::acp_driver::AcpDriver;
use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::driver::AgentEvent;
use agent_intercom::mcp::handler::{
    AppState, PendingApprovals, PendingPrompts, PendingWaits, StallDetectors,
};
use agent_intercom::mcp::{sse, transport};
use agent_intercom::mode::ServerMode;
use agent_intercom::orchestrator::{child_monitor, stall_consumer};
use agent_intercom::persistence::{db, retention};
use agent_intercom::policy::watcher::PolicyWatcher;
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

    /// Server protocol mode: `mcp` (default) or `acp`.
    ///
    /// `mcp` starts the standard Model Context Protocol transport.
    /// `acp` starts the Agent Communication Protocol stream processor
    /// and skips the MCP HTTP/SSE and stdio transports.
    #[arg(long, value_enum, default_value_t = ServerMode::Mcp)]
    mode: ServerMode,
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
        config.default_workspace_root = agent_intercom::config::strip_unc_prefix(canonical);
    }

    // Override HTTP port from CLI if provided.
    let cli_port_override = args.port.is_some();
    if let Some(port) = args.port {
        config.http_port = port;
    }

    // Load Slack credentials from keyring / env vars.
    // Mode-prefixed sources are tried first for ACP (ADR-0015).
    config.load_credentials(args.mode).await?;

    // Validate ACP-specific configuration when running in ACP mode.
    if args.mode == ServerMode::Acp {
        config.validate_for_acp_mode()?;
        // Additional path-security validation (FR-038, FR-039): logs WARN if
        // host_cli is outside standard directories or not found on PATH.
        config.validate_host_cli_path().ok();
        // Auto-suffix the IPC pipe name so MCP and ACP instances don't
        // collide on the same named pipe (ADR-0015). Only applied when
        // the name is still the default; an explicit override is preserved.
        if config.ipc_name == "agent-intercom" {
            config.ipc_name = "agent-intercom-acp".into();
            info!(ipc_name = %config.ipc_name, "ACP mode: IPC name auto-suffixed");
        }
        // Use the ACP-specific HTTP port so MCP and ACP instances can run
        // concurrently without a port conflict.  The CLI --port flag takes
        // precedence over the [acp] config value.
        if !cli_port_override {
            config.http_port = config.acp.http_port;
            info!(
                http_port = config.http_port,
                "ACP mode: HTTP port set from [acp] config"
            );
        }
        info!("ACP mode: host_cli validated");
        // Check for orphan processes from prior runs (ES-004, FR-037).
        agent_intercom::acp::spawner::check_for_orphan_processes(&config.host_cli).await;
    }

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

    // Build the protocol driver and ACP event channel.
    // In MCP mode: McpDriver wraps the pending oneshot maps.
    // In ACP mode: AcpDriver routes operator decisions to per-session streams;
    //              a shared event channel carries inbound AgentEvents from all
    //              reader tasks to a single consumer task started after AppState
    //              is fully constructed so the consumer has access to the full
    //              state (stall detectors, Slack service, DB).
    let (driver, acp_driver_opt, acp_event_tx_opt, acp_event_recv) = if args.mode == ServerMode::Acp
    {
        let acp = Arc::new(AcpDriver::new());
        let (tx, rx) = tokio::sync::mpsc::channel::<AgentEvent>(1024);
        // Coerce Arc<AcpDriver> → Arc<dyn AgentDriver> via unsized coercion.
        let driver_arc: Arc<dyn agent_intercom::driver::AgentDriver> = acp.clone();
        (driver_arc, Some(acp), Some(tx), Some(rx))
    } else {
        // MCP mode: build McpDriver from clones of the pending maps so that
        // Slack handlers and MCP tool handlers share the same in-memory channels.
        let mcp = McpDriver::new(
            Arc::clone(&pending_approvals),
            Arc::clone(&pending_prompts),
            Arc::clone(&pending_waits),
        );
        let driver_arc: Arc<dyn agent_intercom::driver::AgentDriver> = Arc::new(mcp);
        (driver_arc, None, None, None)
    };

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

    // ── Initialize audit logger ─────────────────────────
    let audit_log_dir = config.default_workspace_root.join(".intercom/logs");
    let audit_logger: Option<Arc<dyn AuditLogger>> = match JsonlAuditWriter::new(audit_log_dir) {
        Ok(writer) => Some(Arc::new(writer)),
        Err(err) => {
            warn!(%err, "failed to initialize audit logger, continuing without audit logging");
            None
        }
    };

    // ── Initialize policy watcher ────────────────────────
    // The watcher loads the initial policy from `.intercom/settings.json`,
    // sets up a `notify` file watcher on that directory, and hot-reloads the
    // in-memory cache whenever the file changes.  `AppState.policy_cache` is
    // the SAME `Arc` owned by the watcher so all hot-reload events are
    // immediately visible to `check_auto_approve` without any additional
    // invalidation logic.
    let policy_watcher = PolicyWatcher::new();
    if let Err(err) = policy_watcher
        .register(&config.default_workspace_root)
        .await
    {
        warn!(%err, "policy watcher registration failed — falling back to on-demand loads");
    } else {
        info!("policy watcher registered for default workspace root");
    }
    let policy_cache = policy_watcher.cache().clone();

    // ── Initialize config watcher for workspace mapping hot-reload ───────
    // Watches `config.toml` for changes and re-parses `[[workspace]]` entries
    // so that new sessions always see the latest workspace→channel mappings
    // without a server restart (FR-014).
    let config_watcher = ConfigWatcher::new(&args.config)
        .map_err(|err| {
            warn!(%err, "config watcher failed to start — workspace mappings will not hot-reload");
            err
        })
        .ok();
    let workspace_mappings = config_watcher.as_ref().map_or_else(
        || Arc::new(std::sync::RwLock::new(config.workspaces.clone())),
        ConfigWatcher::mappings,
    );

    // ── Create stall event channel ──────────────────────
    let (stall_tx, stall_rx) = tokio::sync::mpsc::channel(256);

    let state = Arc::new(AppState {
        config: Arc::clone(&config),
        db,
        slack: slack_service,
        pending_approvals,
        pending_prompts,
        pending_waits,
        pending_modal_contexts: Arc::default(),
        pending_thread_replies: Arc::default(),
        stall_detectors: Some(StallDetectors::default()),
        ipc_auth_token,
        policy_cache,
        audit_logger,
        active_children: Arc::default(),
        pending_command_approvals: Arc::default(),
        stall_event_tx: Some(stall_tx),
        driver,
        server_mode: args.mode,
        workspace_mappings,
        acp_event_tx: acp_event_tx_opt,
        acp_driver: acp_driver_opt,
    });

    // Keep the watchers alive for the server's lifetime — dropping them stops
    // the notify subscriptions and hot-reload stops working.
    let _policy_watcher = policy_watcher;
    let _config_watcher = config_watcher;

    // ── Spawn ACP event consumer (T099) ────────────────────
    // Spawned after AppState is built so the consumer has access to the full
    // state: stall detectors (for StreamActivity), Slack service (for crash
    // notifications), and the database (for pending clearance resolution).
    if let Some(rx) = acp_event_recv {
        let consumer_ct = ct.clone();
        let consumer_state = Arc::clone(&state);
        tokio::spawn(async move {
            run_acp_event_consumer(rx, consumer_ct, consumer_state).await;
        });
        info!("acp event consumer started");
    }

    // ── Check for interrupted sessions from prior crash (T082) ──
    check_interrupted_on_startup(&state).await;

    // ── Spawn stall event consumer ──────────────────────
    let _stall_consumer_handle = if let Some(ref slack) = state.slack {
        let default_channel = state.config.slack.channel_id.clone();
        // T097: Pass the driver so the consumer can deliver ACP nudges on-stream.
        let stall_driver: Option<Arc<dyn agent_intercom::driver::AgentDriver>> =
            state.acp_driver.as_ref().map(|d| {
                let drv: Arc<dyn agent_intercom::driver::AgentDriver> = d.clone();
                drv
            });
        Some(stall_consumer::spawn_stall_event_consumer(
            stall_rx,
            Arc::clone(slack),
            default_channel,
            Arc::clone(&state.db),
            stall_driver,
            ct.clone(),
        ))
    } else {
        info!("stall event consumer not started (no slack service)");
        // Drop the receiver so senders fail fast.
        drop(stall_rx);
        None
    };

    // ── Spawn child process monitor ─────────────────────
    let _child_monitor_handle = if let Some(ref slack) = state.slack {
        let default_channel = state.config.slack.channel_id.clone();
        Some(child_monitor::spawn_child_monitor(
            Arc::clone(&state.active_children),
            Arc::clone(slack),
            default_channel,
            Arc::clone(&state.db),
            ct.clone(),
        ))
    } else {
        info!("child process monitor not started (no slack service)");
        None
    };

    // ── Wire socket mode with the live AppState (T093/T094 fix) ────
    // Start socket mode AFTER AppState is built so the interaction
    // callbacks share the same pending_prompts/approvals/waits maps
    // as the MCP transport and can resolve oneshot channels correctly.
    if let (Some(ref svc), Some(ref mut rt)) = (&state.slack, slack_runtime.as_mut()) {
        rt.socket_task = Some(svc.start_socket_mode(Arc::clone(&state)));
        info!("slack socket mode started with live app state");
    }

    // ── Start transports ────────────────────────────────
    // The HTTP transport starts in BOTH MCP and ACP modes. In ACP mode,
    // the endpoint lets agent subprocesses call MCP tools (check_clearance,
    // transmit, auto_check, etc.) via HTTP. Without it, tools are unreachable
    // from spawned ACP sessions (HITL-003 / FR-032).
    let start_stdio = args.mode == ServerMode::Mcp
        && matches!(args.transport, Transport::Stdio | Transport::Both);
    let start_sse = matches!(args.transport, Transport::Sse | Transport::Both);

    if args.mode == ServerMode::Acp {
        info!(
            "ACP mode: stdio transport disabled; HTTP transport starting for MCP tool access \
             by ACP subprocesses (HITL-003)"
        );
    }

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
        // Attempt to bind BEFORE spawning. A bind failure means the port is
        // already in use (e.g., second instance). Exit cleanly rather than
        // leaving Slack and stdio running with no HTTP front-end.
        match sse::bind_http(&state).await {
            Ok(listener) => Some(tokio::spawn(async move {
                if let Err(err) = sse::serve_with_listener(listener, sse_state, sse_ct).await {
                    error!(%err, "http transport failed — initiating shutdown");
                    sse_shutdown_ct.cancel();
                }
            })),
            Err(err) => {
                error!(
                    %err,
                    "failed to bind HTTP transport — shutting down and exiting"
                );
                // Abort Slack runtime so the process can exit cleanly.
                if let Some(ref rt) = slack_runtime {
                    if let Some(ref socket) = rt.socket_task {
                        socket.abort();
                    }
                    rt.queue_task.abort();
                }
                std::process::exit(1);
            }
        }
    } else {
        info!("SSE transport disabled (--transport stdio)");
        None
    };

    info!(transport = ?args.transport, mode = ?args.mode, "server ready");

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

/// Brief delay to let the Slack outgoing queue drain before aborting the task (T072).
///
/// This ensures any messages enqueued during `graceful_shutdown` (such as the
/// shutdown notification) have time to be posted, regardless of whether a Slack
/// channel is configured.
const QUEUE_DRAIN_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

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

        // 2. Unconditional queue drain (T072): let the outgoing Slack message
        //    queue flush any enqueued messages (e.g. shutdown notification)
        //    before the background worker task is aborted.
        tokio::time::sleep(QUEUE_DRAIN_DELAY).await;

        // 3. Abort Slack runtime tasks (they have no cancellation token).
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
///
/// On server restart, any sessions that were Active/Online are now orphaned
/// (their agent processes are dead). This function first marks all such
/// sessions as Interrupted, then counts pending requests and optionally
/// posts a recovery summary to Slack.
async fn check_interrupted_on_startup(state: &AppState) {
    use agent_intercom::models::session::SessionStatus;
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::prompt_repo::PromptRepo;
    use agent_intercom::persistence::session_repo::SessionRepo;

    let _span = tracing::info_span!("startup_recovery_check").entered();

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));

    // Mark all Active sessions as Interrupted — their agent processes were
    // lost when the server shut down.
    let active = session_repo.list_active().await.unwrap_or_default();
    for session in &active {
        if let Err(err) = session_repo
            .update_status(&session.id, SessionStatus::Interrupted)
            .await
        {
            warn!(%err, session_id = %session.id, "failed to mark active session as interrupted");
        }
    }

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

/// ACP event consumer — dispatches [`AgentEvent`]s from all reader tasks.
///
/// Reads events from the shared channel and:
/// - [`StreamActivity`]: resets the per-session stall detector timer (S063).
/// - [`StatusUpdated`]: accumulates text fragments per session and posts
///   aggregated messages to the session's Slack thread after a 2-second
///   debounce (prevents flooding from word-by-word `agent_message_chunk`
///   streaming).
/// - [`SessionTerminated`]: resolves any pending clearance requests as
///   `Interrupted` (S068) and optionally notifies the operator on Slack.
/// - All other variants: logged at INFO for observability.
///
/// Exits when the channel closes or `cancel` fires.
#[allow(clippy::too_many_lines)]
async fn run_acp_event_consumer(
    mut rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    cancel: tokio_util::sync::CancellationToken,
    state: Arc<AppState>,
) {
    use std::collections::HashMap;
    use tokio::time::{Duration, Instant};

    /// Per-session text accumulator for debounced Slack posting.
    struct TextBuffer {
        text: String,
        last_update: Instant,
    }

    let mut text_buffers: HashMap<String, TextBuffer> = HashMap::new();
    let debounce = Duration::from_secs(2);
    let mut flush_interval = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                info!("acp event consumer: cancellation received, stopping");
                break;
            }
            _ = flush_interval.tick() => {
                // Flush text buffers that have been idle for ≥ debounce duration.
                let now = Instant::now();
                let expired_keys: Vec<String> = text_buffers
                    .iter()
                    .filter(|(_, buf)| now.duration_since(buf.last_update) >= debounce)
                    .map(|(k, _)| k.clone())
                    .collect();
                for session_id in expired_keys {
                    if let Some(buf) = text_buffers.remove(&session_id) {
                        flush_text_to_slack(&state, &session_id, &buf.text).await;
                    }
                }
            }
            event = rx.recv() => {
                match event {
                    None => {
                        info!("acp event consumer: channel closed, stopping");
                        break;
                    }
                    Some(AgentEvent::StreamActivity { ref session_id }) => {
                        // Reset stall detector timer on any stream activity (S063).
                        if let Some(ref detectors) = state.stall_detectors {
                            let map = detectors.lock().await;
                            if let Some(handle) = map.get(session_id) {
                                handle.reset();
                            }
                        }
                    }
                    Some(AgentEvent::ClearanceRequested {
                        ref session_id,
                        ref request_id,
                        ref title,
                        ref description,
                        ref diff,
                        ref file_path,
                        ref risk_level,
                    }) => {
                        info!(
                            session_id,
                            request_id,
                            title,
                            "acp event: clearance requested"
                        );
                        handle_clearance_requested(
                            &state,
                            session_id,
                            request_id,
                            title,
                            description,
                            diff.clone(),
                            file_path,
                            risk_level,
                        )
                        .await;
                    }
                    Some(AgentEvent::StatusUpdated { ref session_id, ref message }) => {
                        // Accumulate text; debounce flush posts to Slack thread.
                        let entry = text_buffers
                            .entry(session_id.clone())
                            .or_insert_with(|| TextBuffer {
                                text: String::new(),
                                last_update: Instant::now(),
                            });
                        entry.text.push_str(message);
                        entry.last_update = Instant::now();
                    }
                    Some(AgentEvent::PromptForwarded {
                        ref session_id,
                        ref prompt_id,
                        ref prompt_text,
                        ref prompt_type,
                    }) => {
                        info!(
                            session_id,
                            prompt_id,
                            "acp event: prompt forwarded"
                        );
                        handle_prompt_forwarded(
                            &state,
                            session_id,
                            prompt_id,
                            prompt_text,
                            prompt_type,
                        )
                        .await;
                    }
                    Some(AgentEvent::HeartbeatReceived { ref session_id, .. }) => {
                        info!(session_id, "acp event: heartbeat received");
                    }
                    Some(AgentEvent::SessionTerminated { ref session_id, exit_code, ref reason }) => {
                        info!(
                            session_id,
                            exit_code,
                            reason,
                            "acp event: session terminated — resolving pending clearances"
                        );

                        // Flush any buffered text before posting the termination notice.
                        if let Some(buf) = text_buffers.remove(session_id) {
                            flush_text_to_slack(&state, session_id, &buf.text).await;
                        }

                        handle_session_terminated(&state, session_id, reason).await;
                    }
                }
            }
        }
    }
}

/// Handle the `SessionTerminated` event: update DB status, resolve pending
/// clearances, deregister driver state, and notify the operator on Slack.
async fn handle_session_terminated(state: &Arc<AppState>, session_id: &str, reason: &str) {
    use agent_intercom::models::approval::ApprovalStatus;
    use agent_intercom::models::session::SessionStatus;
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::session_repo::SessionRepo;
    use agent_intercom::slack::client::SlackMessage;
    use slack_morphism::prelude::{SlackChannelId, SlackTs};

    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    // F-03: Mark the session as Interrupted in the database so it no longer
    // appears in list_active(). Without this, sessions whose agent process
    // exits naturally (EOF, crash) remain Active forever.
    if let Err(err) = session_repo
        .set_terminated(session_id, SessionStatus::Interrupted)
        .await
    {
        warn!(%err, session_id, "failed to mark session as interrupted on termination");
    }

    // S068: Resolve any pending clearance requests as Interrupted
    // so the operator is not left waiting for buttons that will never be clicked.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    if let Err(err) = approval_repo
        .resolve_pending_for_session(session_id, ApprovalStatus::Interrupted)
        .await
    {
        warn!(%err, session_id, "failed to resolve pending clearances on termination");
    }

    // Deregister the ACP driver's in-memory state for this session.
    if let Some(ref acp_driver) = state.acp_driver {
        acp_driver.deregister_session(session_id).await;
    }

    // F-20: clean up any pending thread-reply fallback entries for this session.
    // Dropping the senders causes the spawned waiter tasks to exit cleanly.
    agent_intercom::slack::handlers::thread_reply::cleanup_session_fallbacks(
        session_id,
        &state.pending_thread_replies,
    )
    .await;

    // Notify the operator via Slack (when available).
    let Some(ref slack) = state.slack else { return };
    let (ch, ts) = match session_repo.get_by_id(session_id).await {
        Ok(Some(sess)) => (
            sess.channel_id.unwrap_or_default(),
            sess.thread_ts.map(SlackTs),
        ),
        _ => (state.config.slack.channel_id.clone(), None),
    };
    if !ch.is_empty() {
        let text = format!(
            "\u{1f534} ACP session `{session_id}` terminated \
             (reason: {reason}). Any pending clearances have been cancelled."
        );
        let msg = SlackMessage {
            channel: SlackChannelId(ch),
            text: Some(text),
            blocks: None,
            thread_ts: ts,
        };
        if let Err(err) = slack.enqueue(msg).await {
            warn!(%err, session_id, "failed to post termination notification");
        }
    }
}

/// Post accumulated text to the session's Slack thread.
///
/// Looks up the session's `channel_id` and `thread_ts` from the database,
/// then enqueues a single aggregated message. Silently logs failures
/// without propagating errors.
async fn flush_text_to_slack(state: &Arc<AppState>, session_id: &str, text: &str) {
    use agent_intercom::persistence::session_repo::SessionRepo;
    use agent_intercom::slack::client::SlackMessage;
    use slack_morphism::prelude::{SlackChannelId, SlackTs};

    if text.trim().is_empty() {
        return;
    }

    let Some(ref slack) = state.slack else {
        info!(session_id, "acp text flush: no slack service, logging only");
        info!(session_id, text, "acp agent response");
        return;
    };

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let (channel_id, thread_ts) = match session_repo.get_by_id(session_id).await {
        Ok(Some(sess)) => (sess.channel_id, sess.thread_ts),
        Ok(None) => {
            warn!(session_id, "acp text flush: session not found in db");
            return;
        }
        Err(err) => {
            warn!(%err, session_id, "acp text flush: db lookup failed");
            return;
        }
    };

    let Some(ch) = channel_id else {
        info!(session_id, text, "acp agent response (no channel)");
        return;
    };

    let msg = SlackMessage {
        channel: SlackChannelId(ch),
        text: Some(text.to_owned()),
        blocks: None,
        thread_ts: thread_ts.map(SlackTs),
    };

    if let Err(err) = slack.enqueue(msg).await {
        warn!(%err, session_id, "acp text flush: failed to post to Slack");
    }
}

/// Handle the `ClearanceRequested` ACP event (FR-002, FR-003, FR-010, FR-011).
///
/// Creates and persists an [`ApprovalRequest`], registers it with the ACP
/// driver for response routing, and posts an interactive approval message to
/// the session's Slack thread (directly, to capture the message `ts`).
///
/// Failures at each step are logged and handled gracefully — no panic, no
/// propagation. If the session is not found, the event is discarded silently
/// (with a warning). If the DB write fails, the driver registration is also
/// skipped (SC-003) to avoid unaudited in-memory state.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
async fn handle_clearance_requested(
    state: &Arc<AppState>,
    session_id: &str,
    request_id: &str,
    title: &str,
    description: &str,
    diff: Option<String>,
    file_path: &str,
    risk_level_str: &str,
) {
    use std::path::Path;

    use agent_intercom::diff::validate_workspace_path;
    use agent_intercom::mcp::tools::util::compute_file_hash;
    use agent_intercom::models::approval::{parse_risk_level, ApprovalRequest};
    use agent_intercom::persistence::approval_repo::ApprovalRepo;
    use agent_intercom::persistence::session_repo::SessionRepo;
    use agent_intercom::slack::blocks;
    use agent_intercom::slack::client::SlackMessage;
    use slack_morphism::prelude::{SlackChannelId, SlackTs};

    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    // Step 1: look up the owning session — discard if not found.
    let session = match session_repo.get_by_id(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            warn!(
                session_id,
                "clearance requested for unknown session — discarding"
            );
            return;
        }
        Err(err) => {
            warn!(%err, session_id, "clearance requested: session db lookup failed — discarding");
            return;
        }
    };

    // Step 2: parse risk level (case-sensitive, default Low per FR-011).
    let risk_level = parse_risk_level(risk_level_str);

    // Step 3: validate the file path and compute its hash.
    // On path violation, use the "new_file" sentinel — the approval still
    // proceeds but without an integrity check against a specific file.
    let workspace_root = Path::new(&session.workspace_root);
    let (validated_path, original_hash) = match validate_workspace_path(workspace_root, file_path) {
        Ok(abs_path) => {
            let hash = compute_file_hash(&abs_path).await.unwrap_or_else(|err| {
                warn!(%err, session_id, file_path, "failed to compute file hash");
                "new_file".to_owned()
            });
            (Some(abs_path), hash)
        }
        Err(err) => {
            warn!(%err, session_id, file_path, "path validation failed — using 'new_file' sentinel");
            (None, "new_file".to_owned())
        }
    };
    let effective_file_path = match &validated_path {
        Some(abs_path) => abs_path.strip_prefix(workspace_root).map_or_else(
            |_| file_path.to_owned(),
            |rel| rel.to_string_lossy().into_owned(),
        ),
        None => file_path.to_owned(),
    };

    // Step 4: construct the approval request.
    // Use the agent's `request_id` as `approval.id` so that:
    // - The Slack button value matches the driver registration key
    // - `clearance/response` carries `"id": request_id` (per ACP JSON-RPC correlation)
    // - `approval_repo.get_by_id(request_id)` works in the Slack approval handler
    let diff_content = diff.unwrap_or_default();
    let mut approval = ApprovalRequest::new(
        session_id.to_owned(),
        title.to_owned(),
        Some(description.to_owned()),
        diff_content.clone(),
        effective_file_path.clone(),
        risk_level,
        original_hash,
    );
    approval.id = request_id.to_owned();
    let approval_id = approval.id.clone();

    // Step 5: persist to DB — skip driver registration on failure (SC-003).
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    if let Err(err) = approval_repo.create(&approval).await {
        warn!(%err, session_id, "failed to persist clearance request — skipping registration");
        return;
    }

    // Step 6: register with ACP driver for response routing.
    if let Some(ref acp_driver) = state.acp_driver {
        acp_driver
            .register_clearance(session_id, &approval_id)
            .await;
    }

    // Step 7: post interactive approval message to Slack.
    let Some(ref slack) = state.slack else {
        info!(
            session_id,
            approval_id, "clearance persisted; no Slack service configured"
        );
        return;
    };

    let channel_id = match session.channel_id.as_deref() {
        Some(ch) if !ch.is_empty() => ch.to_owned(),
        _ => {
            if state.config.slack.channel_id.is_empty() {
                info!(
                    session_id,
                    approval_id, "clearance persisted; no Slack channel configured"
                );
                return;
            }
            state.config.slack.channel_id.clone()
        }
    };

    let session_thread_ts = session.thread_ts.as_deref().map(|s| SlackTs(s.to_owned()));

    // RI-002: treat empty description as absent so build_approval_blocks does
    // not render a blank section block in the Slack message.
    let description_opt = if description.is_empty() {
        None
    } else {
        Some(description)
    };

    let mut message_blocks = blocks::build_approval_blocks(
        title,
        description_opt,
        &diff_content,
        &effective_file_path,
        risk_level,
    );
    message_blocks.push(blocks::approval_buttons(&approval_id));

    // C5: post the approval message first so we have a Slack `ts` to use as
    // the thread anchor for the diff file upload.  Previously the upload ran
    // before the post, which left the uploaded file detached from the session
    // thread when `session_thread_ts` was `None`.
    let msg = SlackMessage {
        channel: SlackChannelId(channel_id.clone()),
        text: Some(format!("\u{1f4cb} ACP Approval Request: {title}")),
        blocks: Some(message_blocks),
        thread_ts: session_thread_ts.clone(),
    };

    let posted_ts = match slack.post_message_direct(msg).await {
        Ok(ts) => {
            // If this session had no thread root yet, use the approval post as root.
            if session_thread_ts.is_none() {
                if let Err(err) = session_repo.set_thread_ts(session_id, &ts.0).await {
                    warn!(%err, session_id, "failed to record thread_ts from clearance post");
                }
            }
            // Record the Slack ts so the button-replacement handler can update the message.
            if let Err(err) = approval_repo.update_slack_ts(&approval_id, &ts.0).await {
                warn!(%err, approval_id, "failed to record slack_ts on clearance approval");
            }
            Some(ts)
        }
        Err(err) => {
            warn!(%err, session_id, approval_id, "failed to post clearance approval message to Slack");
            None
        }
    };

    // RI-001 / C5: upload large diffs after posting so the file is attached to
    // the session thread (using the ts we just obtained, falling back to the
    // pre-existing session thread ts).
    let diff_line_count = diff_content.lines().count();
    if diff_line_count > blocks::INLINE_DIFF_THRESHOLD {
        let upload_thread_ts = posted_ts.or(session_thread_ts);
        let sanitized = effective_file_path.replace(['/', '.', '\\'], "_");
        let filename = format!("{sanitized}.diff.txt");
        if let Err(err) = slack
            .upload_file(
                SlackChannelId(channel_id),
                &filename,
                &diff_content,
                upload_thread_ts,
                Some("text"),
            )
            .await
        {
            warn!(%err, session_id, approval_id, "failed to upload diff file to Slack");
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_prompt_forwarded(
    state: &Arc<AppState>,
    session_id: &str,
    prompt_id: &str,
    prompt_text: &str,
    prompt_type_str: &str,
) {
    use agent_intercom::models::prompt::{parse_prompt_type, ContinuationPrompt};
    use agent_intercom::persistence::prompt_repo::PromptRepo;
    use agent_intercom::persistence::session_repo::SessionRepo;
    use agent_intercom::slack::blocks;
    use agent_intercom::slack::client::SlackMessage;
    use slack_morphism::prelude::{SlackChannelId, SlackTs};

    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    // Step 1: look up the owning session — discard if not found.
    let session = match session_repo.get_by_id(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            warn!(
                session_id,
                "prompt forwarded for unknown session — discarding"
            );
            return;
        }
        Err(err) => {
            warn!(%err, session_id, "prompt forwarded: session db lookup failed — discarding");
            return;
        }
    };

    // Step 2: parse prompt type string (case-sensitive, default Continuation).
    let prompt_type = parse_prompt_type(prompt_type_str);

    // Step 3: construct the continuation prompt.
    // Override `.id` with the agent's `prompt_id` so that:
    // - The Slack button value matches the driver registration key
    // - `prompt/response` carries `"id": prompt_id` (ACP JSON-RPC correlation)
    // - `prompt_repo.get_by_id(prompt_id)` works in the Slack prompt handler
    let mut prompt = ContinuationPrompt::new(
        session_id.to_owned(),
        prompt_text.to_owned(),
        prompt_type,
        None, // elapsed_seconds — ACP-specific: not available in event
        None, // actions_taken — ACP-specific: not available in event
    );
    prompt.id = prompt_id.to_owned();
    let prompt_db_id = prompt.id.clone();

    // Step 4: persist to DB — skip driver registration on failure (D3).
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    if let Err(err) = prompt_repo.create(&prompt).await {
        warn!(%err, session_id, prompt_id, "failed to persist prompt forward — skipping registration");
        return;
    }

    // Step 5: register with ACP driver for response routing.
    if let Some(ref acp_driver) = state.acp_driver {
        acp_driver
            .register_prompt_request(session_id, &prompt_db_id)
            .await;
    }

    // Step 6: build Slack blocks.
    let message_blocks =
        blocks::build_prompt_blocks(prompt_text, prompt_type, None, None, &prompt_db_id);

    // Step 7: post to Slack (D2 conditional posting).
    let Some(ref slack) = state.slack else {
        warn!(
            session_id,
            prompt_id, "prompt persisted; no Slack service configured — skipping post"
        );
        return;
    };

    let channel_id = match session.channel_id.as_deref() {
        Some(ch) if !ch.is_empty() => ch.to_owned(),
        _ => {
            if state.config.slack.channel_id.is_empty() {
                warn!(
                    session_id,
                    prompt_id, "prompt persisted; no Slack channel configured — skipping post"
                );
                return;
            }
            state.config.slack.channel_id.clone()
        }
    };

    let session_thread_ts = session.thread_ts.as_deref().map(|s| SlackTs(s.to_owned()));

    let prompt_preview = blocks::truncate_text(prompt_text, 160);
    let msg = SlackMessage {
        channel: SlackChannelId(channel_id),
        text: Some(format!(
            "{} ACP Prompt: {} \u{2014} {}",
            blocks::prompt_type_icon(prompt_type),
            blocks::prompt_type_label(prompt_type),
            prompt_preview,
        )),
        blocks: Some(message_blocks),
        thread_ts: session_thread_ts.clone(),
    };

    if session_thread_ts.is_none() {
        // No thread yet — use direct post to capture ts and anchor the session thread.
        match slack.post_message_direct(msg).await {
            Ok(ts) => {
                if let Err(err) = session_repo.set_thread_ts(session_id, &ts.0).await {
                    warn!(%err, session_id, "failed to record thread_ts from prompt post");
                }
            }
            Err(err) => {
                warn!(%err, session_id, prompt_id, "failed to post prompt message to Slack");
            }
        }
    } else {
        // Thread exists — use rate-limited queue for ordered delivery.
        if let Err(err) = slack.enqueue(msg).await {
            warn!(%err, session_id, prompt_id, "failed to enqueue prompt message to Slack");
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
