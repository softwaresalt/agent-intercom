# Addendum Requirements Quality — Deep Checklist (US11 + US12 + US13)

**Purpose**: Validate the completeness, clarity, consistency, measurability, and edge-case coverage of the requirements for User Stories 11 (Slack Environment Variable Configuration), 12 (Dynamic Slack Channel Selection), and 13 (Service Rebranding to Remote Control).

**Created**: 2026-02-14  
**Depth**: Deep  
**Audience**: Autonomous implementation agent (build-gate consumption)  
**Scope**: spec.md §US11–§US13, FRs 038–049, SCs 013–015, edge cases 17–22, plan.md Phases 15–17, tasks.md T200–T217

---

## Requirement Completeness

- [ ] CHK001 - Are all three fixed environment variable names (`SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`, `SLACK_TEAM_ID`) explicitly enumerated in both the US11 narrative and FR-038? [Completeness, Spec §US11 / §FR-038]
- [ ] CHK002 - Is the behavior for empty environment variable values (empty string `""`) specified? Acceptance scenario 5 references `SLACK_TEAM_ID` empty/unset, but FR-038 and FR-041 do not define how the system distinguishes empty from absent for `SLACK_BOT_TOKEN` / `SLACK_APP_TOKEN`. [Gap, Spec §FR-038]
- [ ] CHK003 - Is the behavior for partially-set environment variables specified (e.g., `SLACK_BOT_TOKEN` set but `SLACK_APP_TOKEN` absent)? FR-040 covers the error case, but the requirement should explicitly state per-credential resolution, not all-or-nothing. [Completeness, Spec §FR-040]
- [ ] CHK004 - Is the IPC pipe/socket default name included in the rename scope? The US13 narrative mentions it, the data-model.md references `monocoque-agent-rc`, but no FR (FR-045–FR-049) explicitly mandates the IPC pipe rename. [Gap, Spec §US13]
- [ ] CHK005 - Is the workspace file `agent-rem.code-workspace` included in the rename scope? FR-048 says "zero remaining references" but does not list workspace metadata files as a rename target. [Gap, Spec §FR-048]
- [ ] CHK006 - Are requirements defined for renaming Slack message branding text? US13 narrative lists "Slack message branding" as in-scope, but no FR specifies which Slack message strings must change. [Gap, Spec §US13]
- [ ] CHK007 - Is there a requirement specifying which files/directories are excluded from the "zero references" rule in SC-015 and FR-048? Both mention "historical changelog entries" but do not define which files qualify as historical changelogs. [Completeness, Spec §FR-048 / §SC-015]
- [ ] CHK008 - Are requirements defined for what happens when the `channel_id` query parameter value changes on SSE reconnection (same session, different channel)? The spec addresses new connections but not reconnection semantics. [Gap, Spec §US12]
- [ ] CHK009 - Is there a requirement for the `channel_id` parameter on non-SSE HTTP endpoints (e.g., health check, metrics)? FR-042 scopes to "HTTP/SSE transport endpoint" — is it clear that only the SSE endpoint accepts this parameter? [Completeness, Spec §FR-042]
- [ ] CHK010 - Are requirements defined for the companion CLI tool (`monocoque-ctl`) post-rename? FR-045 says it "remains `monocoque-ctl`" but no FR addresses whether the CLI's internal references to the server (help text, connection defaults) must also update. [Gap, Spec §FR-045]
- [ ] CHK011 - Is the `SLACK_TEAM_ID` environment variable documented as optional in the Assumptions section? Assumption #2 lists it alongside required credentials without marking it as optional, potentially conflicting with FR-041. [Completeness, Spec §Assumptions]

---

## Requirement Clarity

