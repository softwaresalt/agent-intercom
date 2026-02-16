<!-- markdownlint-disable-file -->
# PR Review Status: 001-mcp-remote-agent-server

## Review Status

* Phase: 4 — Handoff Complete
* Last Updated: 2026-02-15T20:15Z
* Summary: All 36 review items dispositioned (34 approved, 1 rejected, 1 merged). Handoff document generated.

## Branch and Metadata

* Normalized Branch: `001-mcp-remote-agent-server`
* Source Branch: `001-mcp-remote-agent-server`
* Base Branch: `main` (via `origin/main`)
* Total Commits: 37
* Total Files Changed: 181
* Lines Added: ~32,700
* Lines Deleted: ~60
* Linked Work Items: spec `specs/001-mcp-remote-agent-server/`

## Build Verification

* `cargo check`: ✅ Pass
* `cargo clippy -- -D warnings`: ✅ Pass (but see RI-01 — lints not inherited)
* `cargo fmt --all -- --check`: Not run (fmt verified in recent phases)

## Diff Mapping — Source Code Files

| File | Type | Lines | Notes |
|------|------|-------|-------|
| `Cargo.toml` | Modified | 82 | Workspace + package config |
| `src/main.rs` | Added | 338 | CLI bootstrap, server startup |
| `src/lib.rs` | Added | 15 | Crate root, re-exports |
| `src/config.rs` | Added | 312 | TOML config, credential loading |
| `src/errors.rs` | Added | 74 | AppError enum |
| `src/mcp/handler.rs` | Added | 465 | MCP ServerHandler, ToolRouter |
| `src/mcp/context.rs` | Added | 52 | Per-request context |
| `src/mcp/sse.rs` | Added | 178 | HTTP/SSE transport |
| `src/mcp/transport.rs` | Added | 38 | Stdio transport |
| `src/mcp/tools/accept_diff.rs` | Added | 263 | Diff acceptance tool |
| `src/mcp/tools/ask_approval.rs` | Added | 310 | Approval request tool |
| `src/mcp/tools/check_auto_approve.rs` | Added | 107 | Policy check tool |
| `src/mcp/tools/forward_prompt.rs` | Added | 279 | Prompt forwarding tool |
| `src/mcp/tools/heartbeat.rs` | Added | 142 | Heartbeat tool |
| `src/mcp/tools/recover_state.rs` | Added | 199 | State recovery tool |
| `src/mcp/tools/remote_log.rs` | Added | 142 | Remote logging tool |
| `src/mcp/tools/set_operational_mode.rs` | Added | 150 | Mode switching tool |
| `src/mcp/tools/wait_for_instruction.rs` | Added | 217 | Wait-for-instruction tool |
| `src/mcp/resources/slack_channel.rs` | Added | 188 | Slack channel resource |
| `src/models/*.rs` | Added | ~585 | Domain models (7 files) |
| `src/persistence/*.rs` | Added | ~853 | DB repos, schema (8 files) |
| `src/orchestrator/*.rs` | Added | ~781 | Session lifecycle (4 files) |
| `src/policy/*.rs` | Added | ~467 | Auto-approve policy (3 files) |
| `src/slack/*.rs` | Added | ~2271 | Slack integration (8 files) |
| `src/diff/*.rs` | Added | ~231 | Diff parsing & application (4 files) |
| `src/ipc/*.rs` | Added | ~368 | IPC server (2 files) |
| `ctl/main.rs` | Added | 146 | CLI companion |
| `tests/**/*.rs` | Added | ~5428 | 29 test modules |

## Instruction Files Reviewed

* `.github/copilot-instructions.md`: Primary — all code conventions, error handling, naming, testing, path security, async rules

## Review Items

### 🔴 Critical Issues

#### RI-01: Workspace lints not inherited by package

* File: `Cargo.toml`
* Category: Configuration / Convention Compliance
* Severity: Critical

**Description**: `[workspace.lints.clippy]` defines `pedantic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`. However the `[package]` section lacks `[lints] workspace = true`, so **none of these lints are enforced during `cargo clippy`**. This means all `.expect()` calls, `.unwrap()` calls, and non-pedantic patterns compile and lint-check cleanly when they should be errors.

**Suggested Resolution**: Add `[lints]` section after `[package]`:
```toml
[lints]
workspace = true
```
Then fix the 6 `.expect()` calls that will surface as errors.

**User Decision**: ✅ Approved

---

#### RI-02: `.expect()` calls in production code (6 instances)

* Files: `src/mcp/sse.rs` (L74, L94, L96), `src/main.rs` (L310, L319), `src/mcp/sse.rs` (L130 — test only)
* Category: Reliability / Convention Compliance
* Severity: Critical (once RI-01 is fixed, these become compile-blocking)

