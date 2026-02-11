//! Error types shared across the application.

use std::fmt::{Display, Formatter};

/// Shared application result type.
pub type Result<T> = std::result::Result<T, AppError>;

/// Application error enumeration covering all domain failure modes.
#[derive(Debug)]
pub enum AppError {
    /// Configuration parsing or validation failure.
    Config(String),
    /// Persistence failure when interacting with `SurrealDB`.
    Db(String),
    /// Slack API or Socket Mode failure.
    Slack(String),
    /// MCP protocol or tool dispatch failure.
    Mcp(String),
    /// Diff parsing or file-write failure.
    Diff(String),
    /// Policy evaluation or loading failure.
    Policy(String),
    /// IPC communication failure.
    Ipc(String),
    /// File system path failed validation against workspace root.
    PathViolation(String),
    /// Patch application failed due to content divergence.
    PatchConflict(String),
    /// Requested entity does not exist.
    NotFound(String),
    /// Caller is not authorized to perform the requested action.
    Unauthorized(String),
    /// Approval or prompt has already been consumed.
    AlreadyConsumed(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(msg)
            | Self::Db(msg)
            | Self::Slack(msg)
            | Self::Mcp(msg)
            | Self::Diff(msg)
            | Self::Policy(msg)
            | Self::Ipc(msg)
            | Self::PathViolation(msg)
            | Self::PatchConflict(msg)
            | Self::NotFound(msg)
            | Self::Unauthorized(msg)
            | Self::AlreadyConsumed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Config(format!("io error: {err}"))
    }
}

impl From<toml::de::Error> for AppError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config(format!("invalid config: {err}"))
    }
}

impl From<surrealdb::Error> for AppError {
    fn from(err: surrealdb::Error) -> Self {
        Self::Db(err.to_string())
    }
}
