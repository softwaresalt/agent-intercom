# Backlog

Features are sized for 1–3 day build cycles. Each feature should be independently spec-able, buildable, testable, and releasable. Ordered by priority (highest first).

---

- I think we need to address long running server reliability for both modes.

## 006 — ACP Event Handler Wiring

**Priority:** Critical — ACP clearance and prompt forwarding are non-functional
**Size:** Small (1 day)

Wire the ACP event consumer's `ClearanceRequested` and `PromptForwarded` handlers to actually register with `AcpDriver`, persist to the DB, and post Slack interactive messages. Without this, ACP agents requesting approval or forwarding prompts hang indefinitely.

- **F-01**: `src/main.rs:756–763` — `ClearanceRequested` handler is a no-op. Must register clearance with `AcpDriver`, persist `approval_request` via `ApprovalRepo`, post Slack interactive approval message to session thread.
- **F-02**: `src/main.rs:775–782` — `PromptForwarded` handler is a no-op. Must register with `acp_driver.register_prompt_request()` and surface to Slack. Mirror MCP `forward_prompt` tool behavior.

---

## 007 — ACP Correctness Fixes + Mobile Input Accessibility

**Priority:** High — data integrity, protocol compliance, and core mobile operator workflow
**Size:** Medium (2–3 days)

Batch of targeted correctness fixes found during adversarial review, plus a mobile accessibility track: Slack block-kit modals (used for operator input prompts such as "Refine") do not render or function correctly in Slack for iOS, blocking the primary remote management scenario. Both ACP and MCP modes are affected because the server currently depends on modal dialogs as the sole mechanism for operator text input in response to agent prompts.

**ACP Correctness Fixes**

- ~~**F-06**: `src/acp/reader.rs:346–355` — Queued steering messages marked consumed even when `send_prompt` fails. Only mark consumed after successful send; keep failed deliveries for retry.~~ ✅ Complete — 007-acp-correctness-mobile
- ~~**F-07**: `src/slack/commands.rs:405–412` — Max concurrent ACP sessions race condition. `count_active()` excludes `created`-state sessions and counts all protocols against `acp.max_sessions`. Fix: count `created` sessions, add `count_active_by_protocol`.~~ ✅ Complete — 007-acp-correctness-mobile
- **F-08**: `src/slack/commands.rs:415–425` — ACP session start resolves workspace from static `state.config` instead of hot-reloaded `state.workspace_mappings`. Violates FR-014.
- **F-09**: `src/driver/acp_driver.rs:130–134` — `deregister_session` doesn't clean up `pending_clearances` or `pending_prompts_acp`. Orphaned entries accumulate as memory leaks once F-01/F-02 are fixed.
- ~~**F-10**: `src/mcp/sse.rs:421–446` — No deprecation warning when both `workspace_id` and `channel_id` query params are provided (FR-013 violation).~~ ✅ Complete — 007-acp-correctness-mobile (`channel_id` removed entirely; `workspace_id` is sole routing mechanism)
- ~~**F-13**: `src/acp/handshake.rs:40–47` — Static handshake correlation ID `"intercom-prompt-1"` collides with `AcpDriver::PROMPT_COUNTER` starting at 1. Start counter at 1000 or use UUIDs.~~ ✅ Complete — 007-acp-correctness-mobile

**Mobile Input Accessibility**

- **F-15** *(Research)*: Investigate Slack modal / `actions` block behavior on iOS. Determine whether `modal` view pushes (triggered via `block_actions` button callbacks) are supported in the Slack iOS app, and whether `plain_text_input` elements inside modals render and accept input. Document findings: (a) modals fully work, (b) modals open but input is broken, or (c) modals are silently swallowed on mobile. Consult Slack API changelog, Block Kit documentation, and community reports.
- **F-16** *(Conditional — if modals are unavailable or broken on iOS)*: Design and implement a thread-reply-based input fallback. When the server sends a prompt requiring operator text input (MCP `transmit`/`standby`, ACP `PromptForwarded`/`ClearanceRequested`), post a Slack message in the session thread that instructs the operator to reply in-thread with their response. Detect the reply via the `message` event handler (scoped to the correct thread `ts`), capture the text, and route it back to the waiting tool call. The modal pathway remains the default for desktop; the reply pathway activates when the client surface is detected as mobile or when the modal callback times out without a submission.
- **F-17** *(Conditional — if modals are unavailable or broken on iOS)*: Audit all existing block-kit interactive components (approve/reject buttons, "Refine" prompt buttons, steer inputs) and add a plain-text thread-reply equivalent for each so that every operator interaction that currently requires a modal is reachable from the Slack mobile app.