- [ ] CHK012 - Is "case-sensitive" in FR-038 specific enough? It states environment variable names are "fixed and case-sensitive" but does not clarify whether this refers to the variable name only or also the variable value. [Clarity, Spec §FR-038]
- [ ] CHK013 - Is "clear, actionable error message" in FR-040 defined with minimum required content fields? The phrase is subjective. The plan.md elaborates (must include keychain service name, env var name, both resolution methods), but the FR itself does not specify these fields. [Clarity, Spec §FR-040]
- [ ] CHK014 - Is "non-empty" in FR-042 quantified? Does it mean non-zero-length, non-whitespace-only, or matching a Slack channel ID format? FR-043 says "empty" uses default, but whitespace-only is not addressed. [Clarity, Spec §FR-042 / §FR-043]
- [ ] CHK015 - Is "all user-visible references" in FR-048 scoped with an enumerated list or discovery mechanism? The phrase is open-ended. Plan.md provides a rename categories table, but the FR leaves the set unbounded. [Clarity, Spec §FR-048]
- [ ] CHK016 - Is "historical changelog or migration notes" in FR-048 defined with specific file paths or naming conventions? Without a clear boundary, an agent cannot determine which files to exclude from the zero-reference verification. [Clarity, Spec §FR-048]
- [ ] CHK017 - Is "scope Socket Mode connections to the correct workspace" in FR-041 defined with a specific technical mechanism? The requirement describes the intent but not how `SLACK_TEAM_ID` is passed to the Socket Mode client or what "scoping" means in the Slack API context. [Clarity, Spec §FR-041]
- [ ] CHK018 - Is "single-workspace mode" in FR-041 defined? The term suggests behavior without a team ID constraint, but the requirement does not clarify whether implicit workspace detection occurs or if the first available workspace is used. [Clarity, Spec §FR-041]

---

## Requirement Consistency

- [ ] CHK019 - Do FR-036 and FR-038/FR-039 create redundant requirements? FR-036 already mandates keychain-first with env var fallback. FR-038/FR-039 restate this for specific variables. Is the precedence behavior defined identically in both, or could an implementer interpret them differently? [Consistency, Spec §FR-036 / §FR-038 / §FR-039]
- [ ] CHK020 - Does Assumption #2 align with FR-041 regarding `SLACK_TEAM_ID` optionality? Assumption #2 lists all three env vars as credential sources without distinguishing required from optional, while FR-041 explicitly marks `SLACK_TEAM_ID` as optional. [Consistency, Spec §Assumptions / §FR-041]
- [ ] CHK021 - Is the keychain service name consistent across FR-046, Assumption #2, data-model.md, and edge case 21? FR-046 says `monocoque-agent-rc`, Assumption #2 says `monocoque-agent-rc`, edge case 21 references the old name as `monocoque-agent-rem`. Verify no document uses the old name as the current service name. [Consistency, Spec §FR-046 / §Assumptions / Edge Case 21]
- [ ] CHK022 - Does acceptance scenario 7 of US13 conflict with edge case 21? Scenario 7 says the server does NOT migrate and the "startup error message clearly explains the required action." Edge case 21 says the server "falls back to environment variables" and only fails if those are absent. Both are consistent (error only on total credential absence) but the scenario 7 phrasing could imply an error occurs whenever old keychain entries exist. [Consistency, Spec §US13-SC7 / Edge Case 21]
- [ ] CHK023 - Is the SurrealDB namespace consistent between FR-047, US13 acceptance scenario 2, and data-model.md? FR-047 says the namespace is `monocoque` (unchanged) and the database changes to `agent_rc`. Verify data-model.md uses the same namespace. [Consistency, Spec §FR-047 / §US13-SC2]
- [ ] CHK024 - Are the plan.md Phase 15 tasks and tasks.md Phase 15 tasks in the same order with matching descriptions? (Post-remediation verification of F1/F2 fixes.) [Consistency, Plan §Phase15 / Tasks §Phase15]
- [ ] CHK025 - Are the plan.md Phase 16 tasks and tasks.md Phase 16 tasks in the same order with matching descriptions? (Post-remediation verification of F1/F2 fixes.) [Consistency, Plan §Phase16 / Tasks §Phase16]

---

## Acceptance Criteria Quality

- [ ] CHK026 - Are all US11 acceptance scenarios (1–5) testable by an autonomous agent without manual Slack workspace interaction? Scenario 1 requires verifying "connects to Slack successfully" — is the verification method specified (e.g., tracing output, health endpoint, exit code)? [Measurability, Spec §US11]
- [ ] CHK027 - Is SC-013 measurable by an autonomous agent? It requires "starts successfully with Slack credentials provided exclusively via environment variables… in under 10 seconds." Is the measurement method defined (wall clock, process exit code, specific log line)? [Measurability, Spec §SC-013]
- [ ] CHK028 - Is SC-014 measurable by an autonomous agent? "Zero cross-contamination" requires inspecting actual Slack channel messages, which may not be automatable in a test environment. Is a mock/stub verification acceptable? [Measurability, Spec §SC-014]
- [ ] CHK029 - Is SC-015 specified with the exact grep pattern, file scope, and exclusion list for the zero-reference verification? The plan.md uses `grep -r "agent.rem" src/ tests/ ctl/ Cargo.toml`, but the spec only says "automated grep across all source files, configuration files, and documentation." [Measurability, Spec §SC-015]
- [ ] CHK030 - Does US12 acceptance scenario 3 define "empty" consistently with FR-043? Scenario 3 says "empty `channel_id` parameter" is treated as absent. FR-043 says "absent, empty." Are both using the same definition of empty? [Measurability, Spec §US12-SC3 / §FR-043]
- [ ] CHK031 - Is US13 acceptance scenario 6 exhaustive in its enumeration of documents to verify? It lists "Cargo.toml, README, CLI help text, and config.toml comments" but the codebase includes additional documents (quickstart.md, spec.md, constitution.md, copilot-instructions.md). [Measurability, Spec §US13-SC6]

