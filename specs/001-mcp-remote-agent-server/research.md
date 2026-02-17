# Research: MCP Remote Agent Server

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)
**Date**: 2026-02-10 (updated from 2026-02-09)

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

**License note**: SurrealDB SDK is Apache-2.0; the embedded engine is BSL 1.1. Acceptable for this project because monocoque-agent-rc is standalone developer tooling, not a redistributed library or database service. The BSL restriction (competing database product) is categorically inapplicable.

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

## 10. Credential Storage (OS Keychain)

**Decision**: Use `keyring` crate for cross-platform OS keychain access, with environment variable fallback.

**Rationale**: `keyring` is the most widely used Rust crate for OS credential storage, supporting Windows Credential Manager, macOS Keychain, and Linux Secret Service. It provides a simple sync API. For the tokio runtime, keychain calls are wrapped in `tokio::task::spawn_blocking()` since they are infrequent (startup only). Environment variables serve as a fallback for environments without a keychain daemon (e.g., headless CI).

**Alternatives considered**:

- `secret-service` crate — Linux-only, no cross-platform support.
- Encrypted config file — adds key management complexity without clear benefit for a single-workstation tool.
- Plaintext config — explicitly rejected by FR-036.

**API pattern**:

```rust
use keyring::Entry;

fn load_credential(service: &str, key: &str, env_var: &str) -> Result<String> {
    Entry::new(service, key)
        .ok()
        .and_then(|e| e.get_password().ok())
        .or_else(|| std::env::var(env_var).ok())
        .ok_or_else(|| ConfigError::MissingCredential {
            key: key.to_string(),
            env_var: env_var.to_string(),
        })
}
```

**Service name**: `monocoque-agent-rc` (consistent across platforms).
**Account names**: `slack_app_token`, `slack_bot_token`.

**Platform caveats**:

- Linux: requires `secret-service` D-Bus daemon (e.g., `gnome-keyring`). Falls back to env vars if unavailable.
- Windows: built-in, no extra setup.
- macOS: built-in, no extra setup.

## 11. Data Retention and Auto-Purge

**Decision**: Background periodic purge task using SurrealQL `DELETE` queries, running hourly via `tokio::time::interval`.

**Rationale**: SurrealDB does not have native TTL or scheduled deletion. A background task with a configurable retention period (default: 30 days) is the simplest reliable approach. Child records (approval requests, checkpoints, continuation prompts, stall alerts) are deleted before parent sessions to maintain referential integrity. Only terminated sessions are purged; active sessions are never touched.

**Alternatives considered**:

- SurrealDB events/triggers — not reliable for periodic purge in embedded mode.
- Application-level TTL fields — requires checking on every read, adds complexity.
- Manual cleanup only — violates FR-035 which requires automatic purge.

**Purge query pattern**:

```sql
-- Find eligible sessions
SELECT id FROM session
WHERE status = 'terminated'
AND updated_at < type::datetime($cutoff);

-- Delete children first (no cascade in SurrealDB)
DELETE FROM approval_request WHERE session_id = $id;
DELETE FROM checkpoint WHERE session_id = $id;
DELETE FROM continuation_prompt WHERE session_id = $id;
DELETE FROM stall_alert WHERE session_id = $id;
DELETE FROM session WHERE id = $id;
```

**Configuration**: `retention_days` field in `config.toml` (default: 30). Purge interval: hourly.

## 12. Multi-Workspace Support

**Decision**: Workspace root is specified per-session rather than as a single global setting. The `Session` model gains a `workspace_root` field. All tool handlers, policy evaluation, and path validation use the session's workspace root.

**Rationale**: The spec was updated to support multiple concurrent IDE workspaces (VS Code, GitHub Copilot CLI, etc.), each with its own agent sessions. The primary stdio agent inherits a default workspace root from `GlobalConfig` (or CLI arguments). Spawned SSE sessions specify their workspace root via connection parameters.

**Architecture changes from v1**:

1. **Session model**: Add `workspace_root: PathBuf` field (required, set at creation).
2. **Tool context**: Thread `workspace_root` from the active session through all tool handlers instead of reading from `GlobalConfig`.
3. **Policy loading**: `PolicyEvaluator` loads `.monocoque/settings.json` relative to the session's workspace root. `notify` watcher is registered per unique workspace root.
4. **Checkpoint model**: Captures `workspace_root` at checkpoint time for restore fidelity.
5. **Session spawning**: `/monocoque session-start` accepts an optional workspace path argument. Spawned agents receive workspace root via `MONOCOQUE_WORKSPACE_ROOT` environment variable.
6. **Path validation**: `validate_path()` already accepts `workspace_root: &Path` — no change needed.
7. **GlobalConfig**: `workspace_root` remains as the default for the primary stdio agent. Removed as a hard requirement; becomes optional with per-session override.

## 13. Observability Architecture

**Decision**: Structured tracing spans to stderr via `tracing-subscriber` with `env-filter` and `fmt` features. No metrics endpoint or external collector.

**Rationale**: For a single-workstation CLI tool, full observability infrastructure (Prometheus, OpenTelemetry) is unnecessary overhead. `tracing` spans provide sufficient debugging context with zero runtime dependency. The `RUST_LOG` environment variable controls verbosity. JSON output format available via `--log-format json` flag for machine consumption.

