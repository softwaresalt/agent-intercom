<!-- markdownlint-disable-file -->
# PR Review Handoff: 004-intercom-advanced-features

## PR Overview

Feature 004 adds 13 implementation phases covering operator steering queue, task inbox, server reliability (stall detection, child process monitoring, graceful shutdown), Slack modal capture, SSE disconnect cleanup, policy hot-reload with pre-compiled RegexSet, audit logging, detail levels, auto-approve suggestion with pattern generation, ping fallback, approval file attachment with snippets, and polish.

* Branch: `004-intercom-advanced-features`
* Base Branch: `main`
* Total Files Changed: 127
* Lines Added: ~11,319
* Lines Deleted: ~710
* New Files: 51
* Modified Files: 76
* Total Review Comments: 9 (8 actionable + 1 informational)

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo check` | ✅ Pass |
| `cargo clippy -- -D warnings` | ✅ Pass — zero warnings |
| `cargo fmt --check` | ✅ Pass |
| `cargo test` | ✅ Pass — 239 tests + 2 doc-tests, 0 failures |

## PR Comments Ready for Submission

### File: src/audit/writer.rs

#### Comment 1 — RI-01 (Lines 69–99)

* Category: Code Quality / Error Handling
* Severity: Low

`AppError::Config` is used for I/O failures in `log_entry` (file open, write, flush) and in `open_for_date`. The `Config` variant semantically represents configuration errors (missing settings, bad TOML), not runtime I/O failures. This conflation will make it harder to distinguish config issues from I/O errors in structured logging and error recovery paths.

**Suggested Change**

Add an `Io` variant to `AppError` in `src/errors.rs` and use it for all file-system operations in the audit writer:

```rust
// In src/errors.rs
Io(String),

