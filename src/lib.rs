#![forbid(unsafe_code)]

/// Result alias for application code.
pub type Result<T> = std::result::Result<T, AppError>;

/// Placeholder application error to be expanded in foundational work.
#[derive(Debug)]
pub enum AppError {
    /// Feature work not yet implemented.
    Unimplemented,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unimplemented => f.write_str("feature not yet implemented"),
        }
    }
}

impl std::error::Error for AppError {}

pub mod config;
pub mod diff;
pub mod ipc;
pub mod mcp;
pub mod models;
pub mod orchestrator;
pub mod persistence;
pub mod policy;
pub mod slack;
