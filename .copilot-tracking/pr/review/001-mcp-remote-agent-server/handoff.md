<!-- markdownlint-disable-file -->
# PR Review Handoff: 001-mcp-remote-agent-server

## PR Overview

Full implementation of the MCP remote agent server — a Slack-integrated, MCP-based supervisory control plane for remote AI agent sessions. Covers phases 1–17 of the feature spec.

* Branch: `001-mcp-remote-agent-server`
* Base Branch: `main`
* Total Files Changed: 181
* Total Review Comments: 34 approved, 1 rejected
* Build Status: `cargo check` ✅ | `cargo clippy` ✅ (caveat: workspace lints not inherited)

## PR Comments Ready for Submission

---

### File: Cargo.toml

#### Comment 1 (Package section, after line 46)

* Category: Configuration
* Severity: 🔴 Critical

Workspace lints (`pedantic = "deny"`, `unwrap_used = "deny"`, `expect_used = "deny"`) are defined in `[workspace.lints.clippy]` but the package never inherits them. None of the lint discipline is enforced.

**Suggested Change**

Add after the `[package]` section:

```toml
[lints]
workspace = true
```

---

### File: src/mcp/sse.rs

#### Comment 2 (Lines 74, 94, 96)

* Category: Reliability
* Severity: 🔴 Critical

Three `.expect()` calls in production code violate `clippy::expect_used = "deny"` (unenforced due to Comment 1). Poisoned mutex or closed semaphore will panic the SSE server.

**Suggested Change**

```rust
// Line 74: replace .lock().expect("inbox lock").take()
let channel_override = match inbox_for_factory.lock() {
    Ok(mut guard) => guard.take(),
    Err(poisoned) => poisoned.into_inner().take(),
};

// Line 94: replace .acquire().await.expect("semaphore closed")
let Ok(_permit) = sem.acquire().await else {
    return next.run(request).await;
};

// Line 96: replace .lock().expect("inbox lock")
match inbox.lock() {
    Ok(mut guard) => *guard = channel_id,
    Err(mut poisoned) => *poisoned.get_mut() = channel_id,
};
```

Also add `#[allow(clippy::expect_used)]` to the test helper `parse_uri` at line 130.

---

### File: src/main.rs

#### Comment 3 (Lines 310, 319)

* Category: Reliability
* Severity: 🔴 Critical

Two `.expect()` calls in `shutdown_signal()`. Signal handler registration failure and Ctrl+C handler failure cause panics.

**Suggested Change**

```rust
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                _ = ctrl_c => {}
                _ = sigterm.recv() => {}
            }
        } else {
            // Fall back to ctrl-c only if SIGTERM handler cannot be registered.
            let _ = ctrl_c.await;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
```

#### Comment 4 (Line 64)

* Category: Security / Path Safety
* Severity: 🟠 High

CLI `--workspace` override is applied after `validate()` canonicalizes `default_workspace_root`. The override path is never re-canonicalized.

**Suggested Change**

```rust
if let Some(ws) = args.workspace {
    config.default_workspace_root = std::fs::canonicalize(&ws)
        .map_err(|e| AppError::Config(format!("invalid workspace path '{ws}': {e}")))?
        .to_string_lossy()
        .into_owned();
}
```

---

### File: src/ipc/server.rs

#### Comment 5 (Entire file)

* Category: Security
* Severity: 🔴 Critical

IPC server has no authorization. Any local process can connect and approve/reject diffs, resume agents, change modes, or list sessions. Implement shared secret token validation on IPC connect — require a `--token` on connect, validated against a value from `config.toml` or keyring.

---

### File: src/config.rs

#### Comment 6 (Line 17)

* Category: Security
* Severity: 🟠 High

`SlackConfig` derives `Debug`, exposing `app_token` and `bot_token` in log output.

**Suggested Change**

```rust
#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SlackConfig {
    // ... fields unchanged ...
}

impl std::fmt::Debug for SlackConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackConfig")
            .field("app_token", &"[REDACTED]")
            .field("bot_token", &"[REDACTED]")
            .field("channel_id", &self.channel_id)
            .field("team_id", &"[REDACTED]")
            .finish()
    }
}
```

