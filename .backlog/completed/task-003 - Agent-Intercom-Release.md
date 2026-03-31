---
id: TASK-003
title: "Agent Intercom Release"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - feature
dependencies: []
ordinal: 3000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
# Feature Specification: Agent Intercom Release

**Feature Branch**: `003-agent-intercom-release`  
**Created**: 2026-02-23  
**Status**: Draft  
**Input**: User description: "Rename solution to agent-intercom; remove ALL reference to the word monocoque. Root folder in project workspace should use .intercom. MCP tool should show as 'intercom' in list of tools in IDE. Full product documentation update including setup guide, user guide, and developer guide. Release pipeline that produces release executable with feature flagging, tagging, gating, and other related capabilities. Fix missing Slack notifications for approval requests, diff acceptance approvals, and agent session continuation approvals. Upgrade rmcp crate to 0.13.0 with full feature refactor. Consider intercom-themed tool command names."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Consistent Product Identity (Priority: P1)

A developer clones the repository, builds from source or downloads a release binary, and encounters a single cohesive product name — "agent-intercom" — throughout the codebase, binaries, documentation, configuration files, Slack commands, and MCP tool surface. No references to the former "monocoque" brand remain anywhere a user or agent would encounter them.

**Why this priority**: Product identity is the foundation for all other release activities. Documentation, release artifacts, and tool naming cannot be finalized until the name is settled and applied consistently. This is also the highest-risk item due to the ~848 occurrences of the old name across ~110 files, including Cargo package names, binary names, keychain service identifiers, Slack slash commands, environment variable prefixes, IPC pipe names, and test fixtures.

**Independent Test**: Can be verified by building the project (`cargo build`), running all tests (`cargo test`), and performing a full-text search for "monocoque" across the workspace — zero matches expected outside of git history and archived ADR/spec documents that describe the migration itself.

**Acceptance Scenarios**:

1. **Given** the repository is freshly cloned, **When** a developer runs `cargo build --release`, **Then** the output binaries are named `agent-intercom` and `agent-intercom-ctl` (plus platform extension).
2. **Given** a developer opens the `Cargo.toml`, **When** they inspect the package metadata, **Then** the package name is `agent-intercom`, the binary names are `agent-intercom` and `agent-intercom-ctl`, and no "monocoque" string appears.
3. **Given** a user connects the MCP server to their IDE, **When** the IDE displays the MCP server name in the tools list, **Then** the server identifies itself as `"agent-intercom"` with the current version number.
4. **Given** an operator types `/` in a Slack channel with the app installed, **When** Slack autocompletes available commands, **Then** the root command is `/intercom` (not `/monocoque`).
5. **Given** a workspace has a policy configuration directory, **When** the server loads auto-approve policies, **Then** it reads from `.intercom/settings.json` (not `.agentrc/settings.json`).
6. **Given** the server stores credentials in the OS keychain, **When** the keychain entry is inspected, **Then** the service name is `agent-intercom` (not `monocoque-agent-rc`).
7. **Given** the IPC socket/pipe is created for local CLI communication, **When** the pipe name is inspected, **Then** it uses `agent-intercom` as the identifier.
8. **Given** a developer searches the workspace for "monocoque", **When** results are returned, **Then** only historical ADR documents and this migration spec contain the term.

---

### User Story 2 — Reliable Slack Notifications (Priority: P2)

An operator reviews and responds to agent actions via Slack. Every significant lifecycle event — approval requests, approval/rejection outcomes, diff application success or failure, session continuation requests, and error conditions — produces a visible Slack notification so the operator maintains full situational awareness without consulting server logs.

**Why this priority**: The missing notifications represent a functional gap where the operator approves a change but never learns if the application succeeded or failed. This creates a silent-failure mode that undermines trust in the system. Fixing this is higher priority than documentation or release pipeline because it directly impacts the core human-in-the-loop value proposition.

**Independent Test**: Can be tested with HITL (human-in-the-loop) scenarios against a running server with Slack integration, verifying that each lifecycle event produces the expected Slack message.

**Acceptance Scenarios**:

1. **Given** an agent submits a code proposal via `ask_approval`, **When** the Slack channel is configured, **Then** the operator receives a Slack message with the diff, risk level indicator, and Accept/Reject buttons.
2. **Given** an operator approves a proposal, **When** the agent subsequently calls `accept_diff` and the patch applies successfully, **Then** the operator receives a confirmation message in Slack showing the file path and bytes written.
3. **Given** an operator approves a proposal, **When** the agent calls `accept_diff` but the file has changed since the proposal (hash mismatch), **Then** the operator receives an alert in Slack indicating a patch conflict occurred and the change was not applied.
4. **Given** an operator rejects a proposal, **When** the rejection is processed, **Then** the operator receives a summary message confirming the rejection was communicated to the agent.
5. **Given** an agent calls `forward_prompt` with a continuation question, **When** the Slack channel is configured, **Then** the operator receives a Slack message with the prompt text and Continue/Refine/Stop action buttons.
6. **Given** an agent calls `wait_for_instruction` to enter standby, **When** the Slack channel is configured, **Then** the operator receives a Slack notification that the agent is waiting for instructions.
7. **Given** an agent calls `ask_approval` but no Slack channel is configured for the session, **When** the server processes the request, **Then** the server returns a descriptive error to the agent indicating no notification channel is available rather than blocking silently.
8. **Given** an `accept_diff` call is made with `force: true` due to a hash mismatch, **When** the forced write succeeds, **Then** the operator receives a Slack warning indicating the change was force-applied despite a file conflict.

