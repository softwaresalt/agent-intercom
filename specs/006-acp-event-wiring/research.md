# Research: ACP Event Handler Wiring

**Feature**: 006-acp-event-wiring
**Date**: 2026-03-07

## Research Questions

### RQ-1: How to share Slack block-building logic between MCP tools and ACP handlers?

**Decision**: Extract `build_approval_blocks()` and `build_prompt_blocks()` (plus helpers) from MCP tool files to `src/slack/blocks.rs` with `pub(crate)` visibility.

**Rationale**: The functions are currently private in `ask_approval.rs` and `forward_prompt.rs`. Moving them to the existing shared Block Kit module (`slack/blocks.rs`) keeps a single source of truth. Both MCP tools and ACP event handlers import from there.

**Alternatives considered**:
- *Inline duplication*: Copy logic into `main.rs`. Rejected: violates DRY, risks visual divergence between MCP and ACP messages.
- *New shared module*: Create `src/shared/block_builders.rs`. Rejected: unnecessary new module when `slack/blocks.rs` already serves this purpose.

### RQ-2: Should ACP clearance handlers use oneshot channels like MCP tools?

**Decision**: No. ACP handlers register with `AcpDriver::register_clearance()` only, not with `state.pending_approvals`.

**Rationale**: MCP tools block on a oneshot channel because they need to return a response to the calling agent within the same tool call. ACP agents don't block — they continue streaming and receive the clearance response as a separate NDJSON message on their stream. The Slack button handler already dispatches via the polymorphic `state.driver.resolve_clearance()`, which calls `AcpDriver::resolve_clearance()` for ACP sessions (sends response to stream writer) or resolves the MCP oneshot for MCP sessions.

**Alternatives considered**:
- *Dual registration*: Register with both AcpDriver AND pending_approvals. Rejected: the MCP oneshot would never be consumed (no one is waiting on it), creating a resource leak.

### RQ-3: How to determine workspace root for file hash computation in the event handler?

**Decision**: Use the session's workspace root from the database record. Query `SessionRepo::find_by_id()` to get the session, then use `session.workspace_id` to resolve the workspace root from configuration.

**Rationale**: The MCP tool handler resolves the workspace root from the session context (which is set during MCP connection). The ACP event handler must follow the same pattern using the session record. The workspace root is needed to validate the file path (via `path_safety.rs`) and compute the SHA-256 hash.

**Alternatives considered**:
- *Skip hash computation*: Set `original_hash` to empty string. Rejected: breaks conflict detection during diff application (FR-013).
- *Use config workspace root directly*: Rejected: multi-workspace setups need per-session workspace resolution.

### RQ-4: How should `risk_level` string from the event be parsed?

**Decision**: Parse `risk_level` string to `RiskLevel` enum using existing `parse_risk_level()` helper, defaulting to `RiskLevel::Low` for unrecognized values.

**Rationale**: The `ClearanceRequested` event carries `risk_level: String` (e.g., "low", "high", "critical"). The `ApprovalRequest::new()` constructor takes `RiskLevel` enum. A case-insensitive parse with fallback to `Low` is safe — if the agent sends an unexpected value, treating it as low-risk is conservative (operators can still reject).

### RQ-5: How should `prompt_type` string from the event be parsed?

**Decision**: Parse `prompt_type` string to `PromptType` enum, defaulting to `PromptType::Continuation` for unrecognized values (FR-012).

**Rationale**: The `PromptForwarded` event carries `prompt_type: String`. The `ContinuationPrompt::new()` constructor takes `PromptType` enum. Continuation is the safest default — it presents the operator with Continue/Refine/Stop options regardless of the unknown type.

### RQ-6: Do Slack handlers need changes for ACP protocol support?

**Decision**: No changes needed. Slack handlers already dispatch via `state.driver` (polymorphic `Arc<dyn AgentDriver>`).

**Rationale**: The approval handler (`src/slack/handlers/approval.rs`) calls `state.driver.resolve_clearance()`. The prompt handler (`src/slack/handlers/prompt.rs`) calls `state.driver.resolve_prompt()`. These are trait methods that dispatch to `AcpDriver` for ACP sessions (sending NDJSON responses to the agent stream) or to the MCP oneshot maps for MCP sessions. No protocol-specific branching is needed.

## Functions to Extract

### From `src/mcp/tools/ask_approval.rs` → `src/slack/blocks.rs`

| Function | Lines | Purpose |
|----------|-------|---------|
| `build_approval_blocks()` | 438-473 | Constructs Slack Block Kit sections for approval messages |
| `INLINE_DIFF_THRESHOLD` | ~443 | Constant (20 lines) controlling inline vs file upload |

### From `src/mcp/tools/forward_prompt.rs` → `src/slack/blocks.rs`

| Function | Lines | Purpose |
|----------|-------|---------|
| `build_prompt_blocks()` | 261-294 | Constructs Slack Block Kit sections for prompt messages |
| `prompt_type_label()` | ~296 | Maps PromptType to human-readable label |
| `prompt_type_icon()` | ~308 | Maps PromptType to emoji icon |
| `truncate_text()` | ~320 | Truncates text to max chars with ellipsis |
