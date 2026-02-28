//! Agent Intercom — MCP remote agent server.
//!
//! Provides session lifecycle management, Slack-based human-in-the-loop
//! approval workflows, and a persistence layer for long-running AI agent
//! orchestration.

#![forbid(unsafe_code)]

pub mod audit;
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