---

### User Story 3 — Comprehensive Product Documentation (Priority: P3)

A new user discovers the project and finds well-organized, complete documentation covering installation, configuration, all features, usage patterns, development workflows, and the local CLI companion. The documentation reflects the renamed product and current feature set.

**Why this priority**: Good documentation is essential for adoption but does not gate other development work. The existing README, setup guide, user guide, and reference document provide a foundation but need restructuring, expansion, and renaming to cover the full feature surface under the new brand.

**Independent Test**: Can be verified by having a new user follow the documentation end-to-end — from installation through first approval workflow — without needing to consult source code or ask for help.

**Acceptance Scenarios**:

1. **Given** a new user reads the README, **When** they follow the Quick Start section, **Then** they can install, configure, and run the server within 15 minutes.
2. **Given** a user reads the Setup Guide, **When** they create a Slack app and configure credentials, **Then** every step is documented with clear instructions and no steps require guessing.
3. **Given** a user reads the User Guide, **When** they look up any MCP tool, Slack command, or CLI command, **Then** they find a complete description with parameters, examples, and expected behavior.
4. **Given** a developer reads the Developer Guide, **When** they want to contribute, **Then** they find build instructions, test commands, project structure explanation, coding conventions, and the approval workflow process.
5. **Given** a user reads the `agent-intercom-ctl` CLI documentation, **When** they look up any subcommand, **Then** they find usage examples, parameter descriptions, and common workflows.
6. **Given** all documentation files, **When** searched for "monocoque", **Then** no references to the old name remain (except in historical migration notes if any).

---

### User Story 4 — Intercom-Themed Tool Names (Priority: P4)

An agent connecting to the MCP server sees tool names that evoke "intercom" terminology, making the toolset more memorable and thematically cohesive. The tool names are clear, discoverable, and self-documenting despite the thematic naming convention.

**Why this priority**: Tool renaming is a cosmetic/branding enhancement that should happen alongside the rebranding but does not block functionality. It also requires coordination with documentation and existing agent prompts/instructions that reference current tool names.

**Independent Test**: Can be verified by connecting an MCP client and listing available tools — all tools should have intercom-themed names that are self-describing.

**Acceptance Scenarios**:

1. **Given** an agent connects to the server via MCP, **When** it requests the tool list, **Then** all tools have intercom-themed names that clearly convey their purpose.
2. **Given** the tool naming convention is established, **When** a developer reads a tool name, **Then** the name clearly indicates whether the tool is blocking (requires operator response) or non-blocking.
3. **Given** existing agent instructions reference old tool names, **When** the rename is applied, **Then** all agent prompt files, copilot-instructions, and documentation are updated to use the new names.
4. **Given** the tool list is displayed in an IDE, **When** the user browses available tools, **Then** all tools are grouped under the "intercom" server name with descriptive labels.

---

### User Story 5 — rmcp 0.13 Upgrade (Priority: P5)

The MCP SDK dependency is upgraded from rmcp 0.5 to rmcp 0.13.0, bringing the server up to date with the latest MCP protocol features, transport improvements, and bug fixes. All existing functionality is preserved through the upgrade.

**Why this priority**: While the upgrade brings protocol improvements, it involves breaking API changes that require refactoring the MCP handler layer, transport layer, and tool registration patterns. This is high-effort and should be attempted after the rename and notification fixes are stable to avoid compounding risk.

**Independent Test**: Can be verified by running the full test suite (`cargo test`) with the upgraded dependency, plus manual verification that both SSE and stdio transports work correctly with MCP clients.

**Acceptance Scenarios**:

1. **Given** the rmcp dependency is updated to 0.13.0 in Cargo.toml, **When** the project is built, **Then** compilation succeeds with zero errors and zero clippy warnings.
2. **Given** the upgraded server is running, **When** an MCP client connects via SSE transport, **Then** the connection, tool listing, and tool invocations work identically to the 0.5 behavior.
3. **Given** the upgraded server is running, **When** an MCP client connects via stdio transport, **Then** the connection, tool listing, and tool invocations work identically to the 0.5 behavior.
4. **Given** all existing contract tests, **When** they run against the upgraded handler, **Then** all tests pass without modification to test assertions (only test setup/wiring may change).
5. **Given** the upgrade introduces new MCP capabilities, **When** the server configuration is reviewed, **Then** the server opts in to new capabilities only when they align with existing feature requirements.

---

### User Story 6 — Release Pipeline (Priority: P6)

A maintainer triggers a release workflow that produces versioned, platform-specific binaries with proper tagging, changelog generation, and distribution through GitHub Releases. The pipeline supports feature flagging for gating unreleased capabilities.

**Why this priority**: The release pipeline is the final piece needed for distribution. It depends on the rename being complete (binary names), documentation being final, and the codebase being stable. Feature flagging is included to allow incremental rollout of capabilities like the rmcp upgrade.

**Independent Test**: Can be verified by triggering the release workflow on a test tag and confirming it produces correctly named binaries for all target platforms with proper version metadata.

**Acceptance Scenarios**:

1. **Given** a maintainer pushes a semver tag (e.g., `v1.0.0`), **When** the CI/CD pipeline runs, **Then** it produces release binaries for Windows x64, Linux x64, macOS Apple Silicon, and macOS Intel.
2. **Given** the pipeline completes, **When** the GitHub Release page is checked, **Then** it contains versioned archives named `agent-intercom-vX.Y.Z-{target}.{ext}` with the correct binaries inside.
3. **Given** the pipeline runs, **When** the changelog is generated, **Then** it includes all conventional commit messages since the last tag, grouped by type (feat, fix, docs, etc.).
4. **Given** a feature flag is defined for an unreleased capability, **When** a release build is produced, **Then** the flagged feature is compiled out (or disabled at runtime) unless the flag is explicitly enabled.
5. **Given** the release archive is downloaded by a user, **When** they extract it, **Then** it contains `agent-intercom` (or `.exe`), `agent-intercom-ctl`, and `config.toml.example`.

