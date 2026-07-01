# Research: 003-agent-intercom-release

**Feature Branch**: `003-agent-intercom-release`  
**Date**: 2026-02-23  
**Spec**: [spec.md](spec.md)

## Research Areas

### R-001: Scope of "monocoque" References in Codebase

**Decision**: Full automated find-and-replace with manual verification for context-sensitive locations.

**Findings**: ~848 occurrences of "monocoque" across ~110 files. The references fall into these categories:

| Category | Count | Replacement Strategy |
|---|---|---|
| Rust crate/module identifiers (`monocoque_agent_rc`, `monocoque-agent-rc`) | ~120 | Cargo.toml package/binary rename + global `use` statement update |
| Slack slash commands (`/monocoque`) | ~25 | String literal replacement in `src/slack/commands.rs` |
| Environment variable prefixes (`MONOCOQUE_`) | ~15 | String literal replacement in `src/config.rs`, `src/orchestrator/spawner.rs` |
| Keychain service name (`monocoque-agent-rc`) | 1 | Constant replacement in `src/config.rs` |
| IPC pipe name | ~3 | Constant replacement in `src/ipc/socket.rs`, `ctl/main.rs` |
| Policy directory (`.agentrc`) | ~50 | Constant replacement in `src/policy/loader.rs`, `src/policy/watcher.rs` |
| Doc comments and log messages | ~100 | Global search-replace |
| Test fixtures and assertions | ~200 | Global search-replace |
| Documentation files (README, guides, ADRs) | ~150 | Global search-replace |
| Config files (config.toml, Cargo.toml metadata) | ~30 | Manual edit |
| Spec/agent/skill files | ~150 | Global search-replace |

**Rationale**: The rename is mechanical but scope is large. A phased approach (Cargo first, then source, then tests, then docs) reduces compounding errors.

**Alternatives considered**:
- Partial rename (public-facing only): Rejected — creates confusion between internal and external names.
- sed/regex mass replacement: Rejected — too error-prone for Rust identifiers that need snake_case vs kebab-case variants.

### R-002: rmcp 0.5 → 0.13.0 Migration Path

**Decision**: Isolate the upgrade as the final implementation phase after rename is stable.

**Findings**: The upgrade involves one critical breaking change and several medium-impact changes:

1. **CRITICAL — SSE transport removed (v0.11)**: `SseServer` and `SseServerConfig` were deleted. The replacement is `StreamableHttpService` from the `transport-streamable-http-server` feature, which uses a single `/mcp` POST endpoint with SSE response streaming instead of the old `/sse` + `/message` two-endpoint model.

2. **MEDIUM — Session management paradigm**: The new `StreamableHttpService` requires a `SessionManager` trait implementation. rmcp provides `LocalSessionManager` as default. The codebase's per-connection state model (channel_id override, session_id override) needs adaptation.

3. **MEDIUM — Feature flag changes**: `transport-sse-server` → `transport-streamable-http-server` in Cargo.toml features.

4. **LOW — Model type additions**: New optional fields (`_meta`) on `Tool`, `CallToolResult`, `ResourceContents`. Additive only — existing code should compile.

5. **LOW — ServerHandler trait expansion**: ~15 new methods with defaults. Existing impl should compile.

**Files requiring refactoring**:
- `src/mcp/sse.rs` — Complete rewrite (CRITICAL)
- `src/mcp/handler.rs` — Moderate changes to factory pattern
- `src/mcp/transport.rs` — Minimal (stdio unchanged)
- `Cargo.toml` — Feature flag update
- Integration tests using SSE — Rewrite test setup

**Rationale**: The SSE transport rewrite is the highest-risk item. Isolating it after the rename prevents compounding risk.

**Alternatives considered**:
- Upgrade before rename: Rejected — introduces two sources of breakage simultaneously.
- Skip upgrade: Rejected — rmcp 0.5 will become unsupported as the protocol evolves.
- Incremental upgrade (0.5→0.8→0.13): Rejected — the SSE removal happens at 0.11 regardless; intermediate stops add work without reducing risk.

### R-003: Slack Notification Gap Analysis

**Decision**: Add Slack messages at 5 identified gap points in the existing tool handlers.

**Findings**: The following notification gaps exist:

