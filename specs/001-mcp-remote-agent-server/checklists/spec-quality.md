# Specification Quality Checklist: MCP Remote Agent Server (Deep)

**Purpose**: Exhaustive requirements quality validation across all user stories, functional requirements, edge cases, and success criteria — pre-planning gate for the spec author
**Created**: 2026-02-09
**Feature**: [spec.md](../spec.md)

## Requirement Completeness

- [ ] CHK001 - Are requirements defined for the primary agent's session creation and initial handshake when connecting via stdio transport? [Completeness, Gap] — Stories 1–4 assume a connected agent but no FR describes how the first session is established for a stdio-connected agent versus a spawned SSE agent.
- [ ] CHK002 - Are requirements specified for what happens when no workspace policy file (`.monocoque/settings.json`) exists at all? [Completeness, Gap] — FR-009 and Story 6 describe policy behavior but do not define the default when the file is absent.
- [ ] CHK003 - Is a requirement defined for the initial operational mode at server startup? [Completeness, Gap] — FR-017 defines mode switching at runtime but does not specify whether the startup default (`remote`) is configurable or hardcoded.
- [ ] CHK004 - Are requirements defined for the `help` command's output when no custom registry commands are configured? [Completeness, Spec §FR-019] — The spec defines the help command but not its behavior when the custom commands section is empty.
- [ ] CHK005 - Are requirements specified for agent disconnection handling (stdio pipe closes, SSE connection drops)? [Completeness, Gap] — No FR addresses what happens to the session and stall timer when the MCP transport disconnects unexpectedly.
- [ ] CHK006 - Are requirements specified for the server's behavior when the SurrealDB embedded database file is corrupted or unreadable on startup? [Completeness, Gap] — FR-007 requires persistence but does not define recovery when the database itself is damaged.
- [ ] CHK007 - Are requirements defined for maximum diff size limits passed to `ask_approval`? [Completeness, Gap] — Slack has message/file size limits (~40 KB snippets); no requirement addresses what happens when a diff exceeds platform limits.
- [ ] CHK008 - Are requirements specified for the `monocoque-ctl` local CLI binary's command surface and communication protocol? [Completeness, Gap] — FR-016 requires a local IPC channel, but no FR or story defines the exact commands `monocoque-ctl` supports, its argument parsing, or its error reporting.
- [ ] CHK009 - Are multi-file diff requirements fully specified for `ask_approval` and `accept_diff`? [Completeness, Spec §FR-003/FR-005] — The `file_path` parameter description mentions "additional paths are extracted from unified diff headers" but no acceptance scenario covers multi-file proposals.
- [ ] CHK010 - Are requirements defined for concurrent `ask_approval` calls from the same session? [Completeness, Gap] — The data model states "only one pending approval per session" but no FR or acceptance scenario explicitly validates this constraint or defines the error response.
- [ ] CHK011 - Are requirements specified for the `wait_for_instruction` tool's interaction with stall detection? [Completeness, Gap] — If an agent calls `wait_for_instruction`, should the stall timer be paused (the agent is intentionally idle) or should it still fire?
- [ ] CHK012 - Are requirements defined for log message ordering guarantees when multiple `remote_log` calls are made in rapid succession? [Completeness, Gap] — FR-015 and FR-020 address rate limiting but not ordering.
- [ ] CHK013 - Are requirements specified for how the server handles Slack channel archival or deletion while running? [Completeness, Gap] — FR-002 requires a persistent Slack connection but no edge case or FR addresses channel unavailability.

## Requirement Clarity

- [ ] CHK014 - Is "small diff" quantified consistently across the spec? [Clarity, Spec §FR-003] — Story 1 uses "fewer than 20 lines" as the threshold; verify this is the only definition and it matches the contracts and plan.
- [ ] CHK015 - Is "configured timeout period" in Story 1 AS-5 traceable to a specific configuration key? [Clarity, Spec §US-1] — The timeout is mentioned generically; the detailed spec sections define `approval_timeout_seconds` but the story does not cross-reference it.
- [ ] CHK016 - Is "within 5 seconds" in Story 1 AS-3 and Story 5 AS-2 a requirement on server processing time or end-to-end latency including Slack network round-trip? [Clarity, Spec §US-1/US-5] — The measurement boundary is ambiguous.
- [ ] CHK017 - Is the `risk_level` enum fully defined with clear criteria for when each level applies? [Clarity, Spec §FR-003] — The values `low`, `high`, `critical` are listed but no requirement defines what criteria determine each level — is it user-specified per call or derived from the operation type?
- [ ] CHK018 - Are the specific visual indicators for `remote_log` severity levels defined? [Clarity, Spec §FR-015] — Story 3 says "checkmark, caution icon, error icon" but does not specify exact emoji, Block Kit element types, or color coding.
- [ ] CHK019 - Is "wait state" for `session-pause` defined precisely? [Clarity, Spec §FR-012] — Story 7 AS-2 says "no further tool calls are processed until resumed" — does this mean the server buffers calls and replays them, or rejects them with an error?
- [ ] CHK020 - Is "the session's original prompt" in the stall alert requirements clearly defined as always present? [Clarity, Spec §FR-026] — Sessions created via stdio transport may not have a user-specified prompt; the spec should define what shows when `prompt` is null.
- [ ] CHK021 - Is "standard hardware" in SC-010 quantified with specific baseline specifications? [Clarity, Spec §SC-010] — "Within 10 seconds on standard hardware" is unmeasurable without a hardware definition.