---

### Edge Cases

- What happens when an operator's OS keychain already has a credential stored under the old `monocoque-agent-rc` service name? The migration should document the manual keychain update step or provide a one-time migration utility.
- What happens when existing workspaces have `.agentrc/settings.json` but not `.intercom/settings.json`? Users must manually rename the directory. The migration guide documents this step.
- What happens when Slack app slash commands are updated from `/monocoque` to `/intercom`? Users with existing Slack apps will need to update their app configuration. The setup guide must document this migration step.
- What happens when a release build is triggered but compilation fails on one platform? The pipeline should fail the entire release rather than publishing partial artifacts.
- What happens when rmcp 0.13.0 changes the MCP protocol negotiation? The server must gracefully handle both old and new protocol clients during transition, or document the minimum client version requirement.
- What happens when environment variables use the old `MONOCOQUE_` prefix? The server will not recognize them. The migration guide documents the required env var updates.

## Requirements *(mandatory)*

### Functional Requirements

#### Rebranding

- **FR-001**: System MUST rename all Rust crate identifiers from `monocoque-agent-rc` / `monocoque_agent_rc` to `agent-intercom` / `agent_intercom` across Cargo.toml, lib.rs, main.rs, and all `use` statements.
- **FR-002**: System MUST produce binaries named `agent-intercom` and `agent-intercom-ctl` (replacing `monocoque-agent-rc` and `monocoque-ctl`).
- **FR-003**: System MUST rename the workspace policy directory from `.agentrc/` to `.intercom/`. No backward-compatible fallback; this is a hard cutover.
- **FR-004**: System MUST update the MCP server identity to report `name: "agent-intercom"` and `version: "{crate_version}"` in the `ServerInfo` response.
- **FR-005**: System MUST rename the Slack slash command root from `/monocoque` to `/intercom`.
- **FR-006**: System MUST update the OS keychain service name from `monocoque-agent-rc` to `agent-intercom`.
- **FR-007**: System MUST update the IPC pipe/socket name from `monocoque-agent-rc` to `agent-intercom`.
- **FR-008**: System MUST update all environment variable prefixes from `MONOCOQUE_` to `INTERCOM_` (e.g., `MONOCOQUE_WORKSPACE_ROOT` → `INTERCOM_WORKSPACE_ROOT`). No backward-compatible fallback; this is a hard cutover.
- **FR-009**: System MUST remove or replace all occurrences of "monocoque" in source code comments, doc comments, log messages, error messages, and user-facing strings.
- **FR-010**: System MUST update the repository metadata (Cargo.toml `repository`, `homepage`, description) to reflect the new name.

#### Slack Notifications

- **FR-011**: System MUST post a Slack notification when an `ask_approval` request is submitted and a Slack channel is configured.
- **FR-012**: System MUST post a Slack confirmation when `accept_diff` successfully applies an approved change, showing the file path and bytes written.
- **FR-013**: System MUST post a Slack alert when `accept_diff` encounters a patch conflict (hash mismatch), informing the operator the approved change was not applied.
- **FR-014**: System MUST post a Slack notification when `accept_diff` is called with `force: true`, warning the operator that a file conflict was overridden.
- **FR-015**: System MUST return a descriptive error to the agent when `ask_approval` is called without a configured Slack channel, rather than blocking silently.
- **FR-016**: System MUST post a Slack notification when `forward_prompt` submits a continuation prompt, displaying the prompt text and action buttons.
- **FR-017**: System MUST post a Slack notification when `wait_for_instruction` places the agent in standby, informing the operator that the agent is waiting.
- **FR-018**: System MUST post a Slack notification when a proposal rejection is communicated to the agent, confirming delivery.

#### Documentation

- **FR-019**: System MUST provide an updated README.md reflecting the new product name, binary names, configuration, and quick start instructions.
- **FR-020**: System MUST provide an updated Setup Guide covering Slack app creation, credential configuration, and first run for the renamed product.
- **FR-021**: System MUST provide an updated User Guide covering all MCP tools (with any renamed tool names), all Slack commands (under `/intercom`), all CLI subcommands (under `agent-intercom-ctl`), and workspace policy configuration.
- **FR-022**: System MUST provide a new Developer Guide covering build instructions, test commands, project structure, coding conventions, contribution workflow, and the approval workflow process.
- **FR-023**: System MUST provide complete `agent-intercom-ctl` CLI documentation with usage examples for every subcommand.
- **FR-024**: System MUST provide a migration guide documenting the steps for existing users to transition from the old naming to the new naming (keychain, Slack app commands, environment variables, workspace policy directory).

#### Tool Naming

- **FR-025**: System MUST adopt intercom-themed tool names using radio/intercom terminology. The canonical mapping is:

  | Old Name | New Name | Rationale |
  |---|---|---|
  | `ask_approval` | `check_clearance` | Requesting clearance to proceed |
  | `accept_diff` | `check_diff` | Checking and applying a cleared diff |
  | `check_auto_approve` | `auto_check` | Automatic clearance check |
  | `forward_prompt` | `transmit` | Transmitting a message to the operator |
  | `wait_for_instruction` | `standby` | Entering standby awaiting operator |
  | `heartbeat` | `ping` | Periodic liveness ping |
  | `remote_log` | `broadcast` | Broadcasting a status message |
  | `recover_state` | `reboot` | Recovering from prior session state |
  | `set_operational_mode` | `switch_freq` | Switching operational frequency/mode |

