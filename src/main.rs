#![forbid(unsafe_code)]

use clap::{Parser, ValueEnum};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use monocoque_agent_rem::{AppError, Result};

#[derive(Debug, Copy, Clone, Eq, PartialEq, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "monocoque-agent-rem", about = "MCP remote agent server", version, long_about = None)]
struct Cli {
    /// Log output format (text or json).
    #[arg(long, value_enum, default_value_t = LogFormat::Text)]
    log_format: LogFormat,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    init_tracing(args.log_format)?;
    info!("monocoque-agent-rem server bootstrap");
    Ok(())
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