---

### File: src/mcp/tools/forward_prompt.rs, src/mcp/tools/wait_for_instruction.rs, src/slack/commands.rs

#### Comment 7 (forward_prompt.rs L273–279, wait_for_instruction.rs L211–218, commands.rs L844–853)

* Category: Correctness
* Severity: 🟠 High

`truncate_text` and `truncate_output` perform byte-index slicing (`&text[..n]`) which panics on multi-byte UTF-8 boundaries. These process user-provided text.

**Suggested Change**

Extract a shared utility (e.g., `src/mcp/tools/mod.rs` or a new `src/util.rs`):

```rust
/// Truncate text to a maximum byte length at a valid UTF-8 boundary,
/// appending a suffix if truncated.
pub(crate) fn truncate_text(text: &str, max_len: usize, suffix: &str) -> String {
    if text.len() <= max_len {
        text.to_owned()
    } else {
        let target = max_len.saturating_sub(suffix.len());
        let boundary = text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= target)
            .last()
            .unwrap_or(0);
        format!("{}{suffix}", &text[..boundary])
    }
}
```

Replace all three call sites. Remove the duplicated `truncate_text` from `forward_prompt.rs` and `wait_for_instruction.rs`, and adapt `truncate_output` in `commands.rs` to use the shared function.

Also extract `compute_file_hash` from `accept_diff.rs` and `ask_approval.rs` into a shared location, and convert to use `tokio::fs::read` instead of blocking `std::fs::read`.

---

### File: src/persistence/session_repo.rs

#### Comment 8 (Lines 155–160)

* Category: Correctness
* Severity: 🟠 High

`count_active` returns `Err("failed to count sessions")` when zero sessions exist because SurrealDB's `GROUP ALL` returns no rows on empty results.

**Suggested Change**

```rust
Ok(count_row.map_or(0, |row| row.count))
```

---

### File: src/mcp/handler.rs

#### Comment 9 (Line 399)

* Category: Performance
* Severity: 🟠 High

`tool_router()` is rebuilt (9 `ToolRoute` allocations + schema clones) on every `call_tool()` invocation. Cache via `OnceLock` or store as a field initialized during construction.

---

### File: src/diff/path_safety.rs

#### Comment 10 (Lines 40–42)

* Category: Security
* Severity: 🟠 High

Absolute paths are silently rewritten (prefix stripped, joined with workspace root) instead of rejected. Return `PathViolation` for absolute path inputs.

**Suggested Change**

```rust
Component::RootDir | Component::Prefix(_) => {
    return Err(AppError::PathViolation(
        format!("absolute path not allowed: {}", candidate.as_ref().display()),
    ));
}
```

---

### File: src/mcp/tools/accept_diff.rs, src/mcp/tools/ask_approval.rs, src/mcp/tools/forward_prompt.rs, src/mcp/tools/wait_for_instruction.rs

#### Comment 11 (accept_diff L43/L244, ask_approval L253, forward_prompt L209, wait_for_instruction L203)

* Category: Error Handling
* Severity: 🟡 Medium

Four tool handlers use `Content::text(serde_json::to_string(&json).unwrap_or_default())` which silently returns empty text on serialization failure. Others correctly use `Content::json()`. Unify to `Content::json()` throughout.

---

### File: src/errors.rs

#### Comment 12 (Lines 62–64)

* Category: Error Handling
* Severity: 🟡 Medium

Blanket `From<std::io::Error>` maps all I/O errors to `AppError::Config`. I/O errors from diff application, file writes, or IPC are miscategorized. Use `.map_err()` at specific call sites with appropriate variants (`Diff`, `Ipc`, etc.) instead.

#### Comment 13 (Lines 38–53)

* Category: Observability
* Severity: 🟢 Low

`Display` impl writes only the inner message with no variant name prefix. Consider prefixing with the variant for log debuggability (e.g., `"config: io error: ..."` instead of `"io error: ..."`).

---

### File: src/slack/client.rs

#### Comment 14 (Lines 174–205)

* Category: Reliability
* Severity: 🟡 Medium