**Post-Review Technical Debt (Phase 8 — in progress)**

- **TQ-008**: Extract duplicated fallback logic (`spawn_thread_reply_fallback` helper) — `T057` in `specs/007-acp-correctness-mobile/tasks.md`
- **TQ-009**: Push_event integration tests for negative paths — `T058`
- **LC-05**: `StreamActivity` emitted for failed deliveries in `deliver_queued_messages` — `T059`
- **LC-04**: Silent overwrite on duplicate `register_thread_reply_fallback` call — `T060`
- **CS-06**: Hardcoded SQL status strings in `count_active_acp` — `T061`

---

## 008 — Slack UI Automated Testing

**Priority:** High — enables reliable validation of new and improved Slack UI functionality
**Size:** Medium (2–3 days)

Add Playwright or equivalent framework for automated testing of Slack channel interactions. Covers session management commands, approval workflows, and message formatting. Simulates operator interactions and verifies correct message posting and command behavior across scenarios (multiple sessions, different agent states).

---

## 009 — Documentation Update

**Priority:** Medium-High — foundational onboarding before new feature surface area grows further
**Size:** Small (1–2 days)

Update README, Setup Guide, and all user-facing documentation to accurately reflect the current state of the project, with emphasis on ACP features introduced in 006/007 and configuration options added since initial release. Stale docs compound the onboarding problem with every new feature that lands.

- Update README to cover ACP mode, dual-binary layout (`agent-intercom` + `agent-intercom-ctl`), and the new `config.toml` options.
- Revise Setup Guide to include ACP-specific Slack app configuration (Socket Mode, required scopes, slash commands).
- Update User Guide to document the full set of slash commands, session lifecycle states, approval workflows, and operator interaction patterns for both MCP and ACP modes.
- Review and update `docs/configuration.md` for all new config keys.
- Flag any sections that describe in-flight behavior (mobile input fallback, automated testing) as "coming soon" rather than omitting them.

---

## 010 — Session Command UX (Fuzzy ID + Picker)

**Priority:** High — operator usability
**Size:** Small (1–2 days)

Improve session management commands (`session-stop`, `session-restart`, `session-pause`, `session-resume`, `session-checkpoint`) to accept short/partial session IDs and present an interactive picker when no ID is provided.

- Accept a short session ID prefix (e.g., first 8 chars) and fuzzy-match against active sessions in the current channel.
- When no ID is provided, query the DB for eligible sessions (filtered by command context — e.g., `session-pause` shows only active sessions, `session-resume` shows only paused sessions).
- Present a numbered list or Slack dropdown for selection.
- Display each option as `short_id — label/title`.

---

## 011 — MCP/ACP Session Linking Fixes

**Priority:** Medium — correctness for restart and MCP session visibility
**Size:** Small (1 day)

Fix two session linking issues that break operator expectations.

- **F-11**: `src/slack/commands.rs:810–868` — `session-restart` doesn't set `restart_of` field. New session starts a fresh Slack thread instead of continuing in the old one.
- **F-12**: `src/slack/commands.rs:655–689` — `handle_mcp_session_start` doesn't set `channel_id`. MCP sessions started via `/acom session-start` are invisible to `find_active_by_channel`.

---

## 012 — Workspace Query Command

**Priority:** Medium — debugging and operator awareness
**Size:** Tiny (< 1 day)

Add `/intercom get-workspace` command that returns the workspace associated with the active channel. Queries `channel_id → workspace_id` mapping from the DB/config. Useful for debugging, confirming context before session-start, and future commands that need workspace association.

---

## 013 — Slack Message Detail Level

**Priority:** Medium — avoids Slack API errors on large messages
**Size:** Small (1 day)

Add configurable detail level for Slack messages (T011). Simple enum (`None`, `Low`, `Medium`, `High`) settable via config or slash command. `AgentDriver` implementations check this before sending events and truncate/summarize accordingly. Prevents hitting Slack's message size limits with large diffs or verbose agent reasoning.

---

## 014 — File and Image Attachments (ACP)

**Priority:** Medium — enables rich HITL workflows
**Size:** Medium (2–3 days)

Enable operators to attach files, screenshots, or long-form requirements to ACP sessions via Slack. Enables the "upload a spec then start a session" workflow.

- Implement file upload detection in Slack event handlers — tag uploads with session ID.
- Add `attach_file` tool call for agents to upload files to Slack.
- Route operator file uploads to the correct ACP session as context.
- Handle file metadata linking to session and workspace.

---

## 015 — Workspace File References (#path)

**Priority:** Medium — operator convenience
**Size:** Small (1–2 days)