---

## Scenario Coverage

- [ ] CHK032 - Are requirements defined for the credential loading order when multiple fallback sources exist? FR-039 defines keychain > env var, but does not address whether the system should log which source was selected. Plan.md T203 adds tracing, but no FR mandates observability of credential source selection. [Coverage, Gap]
- [ ] CHK033 - Are requirements defined for concurrent SSE session behavior when one session's target channel becomes unavailable mid-session (e.g., channel archived or deleted)? Edge cases 19–20 cover connection-time issues but not mid-session channel loss. [Coverage, Gap]
- [ ] CHK034 - Are requirements defined for the stdio transport explicitly refusing the `channel_id` parameter? US12 acceptance scenario 5 states it always uses the default, but no FR specifies the behavior or whether an error/warning is surfaced when a stdio agent attempts to set a channel override. [Coverage, Spec §US12-SC5]
- [ ] CHK035 - Are recovery/rollback requirements defined for a partially-completed rename (US13)? If the rename fails midway (e.g., after Cargo.toml but before test files), is there a defined recovery procedure? [Coverage, Gap]
- [ ] CHK036 - Are requirements defined for backward compatibility of the `config.toml` file after the rename? If a user has an existing config.toml referencing `monocoque-agent-rem`, does the server reject it, warn, or silently accept it? [Coverage, Gap]
- [ ] CHK037 - Are requirements for DM channels (`D`-prefixed) and group channels (`G`-prefixed) as `channel_id` values specified? FR-042 uses a `C_CHANNEL_ID` example, which may imply only public channels. Edge case 20 says "invalid Slack channel ID format" is passed through, but does not clarify whether non-`C`-prefixed IDs are valid. [Coverage, Spec §FR-042 / Edge Case 20]

---

## Edge Case Coverage

- [ ] CHK038 - Is the behavior for environment variables containing leading/trailing whitespace defined? An env var like `SLACK_BOT_TOKEN=" xoxb-... "` could cause authentication failures. No FR or edge case addresses whitespace handling. [Edge Case, Gap]
- [ ] CHK039 - Is the behavior for multiple `channel_id` query parameters on a single SSE URL defined (e.g., `/sse?channel_id=C1&channel_id=C2`)? Plan.md T207 mentions "first wins" but no FR specifies this. [Edge Case, Gap]
- [ ] CHK040 - Is the behavior for environment variables set to the literal string "null", "undefined", or "none" specified? These are common placeholder values that could bypass empty-string checks. [Edge Case, Gap]
- [ ] CHK041 - Is old-to-new migration of IPC pipe/socket names covered as an edge case? A running process using the old pipe name while the new binary uses the new name could cause connection failures for the CLI tool. [Edge Case, Gap]
- [ ] CHK042 - Are edge cases defined for the rename interacting with in-flight SurrealDB data? If the server is restarted mid-rename with some files using old names and some new, is the database behavior defined? [Edge Case, Gap]
- [ ] CHK043 - Is the edge case defined for `channel_id` containing URL-encoded characters (e.g., `%23` for `#`)? Plan.md T207 mentions URL-encoded values but no FR or edge case addresses this. [Edge Case, Gap]
- [ ] CHK044 - Is the edge case of keychain entries existing under BOTH old (`monocoque-agent-rem`) and new (`monocoque-agent-rc`) service names addressed? FR-046 says the old name is not checked, but if both exist, the operator may be confused about which credentials are active. [Edge Case, Gap]

---

## Non-Functional Requirements Coverage

