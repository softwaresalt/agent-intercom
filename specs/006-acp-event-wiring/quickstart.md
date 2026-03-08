# Quickstart: ACP Event Handler Wiring

**Feature**: 006-acp-event-wiring
**Date**: 2026-03-07

## What This Feature Does

Wires the ACP event consumer's `ClearanceRequested` and `PromptForwarded` handlers so that ACP agents can request operator approval for file operations and forward continuation prompts via Slack. Previously these handlers only logged events — now they persist records, register with the ACP driver, and post interactive Slack messages.

## Testing the Feature

### Prerequisites

1. Server running in ACP mode with Slack configured
2. A workspace mapping configured for a Slack channel
3. An ACP agent session started in the configured channel

### Test 1: Clearance Request Flow

1. Start an ACP session via `/arc session-start`
2. In the ACP agent, trigger a file operation that requires approval (e.g., file deletion or modification in a non-auto-approved path)
3. **Observe**: An approval message appears in the session's Slack thread with:
   - File path and risk level indicator (🟢/🟡/🔴)
   - Diff content (inline for small diffs, file upload for large)
   - Accept and Reject buttons
4. Click **Accept**
5. **Observe**: The agent receives the approval and proceeds with the file operation
6. **Verify**: Check the database — `approval_request` table has a record with `status = 'approved'`

### Test 2: Prompt Forwarding Flow

1. Start an ACP session via `/arc session-start`
2. In the ACP agent, trigger a continuation prompt (e.g., agent asks for clarification)
3. **Observe**: A prompt message appears in the session's Slack thread with:
   - Prompt type icon and label
   - Prompt text
   - Continue, Refine, and Stop buttons
4. Click **Continue** (or Refine with instructions, or Stop)
5. **Observe**: The agent receives the decision and acts accordingly
6. **Verify**: Check the database — `continuation_prompt` table has a record with the operator's decision

### Test 3: Thread Continuity

1. Start an ACP session (no prior messages in the channel thread)
2. Trigger a clearance request
3. **Observe**: The approval message creates a new Slack thread
4. Trigger a second clearance request or prompt
5. **Observe**: The second message appears as a reply in the same thread

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Event consumer with wired handlers |
| `src/slack/blocks.rs` | Shared block builders (extracted from MCP tools) |
| `src/mcp/tools/ask_approval.rs` | MCP tool (now imports shared builders) |
| `src/mcp/tools/forward_prompt.rs` | MCP tool (now imports shared builders) |
| `src/driver/acp_driver.rs` | ACP driver registration and resolution |
| `src/persistence/approval_repo.rs` | Approval request persistence |
| `src/persistence/prompt_repo.rs` | Prompt persistence |