`spawn_worker` retries forever on Slack API errors. If the token is revoked, this blocks all subsequent messages indefinitely. Add a max retry count or check a `CancellationToken`.

#### Comment 15 (Lines 216–218)

* Category: Documentation
* Severity: 🟢 Low

Socket Mode error handler returns `axum::http::StatusCode::INTERNAL_SERVER_ERROR` — this is a `slack-morphism` API requirement, not HTTP. Add a comment: `// Required by slack-morphism error handler signature`.

---

### File: src/slack/events.rs

#### Comment 16 (Lines 106–108)

* Category: Security
* Severity: 🟡 Medium

Empty user ID (`""`) from unauthenticated webhook could bypass `is_authorized` if `authorized_user_ids` contains an empty string. Reject empty user IDs explicitly before the auth check.

**Suggested Change**

```rust
let user_id = block_event
    .user
    .as_ref()
    .map(|u| u.id.to_string())
    .unwrap_or_default();

if user_id.is_empty() {
    warn!("block action with no user identity; dropping");
    return Ok(());
}
```

#### Comment 17 (Lines 130–132)

* Category: Performance
* Severity: 🟢 Low

`replace_buttons_with_processing` is called per action in the loop. Move it before the action loop since it only needs to run once per message.

---

### File: src/ipc/server.rs

#### Comment 18 (Lines 296–307)

* Category: Correctness
* Severity: 🟡 Medium

`handle_resume` picks the first pending wait via `HashMap::keys().next()` which is non-deterministic. If multiple sessions are waiting, the wrong one could be resumed. Require a `session_id` argument or use an ordered data structure.

---

### File: src/policy/watcher.rs

#### Comment 19 (Lines 138–144)

* Category: Reliability
* Severity: 🟡 Medium

When `.agentrc` directory doesn't exist at registration time, the watcher is "deferred" but no mechanism creates it later. Policy changes after server start aren't detected until restart. Document this limitation or implement lazy watcher creation (e.g., watch the parent directory for directory creation events).

---

### File: src/persistence/schema.rs

#### Comment 20 (Lines 3–4, 12)

* Category: Documentation Accuracy
* Severity: 🟡 Medium

Module doc and function doc both claim schema uses `IF NOT EXISTS`, but the actual DDL contains zero `IF NOT EXISTS` clauses. Fix documentation to match reality (SurrealDB 1.x `DEFINE TABLE` is idempotent without the clause).

#### Comment 21 (Lines 28–30)

* Category: Schema
* Severity: 🟢 Low

`created_at`, `updated_at`, `terminated_at` fields have no `TYPE` constraint, undermining `SCHEMAFULL`. Add `TYPE option<datetime>` (or `TYPE datetime` where non-nullable).

---

### File: src/orchestrator/stall_detector.rs

#### Comment 22 (Lines 302–311)

* Category: Reliability
* Severity: 🟡 Medium

`with_join_handle` discards the `JoinHandle` (`_handle: JoinHandle<()>`). Task panics go silently unnoticed. Store the handle in the struct and expose it for graceful shutdown.

#### Comment 23 (Lines 253–261)

* Category: Performance
* Severity: 🟢 Low

`wait_unless_paused` polls every 50ms. A `tokio::sync::Notify` for pause/resume transitions would be more efficient.

---

### File: src/orchestrator/spawner.rs

#### Comment 24 (Lines 70–79)

* Category: Security
* Severity: 🟡 Medium

`workspace_root` is used directly as `current_dir` for spawned processes without canonicalization. Validate via `path_safety::validate_path` or `std::fs::canonicalize` before use.

#### Comment 25 (Line 86)

* Category: Clarity
* Severity: 🟢 Low

`child.id().unwrap_or(0)` — PID `0` is misleading. Use `"unknown"` in the log or `Option<u32>` display.

---

### File: src/mcp/tools/recover_state.rs

#### Comment 26 (Line 89)

* Category: Error Handling
* Severity: 🟡 Medium

`repo.get_by_id(sid).await.ok()` maps DB connection errors to `None`, making them indistinguishable from "session not found". Propagate errors; only treat "not found" as `None`.

---

### File: src/lib.rs