- **FR-026**: System MUST update all copilot-instructions, agent prompt files, and documentation to reference the new tool names after renaming.
- **FR-027**: System MUST ensure renamed tools maintain identical input schemas and output schemas to their predecessors (only the tool name changes, not the contract).

#### rmcp Upgrade

- **FR-028**: System MUST upgrade the `rmcp` dependency from 0.5 to 0.13.0 in Cargo.toml.
- **FR-029**: System MUST refactor `AgentRcServer` (or its successor) to implement the rmcp 0.13.0 `ServerHandler` trait, adapting to any breaking API changes.
- **FR-030**: System MUST refactor tool registration to use rmcp 0.13.0 patterns (whether `ToolRouter`, proc macros, or the new registration API).
- **FR-031**: System MUST maintain SSE and stdio transport support through the upgrade, adapting to any transport API changes in rmcp 0.13.0.
- **FR-032**: System MUST preserve all existing MCP tool contracts (input/output schemas) through the upgrade, validated by contract tests.

#### Release Pipeline

- **FR-033**: System MUST provide a GitHub Actions workflow that triggers on semver tags (`v*.*.*`) and produces release binaries for Windows x64, Linux x64, macOS ARM64, and macOS x64.
- **FR-034**: System MUST package each platform's release as a versioned archive containing the server binary, CLI binary, and example config file.
- **FR-035**: System MUST generate a changelog from conventional commit messages and attach it to the GitHub Release.
- **FR-036**: System MUST support compile-time feature flags (via Cargo features) for gating unreleased capabilities in release builds.
- **FR-037**: System MUST embed the crate version (from Cargo.toml) into the compiled binary, accessible via `--version` flag and MCP ServerInfo.

### Key Entities

- **Release Artifact**: A platform-specific archive containing the server binary, CLI binary, and configuration template, published to GitHub Releases.
- **Feature Flag**: A compile-time Cargo feature that gates inclusion of unreleased or experimental capabilities in release builds.
- **Tool Name Mapping**: The correspondence between old tool names and new intercom-themed tool names, maintained as a reference during migration.
- **Migration Guide**: A document capturing all user-facing breaking changes with step-by-step transition instructions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A full-text search of the workspace (excluding git history and this migration spec) returns zero matches for "monocoque".
- **SC-002**: `cargo build --release` produces binaries named `agent-intercom` and `agent-intercom-ctl` with the correct version embedded.
- **SC-003**: All existing tests (`cargo test`) pass after the rename with zero failures.
- **SC-004**: An operator observing a full agent session (proposal → approve → apply → continue → standby) receives at minimum 5 distinct Slack notifications covering each lifecycle event.
- **SC-005**: A new user can follow the documentation from download to first successful approval workflow in under 30 minutes.
- **SC-006**: The release pipeline produces correctly named, properly versioned archives for all 4 target platforms when triggered by a semver tag.
- **SC-007**: Clippy passes with zero warnings after the rmcp 0.13.0 upgrade and full rename.
- **SC-008**: Contract tests validate that all MCP tools return correct response schemas after rename and upgrade.

## Clarifications

### Session 2026-02-23

- Q: What tool naming scheme should be used for FR-025 intercom-themed tool names? → A: Radio/intercom terminology with specific overrides: `check_clearance` (ask_approval), `check_diff` (accept_diff), `auto_check` (check_auto_approve), `transmit` (forward_prompt), `standby` (wait_for_instruction), `ping` (heartbeat), `broadcast` (remote_log), `reboot` (recover_state), `switch_freq` (set_operational_mode).
- Q: How long should the deprecation period last for old .agentrc/ directory and MONOCOQUE_ env vars? → A: No deprecation period. Hard cutover — old names stop working immediately.

## Assumptions