// In src/audit/writer.rs — replace AppError::Config with AppError::Io
.map_err(|e| crate::AppError::Io(format!("failed to open audit log {}: {e}", path.display())))?;
```

---

### File: src/slack/handlers/steer.rs

#### Comment 2 — RI-02 (Lines 155–163)

* Category: Reliability
* Severity: Medium

`strip_mention` uses `line.find('>').map(|i| i + 1)` to find the end of a `<@U...>` mention token, then slices `&line[skip..]`. If the `>` sits at a multi-byte character boundary (e.g. from a pasted emoji right before the mention), the byte offset from `find` could split a UTF-8 sequence, causing a panic.

**Suggested Change**

Use `char_indices` or `str::split_once` instead of byte-offset slicing:

```rust
fn strip_mention(line: &str) -> &str {
    if line.starts_with("<@") {
        line.split_once('>')
            .map_or(line, |(_, rest)| rest.trim_start())
    } else {
        line
    }
}
```

---

### File: src/mcp/handler.rs

#### Comment 3 — RI-03 (Lines 740–764)

* Category: Performance
* Severity: Low

`call_tool` resets the stall timer for **all** active sessions by iterating every entry in the `StallDetectors` map — both before and after the tool call. Each tool invocation acquires the mutex twice and touches every session's `AtomicBool`. With N concurrent sessions, this is O(N) per tool call.

**Suggested Change**

Reset only the calling session's detector by looking up the session ID recorded in `session_db_id` or `session_id_override`:

```rust
if let Some(ref detectors) = state.stall_detectors {
    if let Some(ref sid) = session_id {
        let guards = detectors.lock().await;
        if let Some(handle) = guards.get(sid) {
            handle.reset();
        }
    }
}
```

---

### File: src/slack/handlers/steer.rs

#### Comment 4 — RI-04 (Lines 47–53)

* Category: Correctness / Multi-session
* Severity: Medium

`store_from_slack` picks the first active session from the database regardless of the originating Slack channel. When multiple workspaces are active in different channels, a steering message posted in channel A could be delivered to a session running in channel B.

**Suggested Change**

Acceptable for the single-workspace-per-server model in 004. **Deferred to Feature 005** where multi-workspace support and workspace-to-channel mapping will be introduced. Add a TODO comment:

```rust
// TODO(005): Filter sessions by channel_id when multi-workspace routing is available.
```

Added to [.context/backlog.md](.context/backlog.md) under "Deferred from 004 PR Review".

---

### File: src/slack/handlers/command_approve.rs

#### Comment 5 — RI-05 (Lines 195–243 and 262–309)

* Category: Data Integrity
* Severity: Medium

`write_pattern_to_workspace_file` and `write_pattern_to_vscode_settings` strip all JSONC line comments via `strip_jsonc_line_comment` before parsing, then re-serialize with `serde_json::to_writer_pretty`. The round-trip **permanently deletes every comment** from the target file. A single auto-approve click could silently wipe dozens of documentation comments the operator maintains in their workspace configuration.

**Suggested Change**

Replace the strip-parse-rewrite strategy with a JSONC-aware crate like `jsonc-parser` that preserves comments and formatting during targeted modifications. This prevents destructive loss of operator-maintained comments in `*.code-workspace` and `.vscode/settings.json` files.

---

### File: src/mcp/tools/check_auto_approve.rs

#### Comment 6 — RI-06 (Lines 130–215)

* Category: Observability / Security
* Severity: Medium

The audit log block at line ~217 only executes for the non-blocking policy evaluation path. When `kind = "terminal_command"` and the command is not auto-approved, the function enters the blocking Slack approval gate and returns early at line ~199, **completely bypassing the audit log**. Operator-approved and operator-rejected terminal commands are not audited.

**Suggested Change**

Add audit logging inside the terminal command gate, just before the early return. Use the existing `with_request_id` builder (per RI-09):

```rust
if let Some(ref logger) = state.audit_logger {
    let event_type = if approved {
        AuditEventType::CommandApproval
    } else {
        AuditEventType::CommandRejection
    };
    let entry = AuditEntry::new(event_type)
        .with_session(session.id.clone())
        .with_command(input.tool_name.clone())
        .with_request_id(request_id.clone());
    if let Err(err) = logger.log_entry(entry) {
        warn!(%err, "audit log write failed (terminal command gate)");
    }
}
```

---

### File: src/slack/handlers/command_approve.rs

#### Comment 7 — RI-07 (Lines 97, 184, 262 — called from async handler at ~347–362)

* Category: Performance / Reliability
* Severity: Medium

`handle_auto_approve_action` calls three synchronous file I/O functions directly from the async Slack event handler: `write_pattern_to_settings`, `write_pattern_to_workspace_file`, and `write_pattern_to_vscode_settings`. Each performs multiple `std::fs` operations that block the Tokio executor thread.

**Suggested Change**

Wrap the synchronous file I/O in `tokio::task::spawn_blocking`, consistent with the project's async convention for blocking I/O (used for `keyring` credential lookups):

```rust
let settings_result = tokio::task::spawn_blocking({
    let settings_path = settings_path.clone();
    let command = command.to_owned();
    move || write_pattern_to_settings(&settings_path, &command)
})
.await
.map_err(|e| format!("spawn_blocking join error: {e}"))?
.map_err(|e| format!("failed to write auto-approve pattern: {e}"))?;
```

---

### File: src/mcp/handler.rs

#### Comment 8 — RI-08 (Lines 571–595 and 700–726)

* Category: Maintainability / Code Quality
* Severity: Low

The stall detector spawning logic (~25 lines) is copy-pasted verbatim in Case 1 (spawned agent) and Case 2 (direct connection) of `on_initialized`. If the stall config grows a new field or the detector API changes, both blocks must be updated in lockstep.

**Suggested Change**

Extract a helper function:

```rust
async fn spawn_stall_detector_for_session(state: &AppState, session_id: &str) {
    if !state.config.stall.enabled {
        return;
    }
    if let (Some(ref detectors), Some(ref tx)) =
        (&state.stall_detectors, &state.stall_event_tx)
    {
        let cancel = CancellationToken::new();
        let detector = StallDetector::new(
            session_id.to_owned(),
            Duration::from_secs(state.config.stall.inactivity_threshold_seconds),
            Duration::from_secs(state.config.stall.escalation_threshold_seconds),
            state.config.stall.max_retries,
            tx.clone(),
            cancel,
        );
        let handle = detector.spawn();
        detectors.lock().await.insert(session_id.to_owned(), handle);
        info!(session_id, "stall detector spawned");
    }
}
```

Both cases become `spawn_stall_detector_for_session(&state, &session.id).await;`

---

### File: src/audit/mod.rs

#### Comment 9 — RI-09 (Lines 108–111) — Informational

* Category: Code Quality / Completeness
* Severity: Low (implementation note)

When implementing RI-06, use the existing `with_request_id` builder method on `AuditEntry` (line 108) rather than a nonexistent `with_detail` method. No API change needed.

---

## Review Summary by Category

| Category | Count | Items |
|----------|-------|-------|
| Code Quality / Error Handling | 1 | RI-01 |
| Reliability | 1 | RI-02 |
| Performance | 1 | RI-03 |
| Correctness / Multi-session | 1 | RI-04 (deferred to 005) |
| Data Integrity | 1 | RI-05 |
| Observability / Security | 1 | RI-06 |
| Performance / Reliability | 1 | RI-07 |
| Maintainability | 1 | RI-08 |
| Completeness (informational) | 1 | RI-09 |

## Severity Breakdown

* **Medium**: 5 (RI-02, RI-04, RI-05, RI-06, RI-07)
* **Low**: 4 (RI-01, RI-03, RI-08, RI-09)

## Instruction Compliance

* ✅ `.github/instructions/constitution.instructions.md`: All principles followed — `#![forbid(unsafe_code)]`, explicit error handling, structured tracing, path security, test-first discipline, single-binary
* ✅ `.github/copilot-instructions.md`: Quality gates pass, code style conventions followed, async patterns mostly correct (RI-07 is the exception), documentation present on all public items
* ⚠️ Constitution V (Structured Observability): RI-06 identifies a gap in audit coverage for the terminal command gate path
* ⚠️ Copilot instructions (Async/tokio): RI-07 identifies sync I/O on the async executor without `spawn_blocking`

## Deferred Items

* **RI-04** → Feature 005: Multi-session channel routing for steering messages (added to `.context/backlog.md`)

## Overall Assessment

**Recommendation: Merge with follow-up.** The PR is well-structured, all quality gates pass, and the 239-test suite provides strong coverage. None of the 9 review items are blocking — they are improvements that can be addressed in a follow-up commit or patch. The feature delivers substantial value (13 implementation phases) with clean architecture.

## Outstanding Risks

* The JSONC comment destruction (RI-05) could surprise operators on first auto-approve click — consider prioritizing the `jsonc-parser` fix before the feature ships to production users.
* The audit gap (RI-06) means terminal command approvals won't appear in audit logs until fixed — relevant if audit completeness is a compliance requirement.