#### Comment 27 (Line 1)

* Category: Documentation
* Severity: 🟡 Medium

Missing `//!` crate-level doc comment per convention.

**Suggested Change**

```rust
//! Monocoque Agent RC — MCP remote control server for AI agent supervision.

#![forbid(unsafe_code)]
```

---

### File: src/persistence/retention.rs

#### Comment 28 (Line 60)

* Category: Safety
* Severity: 🟢 Low

String-interpolated table name in SQL. While hardcoded and safe, add a comment: `// Safe: table name is from CLEANUP_TABLES constant, not user input`.

---

### File: src/mcp/tools/accept_diff.rs

#### Comment 29 (Line 170)

* Category: Correctness
* Severity: 🟢 Low

Validated path from `validate_path()` at L110 is discarded. A fresh `PathBuf::from(&approval.file_path)` is used instead. Use the validated path for file operations.

---

### File: src/mcp/tools/ask_approval.rs

#### Comment 30 (Line 147)

* Category: Code Style
* Severity: 🟢 Low

`let _ = &created;` — unconventional suppression pattern. Use `let _created = repo.create(&approval).await?;` directly.

---

### File: src/diff/path_safety.rs

#### Comment 31 (Lines 44–50)

* Category: Clarity
* Severity: 🟢 Low

After the normalization loop, the double-join logic is confusing without a comment. Add an explanatory comment for the `is_absolute()` / `root.join()` branch.

---

### File: src/diff/patcher.rs

#### Comment 32 (Line 49)

* Category: Performance
* Severity: 🟢 Low

`write_full_file` re-validates the path internally after it was already validated at line 30. Redundant canonicalization. Consider a non-validating internal write helper or pass the validated path.

---

### File: src/models/policy.rs

#### Comment 33 (Line 42)

* Category: Type Safety
* Severity: 🟢 Low

`risk_level_threshold` is `String` not a typed enum. Typos like `"lo"` or `"HIGH"` are silently accepted. Consider using a dedicated `RiskLevel` enum.

---

### File: src/slack/commands.rs

#### Comment 34 (Line 830)

* Category: Clarity
* Severity: 🟢 Low

`output.status.code().unwrap_or(-1)` — document that `-1` means signal-terminated (Unix) or use a more descriptive representation.

---

## Review Summary by Category

* 🔒 Security Issues: 5 (RI-03, RI-04, RI-09, RI-16, RI-21)
* ⚙️ Correctness: 5 (RI-05/10, RI-06, RI-17, RI-29, RI-22)
* 🛡️ Reliability: 5 (RI-02, RI-15, RI-18, RI-20, RI-32)
* 🧰 Configuration: 1 (RI-01)
* 📐 Code Quality / Duplication: 4 (RI-08, RI-11, RI-12, RI-13)
* 🔍 Error Handling: 3 (RI-14, RI-22, RI-33)
* 📖 Documentation: 5 (RI-19, RI-23, RI-25, RI-26, RI-36)
* 🎨 Code Style: 4 (RI-24, RI-27, RI-28, RI-30)

## Instruction Compliance

* ✅ `.github/copilot-instructions.md`: Nearly all rules followed
* ⚠️ Lint inheritance gap: `[lints] workspace = true` missing — all clippy deny rules are unenforced
* ⚠️ `expect_used` violations: 5 production `.expect()` calls (blocked once lint inheritance is fixed)
* ⚠️ Blocking I/O in async: `std::fs::read` used instead of `tokio::fs::read` / `spawn_blocking`
* ⚠️ Missing `//!` doc on `src/lib.rs`

## Outstanding Risks

1. **RI-01 has cascading impact** — enabling lint inheritance will surface additional `pedantic` clippy warnings beyond the 5 known `.expect()` calls. Budget time for a lint remediation pass.
2. **IPC auth (RI-03)** is the largest implementation effort — requires new config fields, shared secret generation, and token validation on every IPC connection. Consider defining this as a follow-up ADR.
3. **Test impact** — fixing `path_safety.rs` absolute path rejection (RI-09) may break existing tests that pass absolute paths. Verify all `validate_path` call sites.