## Requirement Consistency

- [ ] CHK022 - Are the ApprovalRequest status values consistent between the Key Entities section and the data model? [Consistency, Spec §Key Entities] — Spec defines statuses as `pending, approved, rejected, expired, consumed`; the data model adds `interrupted`. Verify whether `interrupted` should be documented in the spec.
- [ ] CHK023 - Are Session status values consistent between the spec's Key Entities and the data model's state machine? [Consistency, Spec §Key Entities] — The spec lists `created, active, paused, terminated` but the data model adds `interrupted`. The spec's Key Entities section should include this status.
- [ ] CHK024 - Is the session owner binding behavior consistent between FR-013 and the edge case for "authorized user who is not the session owner"? [Consistency, Spec §FR-013] — FR-013 says interactions are "rejected"; the edge case says the event is "logged but not treated as a security violation." Verify FR-013 doesn't imply it *is* a security violation.
- [ ] CHK025 - Are timeout behaviors consistent across all blocking tools? [Consistency, Spec §FR-004/FR-008] — `ask_approval` times out to `timeout` status, `forward_prompt` auto-continues, `wait_for_instruction` has optional timeout. Are these defaults consistent with each other and with operator expectations?
- [ ] CHK026 - Is the auto-approve behavior for `forward_prompt` consistent with FR-032? [Consistency, Spec §FR-009/FR-032] — FR-032 says all tools are unconditionally exposed and return errors in inapplicable contexts. But `.monocoque/settings.json` can auto-approve `forward_prompt` except for `error_recovery` type. Verify these two constraints are compatible.
- [ ] CHK027 - Are the "first-response-wins" semantics in the Clarifications section consistent with FR-022's double-submission prevention? [Consistency, Spec §Clarifications/FR-022] — First-response-wins is mentioned for race conditions, while FR-022 uses button-replacement. Verify these are complementary, not contradictory.

## Acceptance Criteria Quality

- [ ] CHK028 - Can SC-001 ("under 30 seconds from notification arrival") be objectively measured given network variability? [Measurability, Spec §SC-001] — The measurement requires defining exactly when "notification arrives" (Slack server push? device render?) and who measures.
- [ ] CHK029 - Can SC-003 ("24-hour sessions, reconnecting within 5 minutes") be verified in a test environment? [Measurability, Spec §SC-003] — This requires a long-running test harness and network disruption simulation. Is this testable as stated?
- [ ] CHK030 - Can SC-012 ("80% of stall events") be verified without a statistically significant sample size? [Measurability, Spec §SC-012] — Achieving measurable 80% recovery requires defining the test population and what constitutes a "stall event" in a controlled test.
- [ ] CHK031 - Is Story 7 AS-5 ("warns about file divergences") testable without specifying the warning format and confirmation mechanism? [Measurability, Spec §US-7] — "Warns" and "requires explicit confirmation" are stated but the UX (Slack message with confirm button? modal?) is undefined.
- [ ] CHK032 - Are the stall detection acceptance scenarios (Story 4 AS-1 through AS-10) independently testable without requiring a real stalled agent? [Measurability, Spec §US-4] — Testing requires either mocking the stall condition or having a controllable agent; the independent test describes simulation but no acceptance scenario defines the test contract.

## Scenario Coverage