**Description**: Six `.expect()` calls exist in production code. Per project conventions, `clippy::expect_used = "deny"` should prohibit these. They currently compile only because RI-01 means the lint is unenforced.

- `sse.rs:74` — `inbox_for_factory.lock().expect("inbox lock")` — poisoned mutex panics the server
- `sse.rs:94` — `sem.acquire().await.expect("semaphore closed")` — closed semaphore panics the server
- `sse.rs:96` — `inbox.lock().expect("inbox lock")` — second occurrence
- `main.rs:310` — `signal::unix::signal(SignalKind::terminate())?.recv().await.expect("SIGTERM handler")`
- `main.rs:319` — `ctrl_c.await.expect("ctrl-c handler")`
- `sse.rs:130` — test helper (acceptable with `#[allow]`)

**Suggested Resolution**: Replace with fallible handling. For mutex locks, use `match lock { Ok(guard) => ..., Err(poisoned) => poisoned.into_inner() }`. For signal handlers, use `if let Some(()) = sig.recv().await { }`.

**User Decision**: ✅ Approved — fix production code (1–5), `#[allow]` in test (6)

---

#### RI-03: IPC server has no authorization

* File: `src/ipc/server.rs` (entire file)
* Category: Security
* Severity: Critical

**Description**: The IPC server (named pipe on Windows, Unix socket) accepts connections from any local process and exposes commands for approving/rejecting diffs, resuming agents, changing operational modes, and listing sessions. The Slack path has a centralized authorization guard, but the IPC path has none. On multi-user systems this is a privilege escalation vector.

**Suggested Resolution**: Add OS-level pipe ACLs or require a shared secret token validated on connection. At minimum, document the trust boundary assumption ("single-user workstation").

**User Decision**: ✅ Approved — implement shared secret token validation on IPC connect

---

### 🟠 High Issues

#### RI-04: Secret leakage via `Debug` derive on `SlackConfig`

* File: `src/config.rs` L20
* Category: Security
* Severity: High

**Description**: `SlackConfig` derives `Debug`. Since `app_token` and `bot_token` are plain `String` fields, any `{:?}` formatting (e.g., in tracing) will **leak secrets to logs**. 

**Suggested Resolution**: Implement manual `Debug` that redacts token fields:
```rust
impl std::fmt::Debug for SlackConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackConfig")
            .field("app_token", &"[REDACTED]")
            .field("bot_token", &"[REDACTED]")
            .field("channel_id", &self.channel_id)
            .finish()
    }
}
```

**User Decision**: ✅ Approved

---

#### RI-05: UTF-8 panic in `truncate_text` (2 instances) and `truncate_output` (1 instance)

* Files: `src/mcp/tools/forward_prompt.rs` L277, `src/mcp/tools/wait_for_instruction.rs` L215, `src/slack/commands.rs` L848
* Category: Correctness / Reliability
* Severity: High

**Description**: `&text[..max_len.saturating_sub(3)]` and `&s[..max_len]` perform byte-index slicing on `&str`. If the boundary falls inside a multi-byte UTF-8 character (emoji, CJK, accented chars), the program **panics** at runtime. These functions process user-provided text, making non-ASCII input realistic.

**Suggested Resolution**: Use `char_indices()` to find a safe boundary:
```rust
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_owned()
    } else {
        let boundary = text.char_indices()
            .take_while(|(i, _)| *i + 3 < max_len)
            .last()
            .map_or(0, |(i, c)| i + c.len_utf8());
        format!("{}...", &text[..boundary])
    }
}
```

**User Decision**: Pending

---

#### RI-06: `count_active` fails when no sessions exist

* File: `src/persistence/session_repo.rs` L155-L160
* Category: Correctness
* Severity: High

**Description**: `GROUP ALL` with `count()` in SurrealDB returns no rows when the table is empty or no matches occur. The `ok_or_else` then fires with `"failed to count sessions"`. This makes the very first session spawn fail with a spurious error.

**Suggested Resolution**: Default to `Ok(0)` when the row is absent:
```rust
Ok(count_row.map_or(0, |row| row.count))
```

**User Decision**: ✅ Approved — use `Ok(count_row.map_or(0, |row| row.count))`

---

#### RI-07: CLI `--workspace` override not canonicalized

* File: `src/main.rs` L66 + `src/config.rs` L126-L133
* Category: Security / Path Safety
* Severity: High

**Description**: The CLI `--workspace` flag overrides `default_workspace_root` **after** `from_toml_str()` calls `validate()`, which canonicalizes the path. The override path is never re-canonicalized or checked for existence. This bypasses the path safety guarantees the codebase depends on for workspace root validation.

**Suggested Resolution**: Re-run canonicalization after the CLI override, or move the override into `validate()`.

