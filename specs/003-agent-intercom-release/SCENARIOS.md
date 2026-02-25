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
