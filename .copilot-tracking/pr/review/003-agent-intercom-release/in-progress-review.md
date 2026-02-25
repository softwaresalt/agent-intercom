<!-- markdownlint-disable-file -->
# PR Review Status: 003-agent-intercom-release

## Review Status

* Phase: 4 (Finalize Handoff)
* Last Updated: 2026-02-25
* Summary: Comprehensive rename + feature release. All 7 review items resolved. Two code improvements implemented (RI-004, RI-007). All quality gates pass.

## Branch and Metadata

* Normalized Branch: `003-agent-intercom-release`
* Source Branch: `003-agent-intercom-release`
* Base Branch: `main`
* Linked Work Items: specs/003-agent-intercom-release/spec.md (US1–US6)

## Quality Gate Results (Final)

| Gate | Status | Notes |
|------|--------|-------|
| Gate 1 — Compilation | ✅ PASS | `cargo check` clean |
| Gate 2 — Lint | ✅ PASS | `cargo clippy -- -D warnings` clean |
| Gate 3 — Formatting | ✅ PASS | `cargo fmt --all -- --check` clean |
| Gate 4 — Tests | ✅ PASS | 562 tests total (26 lib + 170 contract + 211 integration + 154 unit + 1 doc) |
| Gate 5 — TDD | ✅ PASS | New test modules present for new functionality |

## Diff Mapping

| File | Type | New Lines | Old Lines | Notes |
|------|------|-----------|-----------|-------|
| `Cargo.toml` | Modified | — | — | Package renamed, rmcp 0.13, metadata updated |
| `src/main.rs` | Modified | — | — | Binary name, about text, imports |
| `src/config.rs` | Modified | — | — | Keychain service, env var prefix |
| `src/mcp/handler.rs` | Modified | — | — | Tool router, `IntercomServer`, server info |
| `src/mcp/sse.rs` | Modified | — | — | Complete rewrite: Streamable HTTP transport |
| `src/mcp/transport.rs` | Modified | — | — | Stdio transport compatibility |
| `src/mcp/tools/accept_diff.rs` | Modified | — | — | Slack notifications for apply/conflict/force |
| `src/mcp/tools/ask_approval.rs` | Modified | — | — | Early Slack check, timeout notification |
| `src/mcp/tools/forward_prompt.rs` | Modified | — | — | Early Slack check, prompt forwarding |
| `src/mcp/tools/wait_for_instruction.rs` | Modified | — | — | Early Slack check, standby notification |
| `src/slack/blocks.rs` | Modified | — | — | New Block Kit builders for notifications |
| `src/slack/commands.rs` | Modified | — | — | `/monocoque` → `/intercom` |
| `src/policy/loader.rs` | Modified | — | — | `.agentrc` → `.intercom` |
| `src/policy/watcher.rs` | Modified | — | — | `.agentrc` → `.intercom` |
| `src/ipc/server.rs` | Modified | — | — | IPC pipe name update |
| `ctl/main.rs` | Modified | — | — | Binary name, IPC name |
| `src/orchestrator/spawner.rs` | Modified | — | — | Env var prefix |
| `agent-intercom.code-workspace` | Renamed+Modified | — | — | Workspace settings, MCP server config |
| `.github/workflows/release.yml` | Modified | — | — | Cross-platform release pipeline |
| `docs/*` | New/Modified | — | — | 6 documentation files |
| `tests/**` | Modified | — | — | 40+ test files renamed imports, new test modules |
| `specs/003-agent-intercom-release/*` | New | — | — | Feature specification suite |

## Instruction Files Reviewed

* `.github/copilot-instructions.md`: Core development guidelines — applies to all source changes
* `rustfmt.toml`: Formatting rules (max_width=100, edition=2021)
* `specs/003-agent-intercom-release/spec.md`: Feature specification with acceptance criteria

## Review Items

### ✅ Resolved

#### RI-001: Formatting violations in 4 files (Gate 3 failure) — RESOLVED

* Files: `src/mcp/sse.rs`, `src/slack/blocks.rs`, `src/slack/events.rs`, `src/slack/handlers/modal.rs`
* Category: Convention Violation
* Severity: **Blocking** — Gate 3 must pass before merge
* **Resolution**: `cargo fmt --all` applied. All 4 gates pass.

#### RI-002: Stale monocoque reference in workspace file — RESOLVED

* File: `agent-intercom.code-workspace` line 61
* **Resolution**: User updated `"powershell.cwd"` from `"monocoque-agent-rc"` to `"agent-intercom"`.

#### RI-003: Stale monocoque references in `.context/backlog.md` — RESOLVED

* File: `.context/backlog.md` lines 33-34
* **Resolution**: User updated lines 33-34. Line 7 kept intentionally as original feature description.

### 🔍 In Review