Parse `#path/filename` references in Slack messages (similar to GitHub Copilot Chat). Resolve to actual file paths in the workspace, read contents, and inject into agent context. Requires:

- Slack message parsing for `#path` patterns.
- Path resolution against workspace root (with security validation via `path_safety.rs`).
- Content injection into ACP session prompt or MCP tool context.

---

## 016 — Auto-Approve Subcommand Merging

**Priority:** Low — quality of life
**Size:** Tiny (< 1 day)

When an operator approves a terminal command in Slack (e.g., `cargo test`), and `cargo` already exists in the auto-approve list with a regex pattern, append `test` to the existing regex subcommand list instead of creating a new node. Reduces auto-approve config bloat.

---

## 017 — ARC Slash Commands

**Priority:** Low — operator awareness and convenience
**Size:** Small (2 days)

Add `/arc` subcommands for workspace discovery and workflow triggers. Discovery commands query the project workspace for available agent capabilities; workflow commands inject prompts into the active ACP session.

**Discovery:**
- `/arc agents` — List available `.github/agents/*.agent.md` files.
- `/arc skills` — List available `.github/skills/*/SKILL.md` files.
- `/arc instructions` — List available `.github/instructions/*.instructions.md` files.

**Workflows:**
- `/arc research <topic>` — Start a research session.
- `/arc review [session_id]` — Trigger a code review.
- `/arc tasks` — List tasks from the current spec.
- `/arc plan` — Generate an implementation plan.

Design the dispatch layer so that 018 (GHCP CLI Command Bridge) can reuse the same workflow routing for its `/research`, `/review`, `/tasks`, `/plan` commands via the ACP bridge.

---

## 018 — GHCP CLI Command Bridge

**Priority:** Low — ACP completeness
**Size:** Small (1–2 days)

Expose a subset of GitHub Copilot CLI slash commands through the ACP bridge server. Workflow commands (`/review`, `/research`, `/plan`, `/tasks`, `/agents`, `/skills`, `/instructions`) should reuse the dispatch layer built in 017 (ARC Slash Commands).

**Session management:**
- `/clear` — Clear agent context.
- `/compact` — Compact conversation history.
- `/context` — Show current context.
- `/quit` — Terminate the agent session.
- `/init` — Initialize a new workspace.
- `/list-dirs` — List workspace directories.
- `/models` — List available models and switch model in use from selection.
- `/agents` — List available agents and switch agent in use from selection.
- `/diff` — Show a diff of the last agent action.
- `/logs` — Show recent agent logs.
- `/status` — Show current session status and stats.
- `/allow-all` — Enable all permissions (tools, paths, and URLs).

**Workflows (shared dispatch with 015):**
- `/review`, `/research`, `/plan`, `/tasks`, `/instructions`, `/skills`

---

## 019 — Service Installation

**Priority:** Low — deployment convenience
**Size:** Medium (2–3 days)

Add `agent-intercom service install/uninstall` commands (similar to VS Code's `code tunnel service install`). Enables running as a background service with auto-start on boot.

- Support separate service instances for ACP and MCP modes.
- Handle config file paths, environment variables, and startup parameters.
- Work on Windows (Windows Service) and macOS/Linux (launchd/systemd).
- Must remain optional — running from a terminal window stays fully supported.
- Graceful handling for users without admin privileges (user-level service where possible).

---

## 020 — Pre-Tool Terminal Filter Hook

**Priority:** Low — architectural exploration
**Size:** Small (1 day)

Evaluate whether a hook mechanism (e.g., `.github/hooks/pre-tool-terminal-filter.ps1`) would be more deterministic for enforcing terminal command auto-approval rules than the current `resolve_clearance` approach. The hook would intercept commands before execution, check against auto-approve patterns, and enforce policy without relying on the agent to call agent-intercom. More deterministic because the agent must observe the hook, whereas it may forget to check auto-approval.

---

## Observability Debt

Non-blocking items to address opportunistically:

- **F-14**: `src/acp/writer.rs:67–70` — Writer task exits silently on write error without emitting `SessionTerminated`. Reader will eventually detect EOF, but there's a window where queued messages are silently dropped.

## Unassigned
- ~~Readme, Setup Guide, and other documentation needs to be updated to reflect the current state of the project, especially around the new ACP features and configuration options.~~ → Promoted to feature **009**.
- Consider making the Slack channel mechanism an abstraction layer that could support other platforms (e.g., Microsoft Teams, Discord, Telegram, Whatsapp) in the future. Not a priority now but could be designed with extensibility in mind.
- ~~Need to address problem that Slack block messages with Refine, for example, don't appear to work in the mobile app.~~ → Promoted to **F-15 / F-16 / F-17** in feature 007.
