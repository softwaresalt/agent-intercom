# Research: MCP Remote Agent Server

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)
**Date**: 2026-02-09

## 1. MCP SDK Selection

**Decision**: Use `rmcp` 0.5 — the official Rust SDK from `modelcontextprotocol/rust-sdk`.

**Rationale**: Only official MCP SDK for Rust with `High` source reputation. Supports both stdio and HTTP/SSE transports, server-to-client notifications (including custom method names), and tool routing via `#[tool]` / `#[tool_router]` macros. Integrates directly with `axum` via `StreamableHttpService` tower service.

**Alternatives considered**:

- Community `rmcp` forks on crates.io — lower quality, inconsistent API coverage.
- Building a custom JSON-RPC layer — unnecessary given the official SDK covers all required surfaces.

**Key API patterns**:

- `#[tool]` macro on impl blocks auto-generates JSON schema from Rust types.
- `ServerHandler` trait wires `call_tool` / `list_tools` / `on_initialized` handlers.
- `context.peer.send_notification(ServerNotification::CustomNotification(...))` sends custom notifications to connected clients.
- `Counter::new().serve(stdio()).await?` for stdio transport.
- `StreamableHttpService` mounts onto an axum `Router` via `.nest_service("/mcp", service)`.

## 2. Slack Integration

**Decision**: Use `slack-morphism` — the only actively maintained Rust Slack crate with Socket Mode support.

**Rationale**: Socket Mode provides outbound-only WebSocket connections (no inbound firewall ports), which is a hard requirement. The crate provides `SlackSocketModeClientListener` with `on_message`, `on_error`, and `on_disconnect` hooks. It handles reconnection via `SlackSocketModeClientsManager`.

**Alternatives considered**:

- `slack-api` / `slack-rs` — effectively abandoned, no Socket Mode support.
- Direct WebSocket implementation — unnecessary re-implementation of connection lifecycle management.

**Key patterns for the project**:

- **Diff rendering**: Small diffs (< 20 lines) render inline via `rich_text_preformatted` Block Kit elements. Large diffs upload as `.diff` file snippets via `files.upload`.
- **Interactive buttons**: `actions` block with button elements carrying serialized JSON payloads in the `value` field. After first action, replace buttons with static status via `chat.update`.
- **Modal dialogs**: `views.open` triggered from button interaction payloads for "Refine" and "Nudge with Instructions" free-text input.
- **Rate limits**: `chat.update` is Tier 3 (~50 req/min), sufficient for approval workflows. `chat.postMessage` is Tier 2 (~20/min). Implement exponential backoff queue for `remote_log` bursts.

## 3. Embedded Storage

**Decision**: Use SurrealDB embedded with RocksDB backend (`surrealdb` crate, `kv-rocksdb` feature).

**Rationale**: Provides ACID-compliant embedded database with a document-graph hybrid model that naturally maps Session → Checkpoint → ApprovalRequest relationships via `RELATE` statements. Native async Rust SDK. Graph traversal queries simplify state recovery. Consistent with the existing monocoque project's technology selection. In-memory backend (`kv-mem`) available for tests.

**Alternatives considered**:

- SQLite (`rusqlite`/`sqlx`) — mature but lacks native graph queries; would require join tables for relationship traversal.
- `sled` — too low-level; all querying must be built on top.
- `redb` — MIT licensed, simple KV store, but no query language or relationship modeling.

**License note**: SurrealDB SDK is Apache-2.0; the embedded engine is BSL 1.1. Acceptable for this project because monocoque-agent-rem is standalone developer tooling, not a redistributed library or database service. The BSL restriction (competing database product) is categorically inapplicable.

**Schema design**:

- `SCHEMAFULL` tables with `DEFINE FIELD` and `ASSERT` constraints for type safety at write time.
- `DEFINE TABLE ... TYPE RELATION` for edge tables (`has_checkpoint`, `has_approval`, `has_stall_alert`).
- `BEGIN`/`COMMIT` transactions for atomic state transitions (e.g., approve → consumed → file written).
- Schema DDL executed on startup via `.query()` with `IF NOT EXISTS` for idempotent migrations.

## 4. Diff Application and File Safety

**Decision**: Use `diffy` 0.4 for unified diff parsing/application, `sha2` for content hashing, and `tempfile` for atomic writes.

**Rationale**: `diffy` provides both `Patch::from_str()` parsing and `diffy::apply()` application with built-in context-line matching. SHA-256 hashing is negligible overhead for file-sized inputs and provides cryptographic integrity guarantees. Atomic file writes via `tempfile::NamedTempFile::persist()` use OS-level rename operations.

**Alternatives considered**:

- `similar` crate — more diff algorithms but lacks the integrated `apply()` / unified patch parsing that `diffy` provides.
- Git-based patching (`git apply`) — requires git installation, adds process overhead.
- CRC32 instead of SHA-256 — faster but higher collision risk.

**Safety workflow**:

1. Store SHA-256 hash of original file content when creating a proposal.
2. Before applying: recompute hash of current file, compare to stored value.
3. If hashes match: apply patch via `diffy::apply()`.
4. If hashes differ and `force=false`: return `patch_conflict` error.
5. If hashes differ and `force=true`: attempt `diffy::apply()`, warn operator via Slack.
6. Write to `tempfile::NamedTempFile` in same directory, then `persist()` for atomic rename.
7. Path traversal prevention: canonicalize all paths, verify `starts_with(workspace_root)`, reject `..` segments.