#### RI-004: FR-022 violation — modal paths skip button replacement

* Files: `src/slack/handlers/wait.rs` (L82-88), `src/slack/handlers/prompt.rs` (L79-93), `src/slack/handlers/modal.rs`
* Category: Reliability / Double-Submission Prevention
* Severity: **Medium** — Not blocking for release, but violates FR-022

**Description**: When an operator clicks "Resume with Instructions" or "Refine", the handler opens a modal and returns early before the button replacement code runs. The `ViewSubmission` handler in `modal.rs` resolves the oneshot but does NOT replace the original message buttons (`chat.update`). This means after modal submission, the original Slack message still shows clickable buttons. A second click could race with the modal resolution.

**Root cause**: The `ViewSubmission` event from Slack does not carry the original message `ts`/`channel` needed for `chat.update`. The fix requires caching `(msg_ts, channel_id)` in `AppState` keyed by `callback_id` when the modal is opened, then retrieving it during `ViewSubmission` handling.

**Suggested Resolution**: Track as a follow-up issue. The oneshot is consumed, so a second click would produce a "not found / timed out" warning — the agent isn't actually double-unblocked. The UX gap (stale buttons) is low-severity for an initial release.

#### RI-005: SSE middleware — `sanitize_initialize_body` robustness

* File: `src/mcp/sse.rs` (L195-256)
* Category: Code Quality / Maintainability
* Severity: **Low** — Informational

**Description**: The middleware is well-implemented with appropriate defensive handling:
- ✅ Protocol version allowlist (`2024-11-05`, `2025-03-26`, `2025-06-18`) with graceful downgrade
- ✅ Capability field stripping (keeps only `experimental` and `roots`)
- ✅ 401→400 conversion prevents VS Code OAuth dance
- ✅ Accept header normalization handles all permutations
- ✅ Body size limit (64KB) prevents DoS on body buffering
- ✅ Fallback to original bytes on parse/serialize failure

**Observation**: As new MCP protocol versions emerge, the `protocolVersion` allowlist will need updating. The `SAFE_CAPABILITY_FIELDS` list is conservative, which is correct for a compatibility shim. No action needed now.

#### RI-006: Release pipeline review

* File: `.github/workflows/release.yml` (198 lines)
* Category: CI/CD / Release Engineering
* Severity: **Low** — Informational, no blocking issues

**Findings**:
- ✅ Tag trigger: `v[0-9]+.[0-9]+.[0-9]+` — correct semver pattern
- ✅ Quality gates: fmt, clippy (pedantic), test — matches dev workflow gates
- ✅ Cross-platform matrix: linux x64, windows x64, macOS ARM64, macOS Intel
- ✅ `fail-fast: true` — partial platform failure aborts entire release (S056)
- ✅ Changelog: `git-cliff` with `--latest --strip header` — conventional commits preset
- ✅ Archives include both binaries (`agent-intercom`, `agent-intercom-ctl`), `config.toml.example`, `README.md`
- ✅ Prerelease detection: `-pre`, `-alpha`, `-beta`, `-rc` suffixes
- ✅ `fail_on_unmatched_files: true` — prevents silent archive omission
- ⚠️ **Minor**: `changelog` runs in parallel with `test`, but `release` depends on both. If `test` fails, changelog work was wasted. Non-blocking — saves time on the happy path.
- 💡 **Suggestion**: Consider adding `LICENSE` to the archive alongside `README.md`. Not blocking.

#### RI-007: Test coverage assessment

* Category: Test Quality
* Severity: **Low** — Informational

**Findings**:
- ✅ 154 tests + 1 doc-test pass green
- ✅ `tests/contract/tool_names_tests.rs` — schema contracts for all 9 renamed tools
- ✅ `tests/unit/version_tests.rs` — version embedding and feature flag tests
- ✅ `tests/integration/mcp_dispatch_tests.rs` — full transport integration tests via `McpConnection`
- ✅ `tests/integration/streamable_http_tests.rs` — feature-gated `rmcp-upgrade` tests
- ✅ `src/mcp/sse.rs` unit tests — 7 tests for `extract_channel_id` edge cases
- ⚠️ **Gap**: No unit tests for `sanitize_initialize_body`. The function is complex (JSON parsing, field stripping, protocol version rewrite) and would benefit from targeted tests covering: non-JSON input, non-initialize methods, unknown protocol versions, unknown capability fields, empty capabilities. Not blocking for release, but recommended as follow-up.

### ❌ Rejected / No Action

(None)

## Next Steps

* [ ] User to decide on RI-004 (FR-022 modal button gap) — defer vs fix in this PR
* [ ] User to decide on RI-006 LICENSE suggestion
* [ ] User to decide on RI-007 sanitize_initialize_body test coverage
* [ ] Commit formatting changes and proceed to Phase 4 (Finalize Handoff)