**User Decision**: ✅ Approved — re-canonicalize after CLI override

---

#### RI-08: Tool router rebuilt on every `call_tool()` invocation

* File: `src/mcp/handler.rs` L399
* Category: Performance
* Severity: High

**Description**: `tool_router()` allocates 9 `ToolRoute` objects, clones all `Tool` schemas, and rebuilds the dispatch map on every single MCP tool call. This is unnecessary overhead since the tool set is static.

**Suggested Resolution**: Cache via `std::sync::OnceLock<ToolRouter<Self>>` or `LazyLock` to construct once.

**User Decision**: ✅ Approved — cache the router

---

#### RI-09: Absolute paths silently rewritten in `path_safety.rs`

* File: `src/diff/path_safety.rs` L40-L42
* Category: Security / Correctness
* Severity: High

**Description**: When `RootDir` or `Prefix(_)` is encountered, `normalized` is cleared. An input like `/etc/passwd` becomes `{workspace_root}/etc/passwd` — safe but semantically wrong. The caller likely passed an absolute path by mistake and should receive an error, not a silently rewritten path. This could mask bugs.

**Suggested Resolution**: Return `AppError::PathViolation` for absolute path inputs instead of silently rewriting.

**User Decision**: ✅ Approved — reject absolute paths with `PathViolation`

---

#### RI-10: UTF-8 panic in Slack `truncate_output`

* File: `src/slack/commands.rs` L848
* Category: Correctness / Reliability
* Severity: High

**Description**: `&s[..max_len]` panics when `max_len` falls on a multi-byte UTF-8 boundary. Command output from subprocess execution can contain arbitrary bytes. Same root cause as RI-05 but in the Slack command execution path.

**User Decision**: ✅ Approved — fix all 3 sites with safe truncation + extract to shared utility (also resolves RI-12 duplication)

---

### 🟡 Medium Issues

#### RI-11: `serde_json::to_string().unwrap_or_default()` pattern (4 tool handlers)

* Files: `accept_diff.rs` L43/L244, `ask_approval.rs` L253, `forward_prompt.rs` L209, `wait_for_instruction.rs` L203
* Category: Error Handling
* Severity: Medium

**Description**: 4 of 9 tool handlers use `Content::text(serde_json::to_string(&json).unwrap_or_default())` while others correctly use `Content::json(value)?`. The `unwrap_or_default` path silently returns empty text on serialization failure.

**User Decision**: ✅ Approved — unify to `Content::json()`

---

#### RI-12: Duplicated utility functions

* Files: `compute_file_hash` in accept_diff.rs L254 + ask_approval.rs L301; `truncate_text` in forward_prompt.rs L273 + wait_for_instruction.rs L211
* Category: Maintainability
* Severity: Medium

**Description**: Two pairs of identical functions duplicated across tool modules. Should be extracted to a shared utility module.

**User Decision**: ✅ Approved — extract to shared utility (covered by RI-05)

---

#### RI-13: Blocking `std::fs::read` in async context

* Files: `accept_diff.rs` L255, `ask_approval.rs` L302
* Category: Performance / Conventions
* Severity: Medium

**Description**: `compute_file_hash` uses synchronous `std::fs::read(path)` inside an `async` function. Per workspace conventions, blocking I/O should use `tokio::fs::read` or `tokio::task::spawn_blocking`.

**User Decision**: ✅ Approved — use `tokio::fs::read`

---

#### RI-14: Blanket `From<std::io::Error>` maps all I/O errors to `AppError::Config`

* File: `src/errors.rs` L62-L64
* Category: Error Handling
* Severity: Medium

**Description**: I/O errors from diff application, file writes, or IPC would be miscategorized as `Config` errors. Consider `.map_err()` at specific call sites with appropriate variants.

**User Decision**: ✅ Approved — use `.map_err()` at call sites

---

#### RI-15: Infinite retry loop in Slack client worker

* File: `src/slack/client.rs` L174-L195
* Category: Reliability
* Severity: Medium

**Description**: The message posting loop retries forever on Slack API errors with backoff capped at 30s. If the token is revoked or the API permanently rejects requests, this blocks subsequent messages indefinitely. Should have a max retry count or check a `CancellationToken`.

**User Decision**: ✅ Approved — add max retry count or CancellationToken check

---

#### RI-16: Empty user ID could bypass authorization

* File: `src/slack/events.rs` L106-L108
* Category: Security
* Severity: Medium

**Description**: When `user` is `None`, the user ID defaults to `""`. If `authorized_user_ids` contains an empty string (e.g., trailing comma in TOML array), authorization would be bypassed. Should reject empty user IDs explicitly.

**User Decision**: ✅ Approved — reject empty user IDs before auth check

