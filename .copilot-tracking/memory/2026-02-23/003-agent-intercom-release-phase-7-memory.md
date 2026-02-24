# Phase 7 Session Memory — US6 Release Pipeline

**Date**: 2026-02-23  
**Branch**: `003-agent-intercom-release`  
**Spec**: `specs/003-agent-intercom-release/`  
**Phase**: 7 — User Story 6: Release Pipeline (T080–T093)

## What Was Built

### Feature Flag Infrastructure (T081, T084)

Added `[features]` section to `Cargo.toml`:
```toml
[features]
default = []
rmcp-upgrade = []
```
- `default = []`: no features enabled by default
- `rmcp-upgrade = []`: placeholder gate for the rmcp 0.13 upgrade (Phase 8 / US5)
- Resolves compiler warnings about unexpected `cfg` conditions in test code

### Version Tests (T080, T081)

New file `tests/unit/version_tests.rs` with 4 tests:
1. `cargo_pkg_version_is_valid_semver` — verifies `CARGO_PKG_VERSION` has MAJOR.MINOR.PATCH format
2. `cargo_pkg_version_matches_expected_prefix` — verifies version starts with a digit
3. `rmcp_upgrade_feature_is_not_enabled_by_default` — asserts `cfg!(feature = "rmcp-upgrade")` is false; uses `#[allow(clippy::assertions_on_constants)]` since the cfg! result is compile-time constant (intentional)
4. `rmcp_upgrade_gated_fn_is_absent_in_default_build` — gated with `#[cfg(not(feature = "rmcp-upgrade"))]`, its presence in test output confirms the feature is not in `default`

### Release Workflow (T085–T090)

Rewrote `.github/workflows/release.yml`. Key changes from the old version:
- **Binary names fixed**: `monocoque-agent-rc` → `agent-intercom`, `monocoque-ctl` → `agent-intercom-ctl`
- **Archive names fixed**: `monocoque-agent-rc-${TAG}-${target}` → `agent-intercom-${TAG}-${target}`
- **`fail-fast: true`** (was `false`) — partial platform failure aborts the entire release (S056)
- **`changelog` job** using `orhun/git-cliff-action@v3` — generates changelog from conventional commits (no `cliff.toml` required; uses built-in preset) (T088)
- **`cross` install step** via `taiki-e/install-action@v2` when `use_cross: true` in matrix — currently false for all 4 native targets; available for future non-native triples (T086)
- **`config.toml.example`** included in archives instead of `config.toml` (prevents credential leaks in release archives)
- **Release job depends on both `changelog` and `build`** — requires all platform builds to succeed before publishing
- **Quality gate job** runs `fmt --check`, `clippy (pedantic)`, and `cargo test` before building
- **Pre-release detection**: `prerelease: ${{ contains(github.ref_name, '-pre') || ... }}`

### Release Archive Configuration (T091)

New file `config.toml.example` with:
- All placeholder values (no real workspace paths, no credentials)
- Full inline documentation for every config field
- Credential loading instructions in doc header (env vars and keychain alternative)
- Channel configuration example showing `.vscode/mcp.json` pattern

### `--version` Implementation (T083)

Already present in both binaries from Phase 2 — clap's `version` attribute on `#[command(...)]` auto-generates `--version` using `CARGO_PKG_VERSION`. No code change needed.

## Architectural Decisions

### No ADR recorded
This phase was primarily CI/CD infrastructure and test additions. No architectural decisions warranted a formal ADR:
- Native runners were chosen over `cross` for the 4 target platforms (macOS ARM on `macos-latest`, macOS Intel on `macos-13`, Linux on `ubuntu-22.04`, Windows on `windows-latest`) — simplest and most reliable approach
- `git-cliff` was chosen over `release-drafter` for changelog since it uses conventional commit format already used in this project

## Quality Gates Passed

| Gate | Result |
|---|---|
| `cargo check` | ✅ Finished dev profile |
| `cargo test` | ✅ 552 tests pass (4 new from version_tests.rs) |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ 0 warnings/errors |
| `cargo fmt --all -- --check` | ✅ 0 formatting violations |
| `cargo build --release` | ✅ Finished release profile in 28s |

## Test Count Delta

| Tier | Before Phase 7 | After Phase 7 |
|---|---|---|
| Unit | 150 | 154 |
| Contract | 170 | 170 |
| Integration | 210 | 210 |
| Other | 18 | 18 |
| **Total** | **548** | **552** |

## Files Modified

| File | Change |
|---|---|
| `Cargo.toml` | Added `[features]` with `default = []` and `rmcp-upgrade = []` |
| `.github/workflows/release.yml` | Full rewrite: new binary/archive names, fail-fast, git-cliff, cross, config.toml.example |
| `config.toml.example` | NEW — release archive config with placeholder values |
| `tests/unit/version_tests.rs` | NEW — 4 tests for T080/T081 |
| `tests/unit.rs` | Added `mod version_tests` registration |
| `specs/003-agent-intercom-release/tasks.md` | Marked T080–T093 as `[x]` |

## Known Limitations

- T082 (red gate): The `--version` implementation was already in place from Phase 2, so the version tests passed immediately rather than failing. The compiler warning about `unexpected cfg condition value: rmcp-upgrade` served as the observable "red" signal before T084 was implemented.
- Phase 8 (rmcp 0.13 upgrade) will be the most complex phase — it requires rewriting `src/mcp/sse.rs` completely and adapting the `ServerHandler` implementation.

## Next Steps: Phase 8 — rmcp 0.13 Upgrade (T094–T109)

Phase 8 involves:
1. Research current rmcp 0.13 API (check docs/changelog)
2. Write integration tests for StreamableHttpService on `/mcp` endpoint (T094)
3. Write integration test for stdio transport with rmcp 0.13 (T095)
4. Write test for `/sse` redirect/410 response (T096)
5. Update `Cargo.toml` rmcp version to `0.13.0`, feature to `transport-streamable-http-server`
6. Rewrite `src/mcp/sse.rs` — `SseServer` → `StreamableHttpService`
7. Update `src/mcp/handler.rs` for new `ServerHandler` trait
8. Update all integration tests that use SSE transport

This is the highest-risk phase. Budget extra time for API research before coding.
