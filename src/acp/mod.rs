//! Agent Client Protocol (ACP) stream handling.
//!
//! This module manages bidirectional NDJSON stream communication with
//! headless agent processes spawned by the server. Each ACP session owns
//! a pair of read/write tasks communicating with the agent's stdio.
//!
//! Submodules (added in later phases):
//! - `codec`: [`LinesCodec`](tokio_util::codec::LinesCodec)-based stream framing for NDJSON messages.
//! - `reader`: Async read task that parses incoming agent messages and emits
//!   [`AgentEvent`](crate::driver::AgentEvent)s.
//! - `writer`: Async write task that serializes outbound responses to the agent.
//! - `spawner`: Process spawning with environment isolation and stdio capture.