- [ ] CHK045 - Are performance requirements specified for credential loading (US11)? SC-013 defines a 10-second startup threshold, but is the credential loading portion of startup bounded separately? [Non-Functional, Gap]
- [ ] CHK046 - Are security requirements for environment variable credential exposure specified? FR-036 prohibits plaintext config files, but environment variables are visible via `/proc/*/environ` on Linux and `Get-Process` on Windows. Is this risk acknowledged or mitigated? [Non-Functional, Gap]
- [ ] CHK047 - Are observability requirements for the `channel_id` override documented as FRs? Plan.md T203 adds tracing for credential loading, but no FR mandates logging when a `channel_id` override is applied to a session. [Non-Functional, Gap]
- [ ] CHK048 - Are the accessibility/discoverability requirements for the `?channel_id=` parameter defined? Is it documented in MCP server metadata, help output, or Slack help command? [Non-Functional, Gap]

---

## Dependencies & Assumptions Coverage

- [ ] CHK049 - Is Assumption #10 (service branded as `monocoque-agent-rc`) traceable to FRs 045–049? Does each FR derive from this assumption, and is the assumption updated post-rename (not still referencing the old name)? [Traceability, Spec §Assumptions / §FR-045–049]
- [ ] CHK050 - Is the dependency between Phase 17 (rename) and Phases 15/16 explicitly documented in the spec, or only in plan.md? The recommended execution order (Phase 17 first) is in plan.md and tasks.md but the spec itself does not mandate rename-first sequencing. [Dependencies, Gap]
- [ ] CHK051 - Is the assumption that `slack-morphism` supports `team_id` scoping in Socket Mode documented? FR-041 requires team ID scoping, but no assumption validates that the chosen SDK (`slack-morphism 2.17`) supports this capability. [Assumption, Gap]
- [ ] CHK052 - Is the assumption that Slack channel IDs passed via `channel_id` are pre-validated by the caller documented? Edge cases 19–20 describe pass-through behavior (no server-side validation), but no explicit assumption states this design choice. [Assumption, Gap]

---

## Cross-Reference Traceability

- [ ] CHK053 - Does every FR in §US11 (FR-038–FR-041) have at least one corresponding acceptance scenario in the US11 narrative? Map each FR to its scenario(s) and identify any FR without scenario coverage. [Traceability, Spec §US11]
- [ ] CHK054 - Does every FR in §US12 (FR-042–FR-044) have at least one corresponding acceptance scenario in the US12 narrative? [Traceability, Spec §US12]
- [ ] CHK055 - Does every FR in §US13 (FR-045–FR-049) have at least one corresponding acceptance scenario in the US13 narrative? [Traceability, Spec §US13]
- [ ] CHK056 - Does every edge case (17–22) trace to at least one FR or acceptance scenario? Identify any orphan edge cases that lack FR backing. [Traceability, Spec §Edge Cases]
- [ ] CHK057 - Does every task in tasks.md (T200–T217) trace to at least one FR with a correct FR reference? (Post-remediation verification of F4 fix — T203 should reference FR-036, not FR-040.) [Traceability, Tasks §Phase15–17]
- [ ] CHK058 - Does every SC (SC-013–SC-015) have at least one FR and at least one task that support its achievement? [Traceability, Spec §SC-013–015 / Tasks §T200–T217]

---

## Ambiguities & Conflicts

- [ ] CHK059 - Is the term "credentials" used consistently? FR-036 uses "Slack tokens and other sensitive credentials," FR-038 uses "credentials," FR-040 uses "required Slack credential." Does the scope of "credentials" always refer to the same set of values? [Ambiguity, Spec §FR-036 / §FR-038 / §FR-040]
- [ ] CHK060 - Does the phrase "falls back to reading from environment variables" in FR-036 conflict with the per-credential fallback semantics of FR-039? FR-036 could be read as all-or-nothing fallback, while FR-039 specifies per-credential precedence. [Conflict, Spec §FR-036 / §FR-039]
- [ ] CHK061 - Is there an implicit requirement for migration documentation (from old name to new name) that is not captured as a FR or task? Edge cases 21–22 describe operator-facing consequences but no FR or task requires producing migration guidance documentation. [Ambiguity, Gap]
- [ ] CHK062 - Does FR-049's scope ("all internal Rust crate references") include or exclude the `hve-core` library crate under `lib/`? The rename categories in plan.md do not list `lib/hve-core` as a target. [Ambiguity, Spec §FR-049 / Plan §Phase17]
