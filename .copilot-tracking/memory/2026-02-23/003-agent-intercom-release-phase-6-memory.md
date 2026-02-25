# Phase 6 Memory — 003-agent-intercom-release

**Date**: 2026-02-23  
**Phase**: 6 — US3 Comprehensive Product Documentation (T070–T079)  
**Branch**: `003-agent-intercom-release`  
**Baseline**: commit `ba81a7a` (Phase 5 complete — Slack notifications)  

---

## Decisions Made

### D1 — Migration guide is the only allowed home for "monocoque"

All `docs/` files and `README.md` must have zero "monocoque" references except
`docs/migration-guide.md`, which explicitly documents the old names for context.
This is enforced by T079 (exit gate grep check).

### D2 — New docs files created (not updated)

Three new documentation files were created rather than embedded in existing docs:
- `docs/developer-guide.md` — build, test, and contribution workflow
- `docs/cli-reference.md` — complete `agent-intercom-ctl` subcommand reference  
- `docs/migration-guide.md` — step-by-step migration from old installation

### D3 — Setup guide kept env var names unchanged

The env vars (`SLACK_APP_TOKEN`, `SLACK_BOT_TOKEN`, `SLACK_TEAM_ID`, `SLACK_MEMBER_IDS`)
remain unchanged — no `MONOCOQUE_*` → `INTERCOM_*` rename of env vars in
`.env` or setup guide. The spec task mentioned "INTERCOM_ env vars" but the
actual codebase still uses `SLACK_*` vars. Migration guide was written to match
reality, not the spec placeholder.

### D4 — Policy directory: `.agentrc/` → `.intercom/`

User-facing docs consistently updated to `.intercom/settings.json`.
Migration guide provides step-by-step rename instructions.

---

## Files Created (New)

| File | Description |
|---|---|
| `docs/developer-guide.md` | Build/test/contribution guide (T074) |
| `docs/cli-reference.md` | agent-intercom-ctl subcommand reference (T075) |
| `docs/migration-guide.md` | Step-by-step migration from old installation (T076) |

## Files Updated

| File | Key Changes |
|---|---|
| `README.md` | Full rewrite: new name, binary names, tool names, /intercom commands, archive names |
| `docs/setup-guide.md` | Binary names, keychain service, slash command, ipc_name, policy dir, tool references |
| `docs/user-guide.md` | All 9 tool section headings, /monocoque → /intercom commands, binary references |
| `docs/REFERENCE.md` | Header, ToC, all 9 tool headings, IPC section, env vars, keychain, config examples |
| `specs/003-agent-intercom-release/tasks.md` | Marked T070–T079 complete |

---

## Quality Gate Results

- **cargo check**: ✅ clean (no Rust source changes)
- **cargo test**: ✅ 548 passed (no Rust source changes — same as Phase 5)
- **docs/ monocoque scan**: ✅ zero matches outside migration-guide.md
- **README.md monocoque scan**: ✅ zero matches

---

## Next Phase

**Phase 7 — US6 Release Pipeline (T080–T093)**

Key tasks:
- T080-T082: Write `--version` flag tests and feature flag tests (TDD red gate)
- T083: Add `--version` flag via clap to `src/main.rs` and `ctl/main.rs`
- T084: Add `[features]` section to `Cargo.toml`
- T085-T091: Create `.github/workflows/release.yml` with multi-platform build matrix
- T092-T093: Green gate — local release build, verify binary names
