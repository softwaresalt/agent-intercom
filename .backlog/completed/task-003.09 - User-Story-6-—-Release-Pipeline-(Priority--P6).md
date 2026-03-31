---
id: TASK-003.09
title: "003 - User Story 6 — Release Pipeline (Priority: P6)"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-003
dependencies: []
ordinal: 3090
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: A GitHub Actions workflow that triggers on semver tags, produces cross-platform binaries, generates a changelog, and publishes to GitHub Releases. Feature flags gate unreleased capabilities.

**Independent Test**: Trigger the release workflow on a test tag; confirm it produces correctly named archives for all 4 target platforms with proper version metadata.

**Scenarios covered**: S048–S056

### Tests for User Story 6 ⚠️

- [x] T080 [P] [US6] Write unit test verifying `--version` flag outputs version matching `env!("CARGO_PKG_VERSION")` for `agent-intercom` binary in `tests/unit/` or `tests/integration/` (S053)
- [x] T081 [P] [US6] Write unit test verifying feature flag compile-time gating works: a `#[cfg(feature = "...")]` gated function is absent when feature is not enabled (S054, S055)
- [x] T082 [US6] Run tests and confirm new assertions FAIL (red gate)

### Implementation for User Story 6

- [x] T083 [US6] Add `--version` flag handling using `env!("CARGO_PKG_VERSION")` to `src/main.rs` (clap `version` attribute) and `ctl/main.rs` (FR-037)
- [x] T084 [US6] Add `[features]` section to `Cargo.toml` with `default = []` and placeholder feature flag (e.g., `rmcp-upgrade = []`) (FR-036)
- [x] T085 [P] [US6] Create `.github/workflows/release.yml` with trigger on `v*.*.*` tags and build matrix for 4 targets: `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin` (FR-033)
- [x] T086 [P] [US6] Add `cross` compilation steps in release workflow for Linux and macOS targets
- [x] T087 [P] [US6] Add archive packaging step to produce `agent-intercom-v{version}-{target}.{zip|tar.gz}` containing server binary, CLI binary, and `config.toml.example` (FR-034)
- [x] T088 [P] [US6] Add `git-cliff` changelog generation step in release workflow (FR-035)
- [x] T089 [US6] Add `softprops/action-gh-release` step to publish archives and changelog to GitHub Releases
- [x] T090 [US6] Add failure handling: ensure partial platform failure aborts entire release (S056)
- [x] T091 [US6] Create `config.toml.example` from current `config.toml` with placeholder values for release archive inclusion
- [x] T092 [US6] Run `cargo test` with feature flag tests passing (green gate) — EXIT GATE for Phase 7
- [x] T093 [US6] Run local `cargo build --release` and verify binary names match `agent-intercom` / `agent-intercom-ctl` (S002, S003)

**Checkpoint**: Release pipeline workflow exists, feature flags work, --version outputs correct version, local release build produces correctly named binaries.

---

<!-- SECTION:DESCRIPTION:END -->