- [ ] CHK033 - Are requirements defined for the server's behavior when Slack Socket Mode authentication fails on startup? [Coverage, Exception Flow, Gap] — FR-002 requires the connection but no scenario covers invalid `app_token` or `bot_token`.
- [ ] CHK034 - Are requirements defined for the sequence: `ask_approval` → timeout → agent retries with same diff? [Coverage, Alternate Flow, Gap] — Story 1 AS-5 mentions timeout but no scenario addresses retry semantics (is a new `request_id` generated? can the same diff be resubmitted?).
- [ ] CHK035 - Are requirements defined for checkpoint restore when the referenced session is currently active (not terminated)? [Coverage, Alternate Flow, Spec §FR-012] — Story 7 AS-5 describes checkpoint restore but doesn't specify behavior when the session being restored is still running.
- [ ] CHK036 - Are requirements defined for `accept_diff` when the target file does not exist but the diff is a unified patch (not a full-file write)? [Coverage, Exception Flow, Gap] — FR-005 mentions both modes but does not address applying a unified diff to a non-existent file.
- [ ] CHK037 - Are requirements defined for the `session-start` command when the configured `host_cli` binary is not found on PATH? [Coverage, Exception Flow, Gap] — Story 7 assumes the CLI is available (Assumption §7) but no FR or edge case specifies the error behavior when it's missing.
- [ ] CHK038 - Are requirements defined for what happens when a spawned agent session (SSE transport) fails to connect back to the server? [Coverage, Exception Flow, Gap] — Session-start spawns a process but no requirement defines the timeout or error handling for a failed connection from the spawned process.
- [ ] CHK039 - Are requirements defined for operator actions on an expired/timed-out approval request? [Coverage, Alternate Flow, Gap] — If the operator opens Slack after the timeout and sees the (now-expired) message, the spec says buttons are replaced — but this is only specified for accepted/rejected outcomes, not timeout.
- [ ] CHK040 - Are recovery flow requirements defined for when `accept_diff` partially succeeds on a multi-file proposal (some files written, some fail)? [Coverage, Recovery Flow, Gap] — FR-005 describes both modes but no requirement addresses atomicity of multi-file writes or rollback on partial failure.

## Edge Case Coverage

- [ ] CHK041 - Is behavior defined when the operator sends a Slack message to the channel that is not a slash command? [Edge Case, Gap] — FR-018 exposes channel history as a resource; the spec should clarify whether free-text messages are treated as instructions, ignored, or logged.
- [ ] CHK042 - Is behavior defined when the `workspace_root` path does not exist at server startup? [Edge Case, Gap] — Assumption §4 says it's "pre-configured" but no requirement defines validation or error behavior at startup.
- [ ] CHK043 - Is behavior defined when `accept_diff` targets a path where the parent directory is read-only or permissions prevent writing? [Edge Case, Gap] — FR-006 addresses path traversal but not filesystem permission errors.
- [ ] CHK044 - Is behavior defined for `session-checkpoint` when no files exist in the workspace? [Edge Case, Gap] — The `file_hashes` manifest would be empty, which is valid but should be documented.
- [ ] CHK045 - Is behavior defined when the Slack API returns an error for `chat.update` when dismissing a stall alert after self-recovery? [Edge Case, Gap] — FR-030 requires updating the Slack message, but the message may have been deleted or the bot may lack permissions.
- [ ] CHK046 - Is behavior defined when two concurrent sessions both trigger stall alerts simultaneously? [Edge Case, Spec §Edge Cases] — The edge case says "each session has its own independent stall timer" and "alerts are posted with the session ID prominently displayed" but does not define whether alerts are batched, interleaved, or rate-limited.
- [ ] CHK047 - Is behavior defined when the agent calls `set_operational_mode("local")` but no IPC listener is active or `monocoque-ctl` is not installed? [Edge Case, Gap] — FR-017 defines mode switching but no requirement addresses the case where the local IPC channel is unavailable after switching to local mode.

## Non-Functional Requirements

- [ ] CHK048 - Are memory consumption requirements defined beyond the stated constraint of "< 200 MB at steady state"? [NFR, Gap] — The plan mentions this constraint but no spec-level FR or SC formalizes it as a requirement with a measurement method.
- [ ] CHK049 - Are logging/observability requirements defined for the server's own operational telemetry? [NFR, Gap] — The plan uses `tracing/tracing-subscriber` but no FR specifies what the server logs locally (log levels, rotation, structured format).
- [ ] CHK050 - Are data retention requirements specified for the SurrealDB embedded database? [NFR, Gap] — The spec requires persistence (FR-007) but does not define how long historical sessions, checkpoints, and consumed approval records are retained or whether purging is needed.
- [ ] CHK051 - Are requirements specified for the server's CPU/resource usage during idle periods (no active sessions)? [NFR, Gap] — The server maintains a Slack WebSocket and a stall detection timer; no requirement bounds idle resource consumption.
- [ ] CHK052 - Are upgrade/migration requirements defined for the SurrealDB schema when the server version is updated? [NFR, Gap] — The data model defines schema via DDL on startup, but no requirement covers schema evolution across server versions.
- [ ] CHK053 - Are requirements defined for the maximum number of historical sessions, checkpoints, and approval records the server must support? [NFR, Gap] — No scalability bound is specified for the embedded database.