| Gap | Location | Current Behavior | Required Behavior |
|---|---|---|---|
| `accept_diff` success | `src/mcp/tools/accept_diff.rs` | Returns success JSON to agent only | Post Slack confirmation with file path and bytes |
| `accept_diff` conflict | `src/mcp/tools/accept_diff.rs` | Returns `patch_conflict` error to agent | Post Slack alert about conflict |
| `accept_diff` force-apply | `src/mcp/tools/accept_diff.rs` | Posts Slack warning (partially works) | Ensure warning always posts for force-apply |
| No Slack channel configured | `src/mcp/tools/ask_approval.rs` | Blocks silently with `warn!` log | Return descriptive error to agent |
| `forward_prompt` notification | `src/mcp/tools/forward_prompt.rs` | Already posts Slack message | Verify all code paths post correctly |
| `wait_for_instruction` notification | `src/mcp/tools/wait_for_instruction.rs` | Already posts Slack message | Verify all code paths post correctly |

**Rationale**: Most gaps are in `accept_diff` which currently has no Slack integration for outcomes. The fix requires access to `SlackService` in the `accept_diff` handler, which already receives `AppState`.

**Alternatives considered**:
- Event-driven notification system: Rejected for v1 — adds architectural complexity. Direct Slack calls in handlers are simpler and already the pattern used by `ask_approval`.

### R-004: GitHub Actions Release Pipeline

**Decision**: Use `cross` for cross-compilation, `cargo-dist` style archive packaging, and `git-cliff` for changelog generation.

**Findings**: Standard Rust release pipeline components:

| Component | Tool | Rationale |
|---|---|---|
| Cross-compilation | `cross` (cross-rs) | Reliable cross-compilation to Linux/macOS from GitHub Actions runners |
| Archive packaging | Custom step with `tar`/`zip` | Simple, no additional tooling needed |
| Changelog | `git-cliff` | Conventional commit parsing, highly configurable |
| Release creation | `softprops/action-gh-release` | Standard GitHub Release action |
| Version embedding | `env!("CARGO_PKG_VERSION")` | Already available from Cargo at compile time |

**Target matrix**:
- `x86_64-pc-windows-msvc` (Windows x64)
- `x86_64-unknown-linux-gnu` (Linux x64)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-apple-darwin` (macOS Intel)

**Rationale**: This is a well-established pattern for Rust projects. No custom infrastructure needed.

**Alternatives considered**:
- `cargo-dist`: Rejected — introduces heavy opinionated tooling. Custom workflow provides more control.
- `release-please`: Considered for version bumping — may be added later but not required for initial release.

### R-005: Feature Flagging Strategy

**Decision**: Use Cargo features for compile-time gating.

**Findings**: Cargo's built-in feature system (`[features]` in Cargo.toml) is the standard Rust approach:

```toml
[features]
default = []
rmcp-upgrade = []  # Gates the rmcp 0.13 transport layer
```

Code uses `#[cfg(feature = "rmcp-upgrade")]` for conditional compilation. This ensures:
- Unused code is completely excluded from release binaries
- No runtime overhead
- Feature combinations tested in CI

**Rationale**: Matches Rust ecosystem conventions. No runtime configuration needed.

**Alternatives considered**:
- Runtime feature flags via config.toml: Rejected — includes all code in binary, adds runtime branching complexity.
- Environment variable flags: Rejected — same issues plus harder to test.

### R-006: Intercom-Themed Tool Names

**Decision**: Use radio/intercom terminology as clarified by the operator.

**Findings**: The confirmed tool name mapping:

| Current Name | New Name | Theme |
|---|---|---|
| `ask_approval` | `check_clearance` | Requesting clearance for an action |
| `accept_diff` | `check_diff` | Checking/applying a cleared diff |
| `check_auto_approve` | `auto_check` | Automatic clearance check |
| `forward_prompt` | `transmit` | Transmitting a message |
| `wait_for_instruction` | `standby` | Entering standby mode |
| `heartbeat` | `ping` | Periodic liveness ping |
| `remote_log` | `broadcast` | Broadcasting status |
| `recover_state` | `reboot` | Recovery/reboot process |
| `set_operational_mode` | `switch_freq` | Switching frequency/mode |

The rename only affects the `name` field in each `Tool` struct definition and the `ToolRouter` dispatch map in `handler.rs`. Input/output schemas are unchanged. All documentation, copilot-instructions, and agent prompts must be updated to use new names.

**Rationale**: Names are concise, thematically consistent, and self-documenting. Each name conveys the tool's purpose through the intercom metaphor.

**Alternatives considered**:
- `intercom_` prefix pattern: Rejected by operator — less thematic.
- Keep existing names: Rejected by operator — branding opportunity.
