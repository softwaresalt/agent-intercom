#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use monocoque_agent_rem::config::GlobalConfig;
use monocoque_agent_rem::mcp::handler::{AppState, PendingApprovals, StallDetectors};
use monocoque_agent_rem::mcp::{sse, transport};
use monocoque_agent_rem::persistence::{db, retention};
use monocoque_agent_rem::slack::client::SlackService;
use monocoque_agent_rem::{AppError, Result};

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "monocoque-agent-rem", about = "MCP remote agent server", version, long_about = None)]
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
    info!("monocoque-agent-rem server bootstrap");

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
        config.default_workspace_root = ws;
    }

    // Load Slack credentials from keyring / env vars.
    config.load_credentials().await?;

    let config = Arc::new(config);
    info!("configuration loaded");

    // ── Initialize database ─────────────────────────────
    let db = Arc::new(db::connect(&config, false).await?);
    info!("database connected");

    // ── Start retention service ──────────────────────────
    let ct = CancellationToken::new();
    let retention_handle =
        retention::spawn_retention_task(Arc::clone(&db), config.retention_days, ct.clone());
    info!("retention service started");

    // ── Build shared application state ──────────────────
    let pending_approvals: PendingApprovals = PendingApprovals::default();

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

    let state = Arc::new(AppState {
        config: Arc::clone(&config),
        db,
        slack: slack_service,
        pending_approvals,
        stall_detectors: Some(StallDetectors::default()),
    });

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

    // ── Graceful shutdown ───────────────────────────────
    let _ = tokio::join!(stdio_handle, sse_handle, retention_handle);
    info!("monocoque-agent-rem shut down");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.expect("ctrl-c handler");
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