## Security Requirements

- [ ] CHK054 - Are requirements defined for how Slack tokens (`app_token`, `bot_token`) are stored and protected? [Security, Gap] — `config.toml` contains sensitive tokens as plaintext strings; no requirement specifies file permissions, encryption at rest, or environment variable alternatives.
- [ ] CHK055 - Are requirements defined for command injection prevention beyond `shlex` escaping in the command dispatcher? [Security, Spec §FR-014] — The edge case mentions "deny-by-default" and the plan mentions `shlex`, but no FR explicitly requires input sanitization or defines the escaping contract.
- [ ] CHK056 - Are requirements defined for rate limiting on the MCP tool surface to prevent abuse by a malicious or misconfigured agent? [Security, Gap] — An agent could call `remote_log` in an infinite loop, flooding the Slack channel. No requirement bounds tool call rates from the agent.
- [ ] CHK057 - Are requirements defined for the security boundary of the SSE/HTTP transport used by spawned sessions? [Security, Gap] — The plan exposes `127.0.0.1:3000` for SSE transport; no requirement specifies authentication, authorization, or mTLS for this endpoint.
- [ ] CHK058 - Are the security logging requirements complete for all security-relevant events? [Security, Spec §Edge Cases] — Unauthorized button interactions are logged, but are failed command allowlist lookups, path traversal attempts, and token validation failures also logged?

## Dependencies and Assumptions

- [ ] CHK059 - Is the assumption that "only one primary agent connects via stdio" validated with a requirement for what happens if two agents attempt stdio connections? [Assumption, Spec §Assumptions] — Assumption §5 states this but no FR defines the error behavior for a second stdio connection attempt.
- [ ] CHK060 - Is the dependency on the `host_cli` binary versioned or compatibility-bounded? [Dependency, Spec §Assumptions] — Assumption §7 requires the CLI to be on PATH; no requirement specifies minimum versions or feature requirements for supported CLIs (Claude, gh copilot, etc.).
- [ ] CHK061 - Is the dependency on Slack's Block Kit API versioned or documented? [Dependency, Gap] — The spec relies on specific Block Kit features (buttons, modals, snippets, `rich_text_preformatted`) but does not document the minimum Slack API version required.
- [ ] CHK062 - Is the assumption that MCP agents support server-to-client notifications validated? [Assumption, Spec §Clarifications] — The nudge mechanism (FR-027) relies on agents handling `monocoque/nudge` notifications; no requirement defines fallback behavior for agents that ignore custom notifications.

## Ambiguities and Conflicts

- [ ] CHK063 - Is the term "session state" in checkpoint requirements unambiguously defined? [Ambiguity, Spec §Key Entities] — "Serialized session state" in the Checkpoint entity includes approval requests and file hashes, but does it include the agent's in-memory context, conversation history, or just the server-side metadata? The boundary is unclear.
- [ ] CHK064 - Does "the agent resumes execution" (Story 4 AS-2) mean the agent acts on the nudge notification, or merely that the server sent the notification? [Ambiguity, Spec §US-4] — The acceptance criteria conflate sending the nudge with the agent actually resuming. The server can only guarantee delivery, not resumption.
- [ ] CHK065 - Are "spawned agent processes" in FR-021 limited to `session-start` processes, or does this include the primary stdio agent? [Ambiguity, Spec §FR-021] — The shutdown requirement mentions "spawned agent processes" but the primary agent is not "spawned" by the server — it connects externally. Clarify scope.
- [ ] CHK066 - Is "operational mode" a server-global or per-session setting? [Ambiguity, Spec §FR-017] — The `set_operational_mode` tool implies server-global, but the data model includes `mode` on the Session entity. These may conflict if one session's `set_operational_mode("local")` call affects another session's routing.
- [ ] CHK067 - Does the "first-response-wins" fallback apply only to hybrid mode (Slack + IPC race), or also within Slack when two interactions arrive for the same request? [Ambiguity, Spec §Clarifications] — The clarification mentions it for "residual race conditions" but FR-022 handles double-clicks via button replacement. The overlap is unclear.

## Notes

- Check items off as completed: `[x]`
- Add comments or findings inline after each item
- Items reference spec sections as `[Spec §...]` or use `[Gap]` for missing requirements
- Total: 67 items across 9 quality dimensions
