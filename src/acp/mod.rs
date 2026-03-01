//! Agent Client Protocol (ACP) stream handling.
//!
//! This module manages bidirectional NDJSON stream communication with
//! headless agent processes spawned by the server. Each ACP session owns
//! a pair of read/write tasks communicating with the agent's stdio.
//!
//! # Submodules
//!
//! - [`codec`]: [`tokio_util::codec::LinesCodec`]-based NDJSON framing with a
//!   1 MiB per-line limit.
//! - [`handshake`]: LSP-style `initialize` / `initialized` exchange performed
//!   before the reader/writer tasks start (FR-030).
//! - [`reader`]: Async read task that parses incoming agent messages and emits
//!   [`AgentEvent`](crate::driver::AgentEvent)s.
//! - [`writer`]: Async write task that serialises outbound JSON messages to
//!   the agent's stdin.
//! - [`spawner`]: Process spawning with environment isolation and stdio capture.

pub mod codec;
pub mod handshake;
pub mod reader;
pub mod spawner;
pub mod writer;