---

#### RI-17: Non-deterministic IPC `handle_resume`

* File: `src/ipc/server.rs` L296-L307
* Category: Correctness
* Severity: Medium

**Description**: `pending.keys().next()` returns whichever key the `HashMap` iterator yields first — non-deterministic. If multiple sessions are waiting, the wrong one could be resumed.

**User Decision**: ✅ Approved — require session_id argument or use ordered structure

---

#### RI-18: Policy watcher never deferred

* File: `src/policy/watcher.rs` L138-L144
* Category: Reliability
* Severity: Medium

**Description**: If the `.monocoque` directory doesn't exist at registration time, the watcher is "deferred" but no mechanism actually creates it later. Policy changes after server start aren't detected until restart.

**User Decision**: ✅ Approved — document limitation or implement lazy watcher creation

---

#### RI-19: Schema docs claim `IF NOT EXISTS` but DDL doesn't use it

* File: `src/persistence/schema.rs` L4, L12, L19-L91
* Category: Documentation Accuracy
* Severity: Medium

**Description**: Module doc and function doc both claim schema uses `IF NOT EXISTS`, but the actual DDL contains zero such clauses.

**User Decision**: ✅ Approved — fix documentation to match actual behavior

---

#### RI-20: `StallDetectorHandle` discards `JoinHandle`

* File: `src/orchestrator/stall_detector.rs` L308-L311
* Category: Reliability
* Severity: Medium

**Description**: `with_join_handle` parameter is `_handle: JoinHandle<()>` — the task handle is thrown away. No way to await completion or catch panics. If the task panics, it goes silently unnoticed.

**User Decision**: ✅ Approved — store handle for graceful shutdown

---

#### RI-21: Spawner workspace root not canonicalized

* File: `src/orchestrator/spawner.rs` L70-L79
* Category: Security
* Severity: Medium

**Description**: `workspace_root` is used directly as `current_dir` for spawned processes without canonicalization or validation. Should be validated via `path_safety::validate_path`.

**User Decision**: ✅ Approved — canonicalize workspace root

---

#### RI-22: `recover_state` silently converts DB errors to `None`

* File: `src/mcp/tools/recover_state.rs` L82
* Category: Error Handling
* Severity: Medium

**Description**: `repo.get_by_id(sid).await.ok()` maps any DB error to `None`, making it indistinguishable from "session not found".

**User Decision**: ✅ Approved — propagate DB errors, only treat NotFound as None

---

#### RI-23: Missing `//!` doc on `src/lib.rs`

* File: `src/lib.rs`
* Category: Documentation
* Severity: Medium

**Description**: Per convention, all module files need `//!` doc comments.

**User Decision**: ✅ Approved — add crate-level doc comment

---

### 🟢 Low Issues

#### RI-24 through RI-36: Low-severity findings (summary)

| ID | File | Issue | Decision |
|----|------|-------|----------|
| RI-24 | `models/policy.rs` L42 | `risk_level_threshold` is `String` not typed enum | ✅ Approved |
| RI-25 | `persistence/schema.rs` L28-L30 | Untyped datetime fields defeat `SCHEMAFULL` | ✅ Approved |
| RI-26 | `persistence/retention.rs` L60 | String-interpolated table name in SQL (comment needed) | ✅ Approved |
| RI-27 | `slack/events.rs` L130-L132 | `replace_buttons_with_processing` called per action (redundant) | ✅ Approved |
| RI-28 | `slack/commands.rs` L830 | PID `0` sentinel from `child.id().unwrap_or(0)` | ✅ Approved |
| RI-29 | `mcp/tools/accept_diff.rs` L170 | Validated path discarded; raw string re-used | ✅ Approved |
| RI-30 | `mcp/tools/ask_approval.rs` L147 | Unconventional `let _ = &created` pattern | ✅ Approved |
| RI-31 | `mcp/resources/slack_channel.rs` L175 | `clamp_limit` appears unused | ❌ Rejected |
| RI-32 | `stall_detector.rs` L253 | Polling loop (50ms) instead of `Notify` | ✅ Approved |
| RI-33 | `errors.rs` L38-L53 | `Display` has no variant name prefix | ✅ Approved |
| RI-34 | `diff/path_safety.rs` L44-L50 | Double-join path confusing without comment | ✅ Approved |
| RI-35 | `diff/patcher.rs` L49 | Redundant double path validation | ✅ Approved |
| RI-36 | `slack/client.rs` L197-L214 | Socket Mode error handler uses HTTP status code (framework quirk) | ✅ Approved |

## Next Steps

* [x] Review all items RI-01 through RI-36 with user in Phase 3
* [x] Collect user decisions (approve / reject / modify)
* [ ] Finalize handoff document in Phase 4
