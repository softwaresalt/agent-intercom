# Phase 5 Memory — 003-agent-intercom-release

**Date**: 2026-02-23  
**Phase**: 5 — US2 Reliable Slack Notifications (T047–T069)  
**Branch**: `003-agent-intercom-release`  
**Baseline**: commit `faae526` (Phase 4 complete — all 9 tool renames)  

---

## Decisions Made

### D1 — `check_clearance` / `transmit` / `standby` error path: no-channel returns immediately

Blocking tools (`check_clearance`, `transmit`, `standby`) previously got stuck
indefinitely when no Slack channel was configured. They now return a structured
`{ status: "error", error_code: "no_channel"|"slack_unavailable", error_message: "..." }`
response immediately, without registering a oneshot sender. This prevents
silent hangs in channel-less sessions.

**Files changed**: `src/mcp/tools/ask_approval.rs`, `src/mcp/tools/forward_prompt.rs`,
`src/mcp/tools/wait_for_instruction.rs`

### D2 — `check_diff` (accept_diff) posts Slack on every path

Added Slack notifications for:
- **Successful apply**: `diff_applied_section` — posts file path + bytes written
- **Conflict (no force)**: `diff_conflict_section` — posts conflict alert before returning `patch_conflict` error
- **Force-apply**: `diff_force_warning_section` — posts warning before applying

All three builders live in `src/slack/blocks.rs`. Notifications are skipped
silently when no channel is configured (no error, just no Slack).

**Files changed**: `src/mcp/tools/accept_diff.rs`, `src/slack/blocks.rs`

### D3 — `mcp-tools.json` outputSchema updated for error paths

Three tools' outputSchemas now include `error_code` and `error_message` optional
properties. The `required` array for `check_clearance` was narrowed from
`["status", "request_id"]` to `["status"]` only — since `request_id` is absent
in error responses.

The existing `contract_schema_structure_is_valid` test in `ask_approval_tests.rs`
was updated to remove the `request_id` assertion from `required`, matching the
new contract.

### D4 — `WriteSummary.bytes_written` is `usize`, not `u64`

The `diff_applied_section` builder was initially written with `bytes: u64`. Fixed
to `bytes: usize` to match `WriteSummary.bytes_written` in `src/diff/writer.rs`.

---

## Tests Written (T047–T056)

| Test | File | Scenario |
|---|---|---|
| `notification_success_includes_file_path_and_bytes` | accept_diff_tests.rs | S028 |
| `notification_conflict_returns_patch_conflict_code` | accept_diff_tests.rs | S029 |
| `notification_force_apply_returns_applied_status` | accept_diff_tests.rs | S030 |
| `no_channel_success_output_is_same_structure` | accept_diff_tests.rs | S031 |
| `notification_new_file_write_includes_files_written` | accept_diff_tests.rs | S032 |
| `no_channel_error_code_structure_is_valid` | ask_approval_tests.rs | S033 |
| `slack_unavailable_error_structure_is_valid` | ask_approval_tests.rs | S034 |
| `rejection_confirmation_has_reason_field` | ask_approval_tests.rs | S036-S037 |
| `contract_check_clearance_schema_includes_error_code_property` | ask_approval_tests.rs | schema |
| `transmit_no_channel_error_code_structure` | forward_prompt_tests.rs | S040 |
| `contract_transmit_schema_includes_error_code_property` | forward_prompt_tests.rs | schema |
| `standby_no_channel_error_code_structure` | mode_tests.rs | S041 |
| `contract_standby_schema_includes_error_code_property` | mode_tests.rs | schema |

---

## Quality Gate Results

- **cargo check**: ✅ clean
- **cargo test**: ✅ 548 passed, 0 failed (17+0+0+170+210+150+1)
  - Contract: 170 (was 157 — +13 new Phase 5 tests)
  - Integration: 210
  - Unit: 150
- **cargo clippy --all-targets -- -D warnings -D clippy::pedantic**: ✅ clean
- **cargo fmt --all -- --check**: ✅ clean (auto-formatted once)

---

## Files Modified

- `src/mcp/tools/accept_diff.rs` — Added 3 Slack notification calls (success, conflict, force)
- `src/mcp/tools/ask_approval.rs` — Added early no-channel / no-Slack error return
- `src/mcp/tools/forward_prompt.rs` — Added early no-channel error return
- `src/mcp/tools/wait_for_instruction.rs` — Added early no-channel error return
- `src/slack/blocks.rs` — Added `diff_applied_section`, `diff_conflict_section`, `diff_force_warning_section`
- `specs/001-mcp-remote-agent-server/contracts/mcp-tools.json` — Added `error_code` to check_clearance, transmit, standby outputSchemas; narrowed check_clearance `required`
- `specs/003-agent-intercom-release/tasks.md` — Marked T047–T069 complete
- `tests/contract/accept_diff_tests.rs` — Added 5 new tests (T047–T051)
- `tests/contract/ask_approval_tests.rs` — Added 4 new tests (T052–T054); updated `contract_schema_structure_is_valid`
- `tests/contract/forward_prompt_tests.rs` — Added 2 new tests (T055)
- `tests/contract/mode_tests.rs` — Added 2 new tests (T056)

---

## Next Phase

**Phase 6 — US3 Comprehensive Product Documentation (T070–T079)**

Key tasks:
- Rewrite `README.md` with new name, binary names, quick start
- Update `docs/setup-guide.md`, `docs/user-guide.md`, `docs/REFERENCE.md`
- Create `docs/developer-guide.md`, `docs/migration-guide.md`, `docs/cli-reference.md`
- Sweep all doc comments in `src/` for old name references
- T079: verify zero "monocoque" in docs (except migration guide)
