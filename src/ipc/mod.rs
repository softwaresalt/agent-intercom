//! Local IPC layer for `monocoque-ctl` interaction.
//!
//! Provides a named pipe (Windows) or Unix domain socket (Linux/macOS)
//! server that accepts JSON-line commands from the companion CLI.

pub mod server;
pub mod socket;
