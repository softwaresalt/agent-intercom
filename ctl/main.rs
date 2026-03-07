#![forbid(unsafe_code)]

//! `agent-intercom-ctl` — local CLI companion for `agent-intercom`.
//!
//! Connects to the IPC socket and sends JSON commands to the server.
//! Designed for local overrides when the operator is physically present.

use std::io::{BufRead, BufReader, Write};

use clap::{Parser, Subcommand};
use interprocess::local_socket::{traits::Stream as _, GenericNamespaced, Stream, ToNsName};

#[derive(Debug, Parser)]
#[command(
    name = "agent-intercom-ctl",
    about = "Local CLI for agent-intercom server",
    version,
    long_about = None
)]
struct Cli {
    /// IPC socket name (must match server's `ipc_name` config).
    ///
    /// When omitted, derived from `--mode`: `agent-intercom` for MCP,
    /// `agent-intercom-acp` for ACP.
    #[arg(long)]
    ipc_name: Option<String>,

    /// Server protocol mode to connect to: `mcp` (default) or `acp`.
    ///
    /// When `--ipc-name` is not set, the mode determines the IPC socket
    /// name: `agent-intercom` for MCP, `agent-intercom-acp` for ACP.
    /// This matches the auto-suffix applied by the server (ADR-0015).
    #[arg(long, value_enum, default_value_t = CtlMode::Mcp)]
    mode: CtlMode,

    #[command(subcommand)]
    command: Command,
}

/// Protocol mode selector for the ctl companion.
///
/// Mirrors [`agent_intercom::mode::ServerMode`] but is defined locally
/// because the ctl binary does not depend on the library crate.
#[derive(Debug, Copy, Clone, Eq, PartialEq, clap::ValueEnum)]
enum CtlMode {
    /// Connect to the MCP server instance (default pipe: `agent-intercom`).
    Mcp,
    /// Connect to the ACP server instance (default pipe: `agent-intercom-acp`).
    Acp,
}

impl Cli {
    /// Resolve the effective IPC socket name.
    ///
    /// If `--ipc-name` was explicitly provided, use it as-is.
    /// Otherwise derive from `--mode`.
    fn effective_ipc_name(&self) -> String {
        if let Some(ref name) = self.ipc_name {
            name.clone()
        } else {
            match self.mode {
                CtlMode::Mcp => "agent-intercom".into(),
                CtlMode::Acp => "agent-intercom-acp".into(),
            }
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List active sessions.
    List,

    /// Approve a pending approval request.
    Approve {
        /// Approval request ID.
        id: String,
    },

    /// Reject a pending approval request.
    Reject {
        /// Approval request ID.
        id: String,
        /// Optional rejection reason.
        #[arg(long)]
        reason: Option<String>,
    },

    /// Resume a waiting agent with optional instruction.
    Resume {
        /// Optional instruction text.
        instruction: Option<String>,
    },

    /// Switch operational mode.
    Mode {
        /// Target mode: remote, local, or hybrid.
        mode: String,
    },

    /// Queue a steering message for the active agent session.
    Steer {
        /// Message text to send to the agent.
        instruction: String,
    },

    /// Queue a task work item for delivery at the next agent cold-start.
    Task {
        /// Task description or instruction text.
        instruction: String,
    },
}

fn main() {
    let args = Cli::parse();

    let request_json = match &args.command {
        Command::List => serde_json::json!({ "command": "list" }),
        Command::Approve { id } => {
            serde_json::json!({ "command": "approve", "id": id })
        }
        Command::Reject { id, reason } => {
            let mut req = serde_json::json!({ "command": "reject", "id": id });
            if let Some(r) = reason {
                req["reason"] = serde_json::Value::String(r.clone());
            }
            req
        }
        Command::Resume { instruction } => {
            let mut req = serde_json::json!({ "command": "resume" });
            if let Some(inst) = instruction {
                req["instruction"] = serde_json::Value::String(inst.clone());
            }
            req
        }
        Command::Mode { mode } => {
            serde_json::json!({ "command": "mode", "mode": mode })
        }
        Command::Steer { instruction } => {
            serde_json::json!({ "command": "steer", "instruction": instruction })
        }
        Command::Task { instruction } => {
            serde_json::json!({ "command": "task", "instruction": instruction })
        }
    };

    let ipc_name = args.effective_ipc_name();

    match send_ipc_command(&ipc_name, &request_json) {
        Ok(response) => {
            if let Some(obj) = response.as_object() {
                let ok = obj
                    .get("ok")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if ok {
                    if let Some(data) = obj.get("data") {
                        println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
                    } else {
                        println!("OK");
                    }
                } else {
                    let err_msg = obj
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown error");
                    eprintln!("Error: {err_msg}");
                    std::process::exit(1);
                }
            } else {
                println!("{response}");
            }
        }
        Err(err) => {
            eprintln!("Failed to connect to server: {err}");
            eprintln!("Is agent-intercom running with ipc_name '{ipc_name}'?");
            std::process::exit(1);
        }
    }
}

/// Connect to the IPC socket, send a JSON command, and read the response.
fn send_ipc_command(
    ipc_name: &str,
    request: &serde_json::Value,
) -> std::result::Result<serde_json::Value, Box<dyn std::error::Error>> {
    let name = ipc_name.to_ns_name::<GenericNamespaced>()?;
    let mut stream = Stream::connect(name)?;

    // Send request as a single JSON line.
    let mut request_line = serde_json::to_string(request)?;
    request_line.push('\n');
    stream.write_all(request_line.as_bytes())?;
    stream.flush()?;

    // Read response line.
    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response: serde_json::Value = serde_json::from_str(response_line.trim())?;
    Ok(response)
}