## 5. MCP Server-to-Client Notifications (Nudge Mechanism)

**Decision**: Use `CustomNotification` via `peer.send_notification()` with method name `monocoque/nudge`.

**Rationale**: The `rmcp` crate's `ServerNotification` enum includes a `CustomNotification` variant that accepts arbitrary method names and JSON parameters. No registration required. Available from any `ServerHandler` method via `context.peer`.

**Alternatives considered**:

- Abusing `notifications/resources/updated` — semantically wrong, clients may mishandle.
- `CustomRequest` (request/response) — wrong pattern for fire-and-forget nudges.
- Logging notification (`notify_logging_message`) — misuses logging semantics.

**Transport caveat**: The stdio transport's `should_ignore_notification()` filter may drop custom notifications with non-standard methods during deserialization. The Streamable HTTP transport does not have this limitation. For stdio-connected agents, fall back to `notify_logging_message` with structured nudge data in the log body if custom notifications are not received.

## 6. Process Spawning and Session IPC

**Decision**: Use `tokio::process::Command` for spawning with `kill_on_drop(true)`, `interprocess::local_socket` for cross-platform IPC, and `tokio_util::sync::CancellationToken` for graceful shutdown.

**Rationale**: `tokio::process` integrates with the async runtime for non-blocking process management. `interprocess` abstracts Windows named pipes and Unix domain sockets behind a single API with tokio feature support. `CancellationToken` provides a clean shutdown coordination primitive.

**Alternatives considered**:

- `std::process::Command` — blocks the async runtime.
- `tokio::net::UnixStream` directly — Unix-only; no Windows named pipe support.
- gRPC for IPC — heavyweight for same-machine communication.
- `command-group` crate — worth adding for process group cleanup (agent CLIs may spawn sub-processes).

**Platform differences**:

- Unix: `SIGSTOP`/`SIGCONT`/`SIGTERM` via `nix::sys::signal::kill()`.
- Windows: IPC protocol-based pause/resume (send "pause" command over pipe). `TerminateProcess` for forced kill.
- Abstract via a `ProcessController` trait to encapsulate platform specifics.

**Graceful shutdown protocol**:

1. Receive `SIGTERM`/`SIGINT` → cancel `CancellationToken`.
2. Persist in-flight session state to SurrealDB with status `"interrupted"`.
3. Post shutdown notification to Slack.
4. Send `SIGTERM` to all child processes.
5. Wait with 5-second timeout for children to exit.
6. Force-kill remaining processes.
7. Flush and close DB connection.

## 7. HTTP/SSE Transport for Spawned Sessions

**Decision**: Use `axum` 0.8 with the `rmcp` `StreamableHttpService` for SSE transport.

**Rationale**: The `rmcp` SDK's `StreamableHttpService` implements a tower `Service` and mounts directly onto an axum `Router` via `nest_service`. This is the expected pairing — no adapter code required. Axum's built-in SSE support (`axum::response::sse::Sse`) is available for additional streaming endpoints (e.g., agent process stdout forwarding).

**Alternatives considered**:

- `warp` — compatible with tower but less ecosystem adoption; `axum` is tokio-native.
- `actix-web` — not tower-based; incompatible with `rmcp`'s service integration.

**Configuration**: Primary agent connects via stdio transport. Spawned sessions connect via HTTP/SSE on a configurable local port (default `127.0.0.1:3000`). `LocalSessionManager` handles session state for HTTP connections.

## 8. Stall Detection Architecture

**Decision**: Per-session timer using `tokio::time::Interval` with reset on any MCP activity or `heartbeat` call. Auto-pause during known long-running server operations.

**Rationale**: Each session maintains its own independent stall timer per the spec's requirement that "each session has its own independent stall timer." Using `tokio::time::Interval` with manual reset provides precise control. The `heartbeat` tool resets the timer explicitly. Server-side long operations (e.g., command execution, file writes) pause the timer automatically.

**Escalation chain**:

1. Inactivity threshold exceeded → post stall alert to Slack with last-tool context.
2. Configurable wait period → auto-nudge via `monocoque/nudge` notification.
3. Agent still idle → increment nudge counter, retry up to `max_retries`.
4. Max retries exceeded → escalated alert with `@channel` mention.
5. Agent self-recovers at any point → `chat.update` to dismiss alert, disable buttons.

## 9. Configuration Architecture

**Decision**: TOML global config (`config.toml`) + JSON workspace policy (`.monocoque/settings.json`).

**Rationale**: Follows the existing technical spec's architecture. TOML for the server's global configuration (Slack tokens, workspace root, authorized users, command allowlist, timeouts). JSON for per-workspace auto-approve policies with JSON Schema validation. `notify` crate watches the workspace policy file for hot-reload.

**Hierarchy**: Global `config.toml` defines the absolute security boundary. Workspace `.monocoque/settings.json` can only reduce friction within that boundary, never expand it. Runtime mode overrides supersede workspace policy.
