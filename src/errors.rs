//! Error types shared across the application.

use std::fmt::{Display, Formatter};

/// Shared application result type.
pub type Result<T> = std::result::Result<T, AppError>;

/// Application error enumeration covering all domain failure modes.
#[derive(Debug)]
pub enum AppError {
    /// Configuration parsing or validation failure.
    Config(String),
    /// Persistence failure when interacting with `SQLite`.
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
    /// File-system or I/O operation failure.
    Io(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "config: {msg}"),
            Self::Db(msg) => write!(f, "db: {msg}"),
            Self::Slack(msg) => write!(f, "slack: {msg}"),
            Self::Mcp(msg) => write!(f, "mcp: {msg}"),
            Self::Diff(msg) => write!(f, "diff: {msg}"),
            Self::Policy(msg) => write!(f, "policy: {msg}"),
            Self::Ipc(msg) => write!(f, "ipc: {msg}"),
            Self::PathViolation(msg) => write!(f, "path violation: {msg}"),
            Self::PatchConflict(msg) => write!(f, "patch conflict: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Unauthorized(msg) => write!(f, "unauthorized: {msg}"),
            Self::AlreadyConsumed(msg) => write!(f, "already consumed: {msg}"),
            Self::Io(msg) => write!(f, "io: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<toml::de::Error> for AppError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config(format!("invalid config: {err}"))
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Db(err.to_string())
    }
}
