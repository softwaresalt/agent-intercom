# Harness Tuning Report — 2026-04-15

## Drift Summary

| Category | Count |
|----------|-------|
| Breaking changes (P0) | 6 |
| Degrading changes (P1) | 1 |
| Growth opportunities (P2) | 0 |
| Cosmetic adjustments (P3) | 0 |

## Composition

| Dimension | Installed | Current |
|-----------|-----------|---------|
| Preset | full | full |
| Primary stack pack | mcp-server | mcp-server |
| Stack packs | mcp-server, api-service, cli-tool | mcp-server, api-service, cli-tool |
| Install layers | foundation, instructions, workflow, review, runtime, backlog, knowledge, overlays | foundation, instructions, workflow, review, runtime, backlog, knowledge, overlays |
| Capability packs | agent-intercom, agent-engram, backlogit, browser-verification, strict-safety, adversarial-review | agent-intercom, agent-engram, backlogit, browser-verification, strict-safety, adversarial-review |

> No composition drift detected. Preset, stack packs, layers, and capability packs all match.

## Checksum Scan

| Classification | Count | Details |
|----------------|-------|---------|
| Unchanged | 66 | All match manifest checksums |
| User-modified | 3 | Intentional review feedback fixes (commit eeff322) |
| Missing | 0 | — |
| Ignored | 0 | No drift-ignore file present |

### User-Modified Artifacts

These were modified post-install in commit `eeff322` ("fix: address Copilot review feedback on harness artifacts"). Changes are intentional and should be preserved.

| Artifact | Manifest Hash | Current Hash |
|----------|---------------|--------------|
| `.github/agents/research/learnings-researcher.agent.md` | `0284ce665df0ad3b` | `69aef125ad743e25` |
| `.github/agents/review/constitution-reviewer.agent.md` | `2c854db2bc134d83` | `edf5e5555a48027f` |
| `.github/skills/harness-architect/SKILL.md` | `9611ddc82637cdca` | `09d5d1ee326aa287` |

## Coherence Verification

| Pipeline / Overlay | Status |
|--------------------|--------|
| Risky-plan hardening (impl-plan → stage → plan-harden → plan-review) | ✅ Fully coherent |
| agent-intercom weaving | ✅ Consistent |
| agent-engram weaving | ✅ Consistent |
| backlogit weaving | ✅ Consistent |
| browser-verification weaving | ✅ Consistent |
| strict-safety weaving | ✅ Consistent |
| Instruction glob patterns | ✅ All match existing files |
| Agent → skill references | ✅ No broken references |
| Skill build/test commands | ✅ All accurate |
| Foundational Protocols table (AGENTS.md) | ✅ Present |
| Stop Conditions (constitution) | ✅ Present |

## Artifact Health

| Category | Status |
|----------|--------|
| Instruction glob patterns | ✅ All 23 patterns match ≥1 file |
| Agent skill references | ✅ 0 broken references |
| Skill build/test commands | ✅ All match workspace conventions |
| Manifest coverage | ✅ All active artifacts tracked |
| Deprecated agents | ⚠️ 10 files in `.github/agents/deprecated/` (properly isolated) |

## Proposed Changes (ordered by priority)

### P0 — Breaking

#### TUNE-001: Missing acquire_lock.ps1

```yaml
id: "TUNE-001"
priority: "P0"
category: "breaking"
artifact: "scripts/acquire_lock.ps1"
issue: "concurrency.instructions.md references scripts/acquire_lock.ps1 but the file does not exist"
proposal: "Generate acquire_lock.ps1 from file-lock skill specification"
classification: "missing"
```

#### TUNE-002: Missing acquire_lock.sh

```yaml
id: "TUNE-002"
priority: "P0"
category: "breaking"
artifact: "scripts/acquire_lock.sh"
issue: "concurrency.instructions.md references scripts/acquire_lock.sh but the file does not exist"
proposal: "Generate acquire_lock.sh from file-lock skill specification"
classification: "missing"
```

#### TUNE-003: Missing release_lock.ps1

```yaml
id: "TUNE-003"
priority: "P0"
category: "breaking"
artifact: "scripts/release_lock.ps1"
issue: "concurrency.instructions.md references scripts/release_lock.ps1 but the file does not exist"
proposal: "Generate release_lock.ps1 from file-lock skill specification"
classification: "missing"
```

#### TUNE-004: Missing release_lock.sh

```yaml
id: "TUNE-004"
priority: "P0"
category: "breaking"
artifact: "scripts/release_lock.sh"
issue: "concurrency.instructions.md references scripts/release_lock.sh but the file does not exist"
proposal: "Generate release_lock.sh from file-lock skill specification"
classification: "missing"
```

#### TUNE-005: Missing search.ps1

```yaml
id: "TUNE-005"
priority: "P0"
category: "breaking"
artifact: "scripts/search.ps1"
issue: "skill-search/SKILL.md references scripts/search.ps1 but the file does not exist"
proposal: "Generate search.ps1 from skill-search skill specification"
classification: "missing"
```

#### TUNE-006: Missing search.sh

```yaml
id: "TUNE-006"
priority: "P0"
category: "breaking"
artifact: "scripts/search.sh"
issue: "skill-search/SKILL.md references scripts/search.sh but the file does not exist"
proposal: "Generate search.sh from skill-search skill specification"
classification: "missing"
```

### P1 — Degrading

#### TUNE-007: Deprecated agents still present

```yaml
id: "TUNE-007"
priority: "P1"
category: "degrading"
artifact: ".github/agents/deprecated/"
issue: "10 deprecated agent files remain in .github/agents/deprecated/ — functionality absorbed into active agent/skill set"
proposal: "Remove deprecated agents directory after confirming no active references"
classification: "cleanup"
files:
  - ".github/agents/deprecated/backlog-harvester.agent.md"
  - ".github/agents/deprecated/build-orchestrator.agent.md"
  - ".github/agents/deprecated/doc-ops.agent.md"
  - ".github/agents/deprecated/harness-architect.agent.md"
  - ".github/agents/deprecated/mcp-protocol-reviewer.agent.md"
  - ".github/agents/deprecated/memory.agent.md"
  - ".github/agents/deprecated/pr-review.agent.md"
  - ".github/agents/deprecated/rust-safety-reviewer.agent.md"
  - ".github/agents/deprecated/sqlite-reviewer.agent.md"
  - ".github/agents/deprecated/surrealdb-reviewer.agent.md"
```

## Recommendation

The harness is in excellent health overall. All capability pack overlays are consistently woven, the plan-hardening pipeline is fully coherent, and all cross-references resolve correctly.

The primary gap is 6 missing script artifacts referenced by `concurrency.instructions.md` and `skill-search/SKILL.md`. These are P0 breaking because agents following the concurrency or skill-search protocols will fail at runtime when they attempt to execute these scripts. Generating and installing these scripts will fully resolve the breaking drift.

The deprecated agents in `.github/agents/deprecated/` are a low-priority cleanup item. They are properly isolated and do not interfere with active agents, but removing them reduces confusion.

The 3 user-modified artifacts are intentional review fixes and should be preserved with updated manifest checksums.
