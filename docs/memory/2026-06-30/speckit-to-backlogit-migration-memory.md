---
type: session-memory
date: 2026-06-30
topic: spec-kit → backlogit migration and removal
branch: chore/migrate-speckit-to-backlogit
---

# Spec-kit → backlogit migration

## Outcome

Migrated the legacy spec-kit `specs/` tree into the backlogit/docs knowledge
system and removed the spec-kit system surfaces. Committed as `f67e4d8` on
branch `chore/migrate-speckit-to-backlogit` (140 files). `main` untouched.

## Starting state (verified)

- Backlogit features `003-F`..`010-F` (all `done`) already represented the 8
  shipped spec-kit features; their bodies already embedded the `spec.md` text.
  `references` was null (they were migrated earlier from a `backlog.md` tool).
- Plans already graduated to `docs/exec-plans/`; research to `docs/research/`.
- Only active code coupling: `mcp-tools.json` loaded via `include_str!` in 5
  contract tests. `mcp-resources.json` referenced in comments only.
- No `.specify/` dir and no spec-kit prompt files existed.

## Changes made

- Relocated contract fixtures `specs/001-.../contracts/{mcp-tools,mcp-resources}.json`
  → `tests/fixtures/contracts/`; updated 5 test `include_str!` paths; `cargo fmt`
  collapsed the now-shorter calls.
- `git mv specs/ → docs/product-specs/` (84 files, lossless).
- Replaced `specs/` → `docs/product-specs/` path references across tracked docs
  and `.backlogit` archive records (lookbehind-guarded to protect `product-specs`).
- Added `references:` frontmatter linking `003-F`..`010-F` to their migrated spec
  folders. Note: the header schema does not project `references` into the index,
  so `backlogit_query_sql` still shows null — the markdown (source of truth) holds it.
- Removed spec-kit doc templates `.vscode/templates/*.md` (5 files) and the
  `.specify/scripts/...` command allow-list entries in `agent-intercom.code-workspace`
  and `.intercom/settings.json`.
- Neutralized `/speckit.*` provenance annotations in `docs/exec-plans/`; pointed a
  stale `.specify/memory/constitution.md` ref at `.github/instructions/constitution.instructions.md`.
- Fixed the project-structure trees in `.github/copilot-instructions.md` and
  `docs/developer-guide.md` to show `docs/product-specs/`.

## Decisions

- Preserved the migrated archive documents' original internal spec-kit text
  (`/speckit`, `.specify` mentions inside `docs/product-specs/**/plan.md`, `tasks.md`,
  `checklists/`, `quickstart.md`). Rationale: they are a faithful historical archive;
  scrubbing them would alter records and risk information loss. The spec-kit *system*
  (root folder, templates, tooling config) is fully removed; the workspace operates
  via backlogit + docs.
- Kept the commit surgical: excluded pre-existing unrelated working-tree changes
  (`.gitignore`, `.engram/`, `.cursor/`, `.mcp.json`, `.github/copilot/`, the
  `2026-06-30-acp-only-remote-controller-spike.md` doc, backlogit runtime jsonl).

## Verification

`cargo fmt --all -- --check` passed, `cargo clippy --all-targets -- -D warnings -D clippy::pedantic`
passed, `cargo test` passed (contract 250, plus unit/integration all green).
`backlogit_doctor` reports no findings. No operational or config surface references
the removed `specs/` folder (source code, tests, active docs, `.engram/registry.yaml`,
`.context/backlog.md`). Dated frozen session logs under `.copilot-tracking/**` and
`.context/memory|sessions/**` intentionally retain their point-in-time references as
accurate historical records and were left untouched.

## Next steps / open questions

- Push the branch and open a PR when ready (not pushed — left for operator review).
- Optional deeper scrub: rewrite spec-kit command text *inside* the archived
  `docs/product-specs/**` documents if a zero-mention end state is desired.
- Optional: de-duplicate `plan.md`/`research.md` that now exist in both
  `docs/product-specs/**` and `docs/exec-plans/`/`docs/research/`.
