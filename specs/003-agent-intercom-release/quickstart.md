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
