#![forbid(unsafe_code)]

pub mod config;
pub mod diff;
pub mod errors;
pub mod ipc;
pub mod mcp;
pub mod models;
pub mod orchestrator;
pub mod persistence;
pub mod policy;
pub mod slack;

pub use config::GlobalConfig;
pub use errors::{AppError, Result};
