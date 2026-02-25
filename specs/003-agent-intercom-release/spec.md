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
