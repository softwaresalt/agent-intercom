//! Server protocol mode — which communication protocol the server uses.
//!
//! `ServerMode` is used as the `--mode` CLI flag value. It determines which
//! transport the server initialises at startup.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Top-level protocol mode for the server.
///
/// Passed as `--mode` on the command line. Determines which transport
/// the server starts at startup. Defaults to [`ServerMode::Mcp`].
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerMode {
    /// Model Context Protocol (MCP) transport. Default mode.
    #[default]
    Mcp,
    /// Agent Communication Protocol (ACP) streaming transport.
    Acp,
}