**Span coverage**:

- MCP tool call execution (tool name, session ID, duration, result status)
- Slack API interactions (method name, response status, rate limit headers)
- Stall detection events (session ID, idle duration, action taken)
- Session lifecycle transitions (session ID, old state → new state)
- Diff application (file path, hash match, write result)
- Policy evaluation (tool/command checked, matched rule, auto-approve decision)
- Credential loading (source: keychain or env var, key name — never the value)

## 14. Slack Environment Variable Configuration (US11)

**Decision**: Existing `load_credential()` implementation already satisfies FR-038 through FR-041. No new code required — only explicit specification and test coverage.

**Rationale**: The `config.rs` `load_credential()` function already implements the keychain-first, env-var-fallback pattern for `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, and `SLACK_TEAM_ID`. The `load_credentials()` method calls this for all three credentials. The `SlackConfig` struct already has `#[serde(skip)]` fields for `app_token`, `bot_token`, and `team_id` that are populated at runtime.

**Verification needed**:

- Confirm error messages when credentials are missing are clear and actionable (identify both keychain service name and env var name).
- Confirm `SLACK_TEAM_ID` is truly optional — current code in `slack/client.rs` handles empty team_id by connecting without workspace scoping.
- Add dedicated test coverage for the env-var-only path, keychain-takes-precedence path, and missing-credential error message path.

**Alternatives considered**: None — the existing implementation is correct. This user story formalizes it.

## 15. Dynamic Slack Channel Selection via Query String (US12)

**Decision**: Existing `extract_channel_id()` and `channel_id_override` implementation in `src/mcp/sse.rs` and `src/mcp/handler.rs` already satisfies FR-042 through FR-044. No new code required — only explicit specification, config documentation, and test coverage.

**Rationale**: The SSE transport already extracts `channel_id` from the query string, passes it to `AgentRcServer::with_channel_override()`, and the `effective_channel_id()` method returns the override or the default. Each SSE connection gets its own `AgentRcServer` instance, ensuring session isolation. A semaphore-protected inbox pattern prevents race conditions during concurrent connection establishment.

**Verification needed**:

- Confirm the semaphore-inbox pattern correctly handles rapid concurrent SSE connections (each factory call consumes the right channel_id).
- Add integration test for two concurrent SSE sessions with different channel_ids posting to different channels independently.
- Confirm stdio transport always uses the default channel (no override mechanism).
- Document the `?channel_id=` parameter in `quickstart.md` and `config.toml` comments.

**Alternatives considered**: None — the existing implementation is correct. This user story formalizes it.

## 16. Service Rebranding from agent-rem to agent-rc (US13)

**Decision**: Mechanical rename of all occurrences of `agent-rem` / `agent_rem` to `agent-rc` / `agent_rc` across the entire codebase. No migration tooling for old keychain entries or SurrealDB databases.

**Rationale**: The rename is a one-time mechanical change performed before external users adopt the current naming. Providing automatic migration would add complexity for a scenario that only affects internal development (no external users exist yet). The operator re-stores credentials under the new service name; the server creates a fresh database.

**Scope analysis (from codebase grep)**:

| Category | Files affected | Approximate occurrences |
|----------|---------------|------------------------|
| Source code (`src/`) | `main.rs`, `config.rs`, `handler.rs`, `db.rs`, `slack_channel.rs` | ~36 |
| CLI (`ctl/main.rs`) | 1 | ~4 |
| Tests (`tests/`) | Multiple | ~75 |
| Cargo.toml | 1 | ~2 (package name, binary name) |
| config.toml | 1 | ~4 |
| README.md | 1 | TBD |
| Spec/plan docs (`specs/`) | Multiple | Many (documentation references) |
| Constitution | 1 | ~5 |
| Agent context files | 1 | TBD |

**What changes**:

- Cargo.toml: `name = "monocoque-agent-rc"`, `[[bin]] name = "monocoque-agent-rc"`
- SurrealDB: namespace stays `monocoque`, database changes from `agent_rem` to `agent_rc`
- Keychain service: `monocoque-agent-rc` (was `monocoque-agent-rem`)
- IPC pipe name: `monocoque-agent-rc` (default in config)
- CLI help text and tracing output
- All `use monocoque_agent_rem::` imports become `use monocoque_agent_rc::`
- All test extern crate references

**What does NOT change**:

- `monocoque/nudge` notification method prefix (the `monocoque` namespace is the product, not the binary name)
- `monocoque-ctl` binary name (it's the control CLI, not the agent binary)
- `.monocoque/` workspace config directory name
- `/monocoque` Slack slash command prefix
- Repository name (that's a separate GitHub operation, out of scope for the code rename)

**Risk assessment**:

- Low risk — all changes are string replacements with no logic changes.
- Compilation (`cargo build`) serves as the primary verification gate — any missed reference will fail to compile.
- `SC-015` (grep validation) serves as the final verification gate.

**Alternatives considered**:

- Providing a migration script for keychain and DB — rejected per spec (US13 acceptance scenario 7: "server does NOT automatically migrate keychain entries").
- Renaming the repository simultaneously — out of scope; can be done independently.
- Keeping `agent-rem` as an alias — adds confusion; clean break is simpler.