- The Slack app configuration (slash command names, bot token scopes) is controlled by the project team and can be updated to reflect the `/intercom` root command.
- The GitHub repository will remain at `softwaresalt/agent-intercom` (already renamed at the repository level).
- Users running existing installations will perform a one-time migration using the provided migration guide. No backward compatibility is provided for old environment variable prefixes or policy directories; the cutover is immediate.
- The rmcp 0.13.0 crate is published and available on crates.io. If the specific version is not yet available, the upgrade user story will target the latest available 0.x release.
- Feature flags use Cargo's built-in conditional compilation (`#[cfg(feature = "...")]`) rather than runtime configuration to ensure unused code is excluded from release binaries.
- The release pipeline uses GitHub Actions (the repository's existing CI platform).


# Behavioral Matrix: Agent Intercom Release

**Input**: Design documents from `specs/003-agent-intercom-release/`  
**Prerequisites**: spec.md (required), plan.md (required), data-model.md, contracts/  
**Created**: 2026-02-23

## Summary

| Metric | Count |
|---|---|
| **Total Scenarios** | 62 |
| Happy-path | 24 |
| Edge-case | 14 |
| Error | 12 |
| Boundary | 5 |
| Concurrent | 2 |
| Security | 5 |

**Non-happy-path coverage**: 61% (minimum 30% required)

## Rebranding — Cargo & Crate Identifiers

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S001 | Cargo package name updated | Cargo.toml `[package] name = "agent-intercom"` | `cargo check` | Compilation succeeds | Exit 0 | happy-path |
| S002 | Binary name: server | Cargo.toml `[[bin]] name = "agent-intercom"` | `cargo build --release` | Binary `agent-intercom(.exe)` produced in `target/release/` | File exists | happy-path |
| S003 | Binary name: CLI | Cargo.toml `[[bin]] name = "agent-intercom-ctl"` | `cargo build --release` | Binary `agent-intercom-ctl(.exe)` produced in `target/release/` | File exists | happy-path |
| S004 | Crate import paths updated | All `use monocoque_agent_rc::` changed to `use agent_intercom::` | `cargo check` | Zero compilation errors | Exit 0 | happy-path |
| S005 | No monocoque in source | After full rename | `grep -r "monocoque" src/ ctl/` | Zero matches | Exit 1 (no match) | happy-path |
| S006 | No monocoque in tests | After full rename | `grep -r "monocoque" tests/` | Zero matches | Exit 1 (no match) | happy-path |
| S007 | No monocoque in config | After full rename | `grep "monocoque" config.toml Cargo.toml` | Zero matches | Exit 1 (no match) | happy-path |
| S008 | Stale import causes build failure | One file still has `use monocoque_agent_rc::` | `cargo check` | Compilation error: unresolved import | Exit non-zero | error |

---

## Rebranding — Keychain, IPC, Environment Variables

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S009 | Keychain service name updated | `KEYCHAIN_SERVICE = "agent-intercom"` | Server loads credentials from keychain | Reads from `agent-intercom` service | Credentials loaded | happy-path |
| S010 | IPC pipe name updated (server) | IPC pipe uses `agent-intercom` identifier | Server starts IPC listener | Named pipe / socket created as `agent-intercom` | Pipe exists | happy-path |
| S011 | IPC pipe name updated (ctl) | CLI connects to `agent-intercom` pipe | `agent-intercom-ctl list` | CLI connects to correct pipe | Successful connection | happy-path |
| S012 | Environment variable prefix INTERCOM_ | `INTERCOM_WORKSPACE_ROOT` set | Server reads env vars | Reads `INTERCOM_WORKSPACE_ROOT` value | Config populated | happy-path |
| S013 | Old MONOCOQUE_ env var ignored | `MONOCOQUE_WORKSPACE_ROOT` set, `INTERCOM_WORKSPACE_ROOT` not set | Server reads env vars | `MONOCOQUE_` var is not recognized | Config uses default or errors | edge-case |
| S014 | Old keychain entry not found | Only `monocoque-agent-rc` keychain entry exists | Server loads credentials from `agent-intercom` | Keychain entry not found, falls back to env vars | Falls back to env | edge-case |

---

## Rebranding — Policy Directory

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S015 | Policy loads from .intercom/ | `.intercom/settings.json` exists in workspace | `auto_check` tool called | Policy loaded from `.intercom/settings.json` | Policy applied | happy-path |
| S016 | No .intercom/ directory | Neither `.intercom/` nor `.agentrc/` exists | `auto_check` tool called | Default policy (no auto-approve) | No auto-approvals | edge-case |
| S017 | Old .agentrc/ ignored | Only `.agentrc/settings.json` exists (not `.intercom/`) | `auto_check` tool called | `.agentrc/` not recognized; default policy used | No auto-approvals | edge-case |

---

## Rebranding — Slack Commands

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S018 | /intercom help command | Slack app with `/intercom` configured | User types `/intercom help` | Help message listing all commands | Message posted | happy-path |
| S019 | /intercom sessions command | Active sessions exist | User types `/intercom sessions` | Session list displayed | Message posted | happy-path |
| S020 | /intercom session-start | Valid prompt provided | User types `/intercom session-start "fix bug"` | New agent session initiated | Session created | happy-path |

---

## MCP Identity & Tool Naming

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S021 | ServerInfo reports correct name | Server running | MCP client connects, requests server info | `name: "agent-intercom"`, `version: "{crate_version}"` | ServerInfo returned | happy-path |
| S022 | Tool list uses new names | Server running | MCP client calls `tools/list` | Returns 9 tools: check_clearance, check_diff, auto_check, transmit, standby, ping, broadcast, reboot, switch_freq | List returned | happy-path |
| S023 | Old tool name rejected | Server running | MCP client calls `call_tool` with name `ask_approval` | Error: unknown tool `ask_approval` | Error returned | error |
| S024 | check_clearance schema unchanged | Server running | MCP client inspects `check_clearance` input schema | Same fields as old `ask_approval`: title, diff, file_path, description, risk_level | Schema returned | happy-path |
| S025 | check_diff schema unchanged | Server running | MCP client inspects `check_diff` input schema | Same fields as old `accept_diff`: request_id, force | Schema returned | happy-path |
| S026 | All 9 tools visible regardless of config | Minimal server config (no Slack channel) | MCP client calls `tools/list` | All 9 tools listed | List returned | happy-path |
| S027 | Tool call with empty name | Server running | MCP client calls `call_tool` with name `""` | Error: unknown tool | Error returned | boundary |

---

## Slack Notifications — accept_diff Outcomes

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S028 | Notification on successful patch apply | Approved request, file hash matches | Agent calls `check_diff` with valid `request_id` | Patch applied; Slack message posted: file path, bytes written | Applied + notified | happy-path |
| S029 | Notification on patch conflict | Approved request, file hash changed since proposal | Agent calls `check_diff` without `force` | Conflict error returned to agent; Slack alert posted | Not applied + alert sent | error |
| S030 | Notification on force-apply | Approved request, file hash changed, `force: true` | Agent calls `check_diff` with `force: true` | Patch force-applied; Slack warning posted | Applied + warning sent | edge-case |
| S031 | No notification when Slack unavailable | No Slack channel configured for session | Agent calls `check_diff` | Patch applied; no Slack message; only logged | Applied + logged only | edge-case |
| S032 | Full file write notification | New file (no prior content), approved request | Agent calls `check_diff` with valid `request_id` | File written; Slack confirmation posted | Written + notified | happy-path |

---

## Slack Notifications — ask_approval Without Channel

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S033 | Error when no Slack channel | No `channel_id` in SSE URL or session | Agent calls `check_clearance` | Descriptive error returned: "no Slack channel configured" | Error, not blocked | error |
| S034 | Error when Slack service unavailable | Slack connection lost | Agent calls `check_clearance` | Descriptive error returned about Slack unavailability | Error, not blocked | error |
| S035 | Approval works with valid channel | `channel_id` configured in SSE URL | Agent calls `check_clearance` with valid params | Approval request posted to Slack, agent blocks | Waiting for response | happy-path |

---

## Slack Notifications — Rejection Delivery

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S036 | Rejection confirmation posted | Operator clicks Reject on approval message | Rejection processed | Slack message updated; confirmation that rejection was delivered | Agent unblocked | happy-path |
| S037 | Rejection with reason | Operator clicks Reject; reason provided | Rejection processed | Reason included in rejection response to agent and Slack update | Agent receives reason | happy-path |

---

## Slack Notifications — forward_prompt and wait_for_instruction

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S038 | transmit notification posted | Slack channel configured | Agent calls `transmit` with prompt text | Slack message with prompt and Continue/Refine/Stop buttons | Agent blocking, message visible | happy-path |
| S039 | standby notification posted | Slack channel configured | Agent calls `standby` | Slack message: "Agent is waiting for instructions" | Agent blocking, message visible | happy-path |
| S040 | transmit without Slack channel | No channel_id configured | Agent calls `transmit` | Error returned: no Slack channel | Agent not blocked | error |
| S041 | standby without Slack channel | No channel_id configured | Agent calls `standby` | Error returned: no Slack channel | Agent not blocked | error |

---

## Documentation

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S042 | README reflects new branding | docs/README.md published | Reader searches for "monocoque" | Zero occurrences; all references say "agent-intercom" | Consistent branding | happy-path |
| S043 | Setup guide covers credential migration | docs/setup-guide.md exists | User follows setup instructions | Instructions reference `agent-intercom` keychain service, `INTERCOM_` env vars | Guide complete | happy-path |
| S044 | User guide covers all tools with new names | docs/user-guide.md exists | User searches for tool documentation | All 9 tools documented with new names: check_clearance, check_diff, etc. | Guide complete | happy-path |
| S045 | Developer guide exists | docs/developer-guide.md exists | Developer reads for contribution | Build instructions, test commands, project structure, conventions documented | Guide complete | happy-path |
| S046 | Migration guide exists | docs/migration-guide.md exists | Existing user reads migration steps | Covers keychain, env vars, .intercom dir, Slack commands, mcp.json updates | Guide complete | happy-path |
| S047 | No monocoque in documentation | All docs/ files | Search for "monocoque" | Zero matches (except migration guide referencing old name for context) | Clean docs | boundary |

---

## Release Pipeline

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S048 | Release triggered by semver tag | Tag `v1.0.0` pushed | GitHub Actions workflow runs | Builds for 4 platforms initiated | Workflow running | happy-path |
| S049 | Windows x64 binary produced | Release workflow on tag | Windows build step | `agent-intercom-v1.0.0-x86_64-pc-windows-msvc.zip` created | Archive exists | happy-path |
| S050 | Linux x64 binary produced | Release workflow on tag | Linux build step | `agent-intercom-v1.0.0-x86_64-unknown-linux-gnu.tar.gz` created | Archive exists | happy-path |
| S051 | macOS binaries produced | Release workflow on tag | macOS build steps | ARM64 and x64 archives created | Archives exist | happy-path |
| S052 | Changelog generated | Conventional commits since last tag | Release workflow | Changelog attached to GitHub Release | Release has changelog | happy-path |
| S053 | --version flag works | Binary built | `agent-intercom --version` | Prints version matching Cargo.toml | Version printed | happy-path |
| S054 | Feature flag excludes code | `default = []`, code behind `#[cfg(feature = "rmcp-upgrade")]` | `cargo build --release` (no features) | Flagged code is not compiled into binary | Binary smaller | edge-case |
| S055 | Feature flag includes code | `--features rmcp-upgrade` passed | `cargo build --release --features rmcp-upgrade` | Flagged code is compiled into binary | Binary includes feature | edge-case |
| S056 | Partial platform failure aborts release | macOS build fails, Windows/Linux succeed | Release workflow | Entire release marked as failed; no partial publish | No artifacts published | error |

---

## rmcp 0.13 Upgrade

| Scenario ID | Scenario Description | Input State / Data | Execution Trigger | Expected Output / Behavior | Expected System State / Exit Code | Category |
|---|---|---|---|---|---|---|
| S057 | Streamable HTTP transport works | rmcp 0.13, StreamableHttpService on /mcp | MCP client connects via HTTP POST | Connection established, tool list returned | Connected | happy-path |
| S058 | Stdio transport still works | rmcp 0.13, stdio transport | MCP client connects via stdin/stdout | Connection established, tool list returned | Connected | happy-path |
| S059 | Contract tests pass after upgrade | All contract test files, rmcp 0.13 | `cargo test --test contract` | All contract tests pass | Exit 0 | happy-path |
| S060 | Old SSE endpoint redirects | Client connects to /sse (old endpoint) | HTTP GET /sse | Redirect to /mcp or descriptive error | Redirect or 410 Gone | edge-case |
| S061 | Concurrent MCP connections | Two MCP clients connect simultaneously | Both connect via StreamableHttpService | Both connections handled independently | Two sessions active | concurrent |
| S062 | Connection drops handled | MCP client disconnects mid-session | TCP connection closed | Server cleans up session resources; no crash | Server stable | concurrent |

---

## Edge Case Coverage Checklist

- [x] Malformed inputs and invalid arguments (S008, S023, S027)
- [x] Missing dependencies and unavailable resources (S014, S016, S017, S034)
- [x] State errors and race conditions (S029, S060)
- [x] Boundary values (S027, S047, S054, S055)
- [x] Permission and authorization failures (S013, S033, S040, S041)
- [x] Concurrent access patterns (S061, S062)


# Data Model: 003-agent-intercom-release

**Branch**: `003-agent-intercom-release` | **Date**: 2026-02-23

## Overview

This feature introduces **no new database tables, domain entities, or schema migrations**. All changes are to naming conventions, notification behavior, transport infrastructure, and build/release tooling.

## Existing Entities (Unchanged)

The following entities are part of the existing data model and are not modified by this feature:

| Entity | Table | Impact |
|---|---|---|
| Session | `sessions` | No changes |
| ApprovalRequest | `approval_requests` | No changes |
| Prompt | `prompts` | No changes |
| Checkpoint | `checkpoints` | No changes |
| StallEvent | `stall_events` | No changes |

## Name-Affected Constants

The following constants change as part of the rename but do not affect stored data:

| Constant | Old Value | New Value | Location |
|---|---|---|---|
| `KEYCHAIN_SERVICE` | `"monocoque-agent-rc"` | `"agent-intercom"` | `src/config.rs` |
| `POLICY_DIR` | `".agentrc"` | `".intercom"` | `src/policy/watcher.rs` |
| `POLICY_PATH` | `".agentrc/settings.json"` | `".intercom/settings.json"` | `src/policy/loader.rs` |
| IPC pipe name | `"monocoque-agent-rc"` | `"agent-intercom"` | `src/ipc/socket.rs`, `ctl/main.rs` |
| Slash command root | `"/monocoque"` | `"/intercom"` | `src/slack/commands.rs` |
| Env var prefix | `MONOCOQUE_` | `INTERCOM_` | `src/config.rs`, `src/orchestrator/spawner.rs` |

## Tool Name Mapping

| Old Tool Name | New Tool Name |
|---|---|
| `ask_approval` | `check_clearance` |
| `accept_diff` | `check_diff` |
| `check_auto_approve` | `auto_check` |
| `forward_prompt` | `transmit` |
| `wait_for_instruction` | `standby` |
| `heartbeat` | `ping` |
| `remote_log` | `broadcast` |
| `recover_state` | `reboot` |
| `set_operational_mode` | `switch_freq` |


# Quickstart: 003-agent-intercom-release

**Branch**: `003-agent-intercom-release` | **Date**: 2026-02-23

## Getting Started with Implementation

### Prerequisites

- Rust stable toolchain (edition 2021)
- Git with the `003-agent-intercom-release` branch checked out
- Working Slack app credentials (for notification testing)
- Familiarity with the existing codebase structure (see `specs/001-mcp-remote-agent-server/`)

### Implementation Order

Follow the phases in [plan.md](plan.md) strictly. Each phase has a clear entry gate (previous phase passes `cargo check`) and exit gate (current phase passes `cargo check` + relevant tests).

### Phase 1: Cargo + Core Source Rename

```bash
# 1. Update Cargo.toml: package name, binary names, metadata
# 2. Update src/lib.rs, src/main.rs, ctl/main.rs crate identifiers
# 3. Update all `use monocoque_agent_rc::` to `use agent_intercom::`
# 4. Update string constants (keychain, IPC, env vars, policy dir)
# 5. Verify:
cargo check
```

### Phase 2: Tool Naming + MCP Identity

```bash
# 1. Update Tool::name fields in handler.rs (9 tools)
# 2. Update ToolRouter dispatch keys
# 3. Set ServerInfo { name: "agent-intercom", version: env!("CARGO_PKG_VERSION") }
# 4. Update Slack slash command root: /monocoque → /intercom
# 5. Verify:
cargo check
```

### Phase 3: Slack Notification Gaps

```bash
# 1. Write tests for each notification gap (TDD — red first)
# 2. Add Slack messages in accept_diff for success/conflict/force-apply
# 3. Add error return in ask_approval when no channel configured
# 4. Verify:
cargo test
```

### Phase 4: Test Suite Update

```bash
# 1. Update all test imports from monocoque_agent_rc to agent_intercom
# 2. Update test assertions referencing old tool names
# 3. Update test fixtures referencing .agentrc or /monocoque
# 4. Verify:
cargo test
```

### Phase 5: Documentation

```bash
# 1. Rewrite README.md with new branding
# 2. Update setup-guide.md, user-guide.md, REFERENCE.md
# 3. Create developer-guide.md and migration-guide.md
# 4. Update copilot-instructions.md and agent files
# 5. Verify: search for "monocoque" — zero matches outside specs/
```

### Phase 6: Release Pipeline

```bash
# 1. Create .github/workflows/release.yml
# 2. Add Cargo features for feature flagging
# 3. Add --version flag to both binaries
# 4. Test locally: cargo build --release
# 5. Test CI: push a test tag
```

### Phase 7: rmcp 0.13 Upgrade

```bash
# 1. Update Cargo.toml: rmcp version + features
# 2. Rewrite src/mcp/sse.rs (StreamableHttpService)
# 3. Update handler.rs for any API changes
# 4. Update integration tests
# 5. Verify:
cargo test
cargo clippy -- -D warnings
```

### Key References

| Document | Purpose |
|---|---|
| [spec.md](spec.md) | Feature requirements (FR-001 through FR-037) |
| [research.md](research.md) | Technical research and decisions |
| [data-model.md](data-model.md) | Name constant mapping, tool name mapping |
| [contracts/tool-name-mapping.md](contracts/tool-name-mapping.md) | Tool name contracts and notification contracts |
| [Constitution](.../../.specify/memory/constitution.md) | Project principles and quality gates |




---

## Checklists

# Specification Quality Checklist: Agent Intercom Release

**Purpose**: Validate specification completeness and quality before proceeding to planning  
**Created**: 2026-02-23  
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- All items passed validation. Spec is ready for `/speckit.clarify` or `/speckit.plan`.
- FR-001 through FR-010 cover the full rebranding surface area across ~848 occurrences in ~110 files.
- FR-011 through FR-018 explicitly address the 5 notification gaps identified during research.
- FR-025 defers the specific tool name mapping to the plan/behavior stage where concrete naming proposals can be evaluated.
- FR-028 through FR-032 scope the rmcp upgrade but acknowledge API specifics depend on 0.13.0 documentation review during planning.




---

## Contracts

# Tool Name Mapping Contract: 003-agent-intercom-release

**Branch**: `003-agent-intercom-release` | **Date**: 2026-02-23

## Contract Summary

This document defines the authoritative mapping between old MCP tool names and new intercom-themed tool names. The rename affects ONLY the tool `name` field in the MCP protocol. Input schemas, output schemas, and behavior are unchanged.

## Tool Name Contracts

### check_clearance (was: ask_approval)

**Name**: `check_clearance`  
**Blocking**: Yes  
**Input schema**: Unchanged — `title`, `diff`, `file_path`, `description`, `risk_level`  
**Output schema**: Unchanged — `status` (approved/rejected/timeout), `request_id`, `reason`

### check_diff (was: accept_diff)

**Name**: `check_diff`  
**Blocking**: No  
**Input schema**: Unchanged — `request_id`, `force`  
**Output schema**: Unchanged — `status` (applied), `files_written`

### auto_check (was: check_auto_approve)

**Name**: `auto_check`  
**Blocking**: No  
**Input schema**: Unchanged — `tool_name`, `context`  
**Output schema**: Unchanged — `auto_approved`, `reason`

### transmit (was: forward_prompt)

**Name**: `transmit`  
**Blocking**: Yes  
**Input schema**: Unchanged — `prompt`, `options`  
**Output schema**: Unchanged — `response`, `action`

### standby (was: wait_for_instruction)

**Name**: `standby`  
**Blocking**: Yes  
**Input schema**: Unchanged — `timeout_seconds`  
**Output schema**: Unchanged — `instruction`, `source`

### ping (was: heartbeat)

**Name**: `ping`  
**Blocking**: No  
**Input schema**: Unchanged — `session_id` (optional), `progress`  
**Output schema**: Unchanged — `session_id`, `status`, `stall_warning`

### broadcast (was: remote_log)

**Name**: `broadcast`  
**Blocking**: No  
**Input schema**: Unchanged — `message`, `level`  
**Output schema**: Unchanged — `delivered`

### reboot (was: recover_state)

**Name**: `reboot`  
**Blocking**: No  
**Input schema**: Unchanged — (no required params)  
**Output schema**: Unchanged — `has_interrupted_session`, `session`, `pending_approvals`

### switch_freq (was: set_operational_mode)

**Name**: `switch_freq`  
**Blocking**: No  
**Input schema**: Unchanged — `mode` (remote/local/hybrid)  
**Output schema**: Unchanged — `mode`, `previous_mode`

## Notification Contracts (New)

### accept_diff Success Notification

**Trigger**: `check_diff` (accept_diff) applies patch successfully  
**Channel**: Session's configured Slack channel  
**Content**: File path, bytes written, approval request_id  
**Block Kit**: Section block with success emoji + file details

### accept_diff Conflict Notification

**Trigger**: `check_diff` (accept_diff) encounters hash mismatch without `force: true`  
**Channel**: Session's configured Slack channel  
**Content**: File path, expected hash, actual hash, guidance to re-propose  
**Block Kit**: Section block with warning emoji + conflict details

### accept_diff Force-Apply Warning

**Trigger**: `check_diff` (accept_diff) called with `force: true` and hash mismatch  
**Channel**: Session's configured Slack channel  
**Content**: File path, warning that file conflict was overridden  
**Block Kit**: Section block with alert emoji + force-apply warning

### No Channel Error Response

**Trigger**: `check_clearance` (ask_approval) called without configured Slack channel  
**Channel**: N/A — returns error to agent  
**Content**: Descriptive error explaining no Slack channel is configured for this session  
**Response**: `CallToolResult` with `is_error: true`

### Rejection Delivery Confirmation

**Trigger**: Operator rejects a proposal via Slack buttons  
**Channel**: Session's configured Slack channel  
**Content**: Confirmation that rejection was delivered to the agent  
**Block Kit**: Context block appended to original approval message

<!-- SECTION:DESCRIPTION:END -->
