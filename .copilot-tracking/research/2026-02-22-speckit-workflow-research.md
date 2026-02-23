# Spec-Kit (Specify) Workflow Research

**Date**: 2026-02-22  
**Purpose**: Comprehensive inventory and analysis of the existing spec-kit/specify workflow, agents, skills, prompts, and orchestration patterns in the monocoque-agent-rc workspace.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Directory Structure](#2-directory-structure)
3. [Spec-Kit Configuration (.specify/)](#3-spec-kit-configuration)
4. [Agents Inventory (.github/agents/)](#4-agents-inventory)
5. [Skills Inventory (.github/skills/)](#5-skills-inventory)
6. [Prompts Inventory (.github/prompts/)](#6-prompts-inventory)
7. [Spec-Kit Workflow Pipeline](#7-spec-kit-workflow-pipeline)
8. [RPI Agent Orchestration Pattern](#8-rpi-agent-orchestration-pattern)
9. [Build Orchestrator Pattern](#9-build-orchestrator-pattern)
10. [Agent Cross-Reference Matrix](#10-agent-cross-reference-matrix)
11. [Key Patterns & Conventions](#11-key-patterns--conventions)
12. [Recommendations for Orchestration Agent/Skill](#12-recommendations)

---

## 1. Architecture Overview

The workspace uses two **parallel orchestration systems**:

| System | Entry Point | Workflow | Purpose |
|--------|------------|----------|---------|
| **Spec-Kit (speckit.*)** | `/speckit.specify` | Linear pipeline: specify → clarify → plan → behavior → tasks → analyze → implement | Feature specification and implementation following SDD (Spec-Driven Development) |
| **RPI Agent** | `/rpi` | Iterative cycle: Research → Plan → Implement → Review → Discover | Ad-hoc task execution via subagent dispatch |
| **Build Orchestrator** | `build-orchestrator` agent | Phase loop: single phase or full-spec sequential | Phase-by-phase feature builds using the build-feature skill |

### Common Infrastructure

Both systems share:
- `.specify/memory/constitution.md` — project constitution (6 principles)
- `.specify/templates/` — document templates
- `.specify/scripts/powershell/` — helper scripts for branch/path management
- `.github/copilot-instructions.md` — auto-generated development guidelines
- `.copilot-tracking/` — session artifacts (memory, checkpoints, plans, changes, reviews)

---

## 2. Directory Structure

```
.specify/
├── memory/
│   └── constitution.md           # Project constitution (v1.1.0)
├── scripts/
│   └── powershell/
│       ├── common.ps1            # Shared functions (Get-RepoRoot, Get-CurrentBranch, etc.)
│       ├── check-prerequisites.ps1  # Validates feature branch, paths, required docs
│       ├── create-new-feature.ps1   # Creates feature branch + spec directory
│       ├── setup-plan.ps1           # Copies plan template to feature dir
│       └── update-agent-context.ps1 # Updates agent context files (18+ agent types)
└── templates/
    ├── agent-file-template.md       # Template for auto-generated agent context files
    ├── checklist-template.md        # Requirements quality checklist template
    ├── constitution-template.md     # Project constitution template
    ├── plan-template.md             # Implementation plan template
    ├── scenarios-template.md        # Behavioral matrix (SCENARIOS.md) template
    ├── spec-template.md             # Feature specification template
    └── tasks-template.md            # Task breakdown template

.github/
├── agents/          # 26 agent files
├── copilot-instructions.md  # Auto-generated development guidelines
├── instructions/    # (empty)
├── prompts/         # 10 prompt files (thin wrappers → agents)
├── skills/          # 3 skill directories
└── workflows/       # ci.yml, release.yml
```

---

## 3. Spec-Kit Configuration

### 3.1 Constitution (`.specify/memory/constitution.md`)

**Version**: 1.1.0 (amended from 1.0.0 for SQLite migration)

**6 Core Principles**:

| # | Principle | Summary |
|---|-----------|---------|
| I | Safety-First Rust | `#![forbid(unsafe_code)]`, clippy pedantic, no `unwrap()`/`expect()` |
| II | MCP Protocol Fidelity | All tools visible, descriptive errors for inapplicable calls |
| III | Test-First Development | TDD mandatory, three test tiers (contract/integration/unit) |
| IV | Security Boundary Enforcement | Path validation, keychain credentials, session ownership |
| V | Structured Observability | Tracing spans for all significant operations |
| VI | Single-Binary Simplicity | Two binaries, SQLite via sqlx, minimal dependencies |

**Development Workflow** (from constitution):
1. Feature specs first → `specs/###-feature-name/spec.md`
2. Plan before code → speckit workflow
3. Branch per feature
4. Contract-first design
5. Commit discipline (conventional commits)
6. No dead code

### 3.2 Templates

| Template | Purpose | Used By Agent |
|----------|---------|---------------|
| `spec-template.md` | Feature specification with user stories, FRs, success criteria | `speckit.specify` |
| `plan-template.md` | Implementation plan with technical context, constitution check, project structure | `speckit.plan` |
| `tasks-template.md` | Phase-organized task breakdown by user story | `speckit.tasks` |
| `scenarios-template.md` | Behavioral matrix with deterministic scenario rows | `speckit.behavior` |
| `checklist-template.md` | Requirements quality checklist (unit tests for English) | `speckit.checklist` |
| `constitution-template.md` | Project constitution with principles, governance | `speckit.constitution` |
| `agent-file-template.md` | Auto-generated agent context (copilot-instructions.md, CLAUDE.md, etc.) | `update-agent-context.ps1` |

### 3.3 Scripts (.specify/scripts/powershell/)

| Script | Purpose | Called By |
|--------|---------|-----------|
| `common.ps1` | Shared functions: `Get-RepoRoot`, `Get-CurrentBranch`, `Test-FeatureBranch`, `Get-FeaturePathsEnv` | All other scripts |
| `check-prerequisites.ps1` | Validates feature branch, paths, available docs. Flags: `-Json`, `-RequireTasks`, `-IncludeTasks`, `-PathsOnly` | `speckit.behavior`, `speckit.tasks`, `speckit.analyze`, `speckit.checklist`, `speckit.clarify`, `speckit.implement` |
| `create-new-feature.ps1` | Creates feature branch + spec directory structure. Auto-detects next branch number. | `speckit.specify` |
| `setup-plan.ps1` | Copies plan template to feature dir, outputs paths | `speckit.plan` |
| `update-agent-context.ps1` | Updates 18+ agent context files (CLAUDE.md, copilot-instructions.md, etc.) from plan data | `speckit.plan` (Phase 1 agent context update step) |

---

## 4. Agents Inventory

### 4.1 Spec-Kit Agents (10 agents)

These form the core SDD pipeline:

| Agent File | Description | Handoffs | Role |
|------------|-------------|----------|------|
| `speckit.specify.agent.md` | Create feature spec from natural language | → `speckit.plan`, `speckit.clarify` | **Entry point**: generates spec.md, checklists, branch |
| `speckit.clarify.agent.md` | Interactive spec clarification (max 5 questions) | → `speckit.plan` | Reduces ambiguity in spec via Socratic questioning |
| `speckit.plan.agent.md` | Generate implementation plan | → `speckit.behavior`, `speckit.tasks`, `speckit.checklist` | Produces plan.md, research.md, data-model.md, contracts/, quickstart.md |
| `speckit.behavior.agent.md` | Generate behavioral matrix (SCENARIOS.md) | → `speckit.tasks`, `speckit.analyze` | Maps all permutations, edge cases, expected outcomes |
| `speckit.tasks.agent.md` | Generate task breakdown | → `speckit.analyze`, `build-orchestrator` | Produces tasks.md organized by user story phases |
| `speckit.analyze.agent.md` | Cross-artifact consistency analysis | → `build-orchestrator` | Read-only analysis of spec/plan/tasks/scenarios alignment |
| `speckit.checklist.agent.md` | Generate requirements quality checklists | (none) | "Unit tests for English" — validates spec quality |
| `speckit.constitution.agent.md` | Create/update project constitution | → `speckit.specify` | Manages `.specify/memory/constitution.md` with versioning |
| `speckit.implement.agent.md` | Execute task plan implementation | (none) | Phase-by-phase task execution with checklist gates |
| `speckit.taskstoissues.agent.md` | Convert tasks to GitHub issues | (none) | Creates GitHub issues from tasks.md via GitHub MCP |

### 4.2 RPI Agent System (5 agents)

The **Research → Plan → Implement → Review → Discover** cycle:

| Agent File | Description | Handoffs | Role |
|------------|-------------|----------|------|
| `rpi-agent.agent.md` | Autonomous orchestrator | Self-referential (continue/suggest/auto) + `memory` | Dispatches task-* subagents through 5-phase workflow |
| `task-researcher.agent.md` | Deep research specialist | → `task-planner` | Produces research docs in `.copilot-tracking/research/` |
| `task-planner.agent.md` | Implementation planner | → `task-implementor` | Produces plans + details in `.copilot-tracking/plans/` and `.copilot-tracking/details/` |
| `task-implementor.agent.md` | Plan executor | → `task-reviewer` | Executes plans, tracks changes in `.copilot-tracking/changes/` |
| `task-reviewer.agent.md` | Implementation reviewer | → `task-researcher`, `task-planner` | Validates work, produces reviews in `.copilot-tracking/reviews/` |

**Key**: The RPI agent **requires `runSubagent` tool** to dispatch subagents. When unavailable, it shows a warning.

### 4.3 Build System Agents (2 agents)

| Agent File | Description | Handoffs | Role |
|------------|-------------|----------|------|
| `build-orchestrator.agent.md` | Phase build coordinator | (none) | Resolves build target, runs build-feature skill, verifies gates (lint/memory/compaction/commit) |
| `rust-engineer.agent.md` | Rust implementation specialist | (none) | Inherits `speckit.implement`, adds Rust-specific coding standards |

### 4.4 Utility Agents (8 agents)

| Agent File | Description | Handoffs | Role |
|------------|-------------|----------|------|
| `memory.agent.md` | Session memory persistence | → `rpi-agent` | Detect/save/continue memory files in `.copilot-tracking/memory/` |
| `pr-review.agent.md` | PR review assistant | (none) | 4-phase review: Initialize → Analyze → Collaborative → Handoff |
| `prd-builder.agent.md` | Product Requirements Document builder | (none) | 7-phase guided PRD creation with state tracking |
| `adr-creation.agent.md` | ADR coaching agent | (none) | 4-phase Socratic ADR creation |
| `doc-ops.agent.md` | Documentation operations | (none) | Pattern compliance, accuracy, gap detection |
| `prompt-builder.agent.md` | Prompt engineering assistant | Self-referential | 5-phase prompt creation/validation with sandbox |
| `security-plan-creator.agent.md` | Security plan builder | (none) | 5-phase security plan from Azure blueprints |
| `arch-diagram-builder.agent.md` | ASCII diagram builder | (none) | Generates ASCII architecture diagrams from IaC |
| `copilot-instructions.md` | Auto-generated dev guidelines | (none) | Updated by `update-agent-context.ps1` from plan data |

---

## 5. Skills Inventory

### 5.1 `build-feature` Skill

**File**: `.github/skills/build-feature/SKILL.md`  
**Input**: `spec-name` (string), `phase-number` (integer)  
**Purpose**: Implements a single phase from a feature spec's task plan.

**10-Step Workflow**:

| Step | Name | Description |
|------|------|-------------|
| 1 | Load Phase Context | Read tasks.md, plan.md, spec.md, data-model.md, contracts/, research.md, constitution |
| 2 | Check Constitution Gate | Verify principles + checklists pass before building |
| 3 | Build Phase (Iterative) | TDD execution with task-type-aware constraint injection |
| 4 | Test Phase (Hard Gate) | `cargo test` + `cargo clippy` + `cargo fmt` — all must exit 0 |
| 5 | Constitution Validation | Re-check safety, errors, docs, async patterns post-build |
| 6 | Record ADRs | Create numbered ADR files for significant decisions |
| 7 | Record Session Memory (Hard Gate) | Write memory file to `.copilot-tracking/memory/` |
| 8 | Pre-Commit Verification | Final fmt/clippy/test cycle + review all changes |
| 9 | Stage, Commit, and Sync | `git add -A`, conventional commit, `git push` |
| 10 | Compact Context (Hard Gate) | Run compact-context skill, verify checkpoint |

**Task-Type Classification** (Step 3):
- Persistence tasks → Database + Error Handling constraints
- MCP tasks → MCP Tools + Error Handling constraints
- Slack tasks → Slack + Error Handling constraints
- Orchestrator tasks → Async + Error Handling constraints
- Diff/Policy/IPC tasks → General Rust + Error Handling constraints

### 5.2 `compact-context` Skill

**File**: `.github/skills/compact-context/SKILL.md`  
**Input**: None (infers from context)  
**Purpose**: Captures session state to a checkpoint file, then compacts conversation history.

**4-Step Workflow**:

| Step | Name | Description |
|------|------|-------------|
| 1 | Gather Session State | Analyze active tasks, files read/modified, decisions, failed approaches, open questions |
| 2 | Write Checkpoint File | Create `.copilot-tracking/checkpoints/{YYYY-MM-DD}-{HHmm}-checkpoint.md` |
| 3 | Report Checkpoint | Report file path, summary, estimated token reduction |
| 4 | Compact History | Run `/compact` or recommend new session with checkpoint |

**Checkpoint file sections**: Task State, Session Summary, Files Modified, Files in Context, Key Decisions, Failed Approaches, Open Questions, Next Steps, Recovery Instructions.

### 5.3 `fix-ci` Skill

**File**: `.github/skills/fix-ci/SKILL.md`  
**Input**: Optional `pr-number`, `owner`, `repo`, `max-iterations` (default 3), `poll-interval` (default 30s), `max-wait` (default 600s)  
**Purpose**: Detects CI failures, reproduces locally, fixes, pushes, and polls until CI passes.

**8-Step Workflow**:

| Step | Name | Description |
|------|------|-------------|
| 1 | Identify Target PR | Auto-detect branch + PR via GitHub MCP |
| 2 | Check CI Status | Poll check run statuses |
| 3 | Reproduce Locally | Run failing checks (fmt → clippy → test → audit) |
| 4 | Diagnose and Fix | Apply targeted fixes per check type |
| 5 | Local CI Gate (Hard) | All 4 checks must exit 0 |
| 6 | Stage, Commit, Push | Conventional commit + push |
| 7 | Poll Remote CI | Wait for remote checks, iterate if failures remain |
| 8 | Completion Report | Summary of iterations, commits, fixes |

---

## 6. Prompts Inventory

All 10 prompt files are **thin wrappers** that delegate to their corresponding agent:

| Prompt File | Agent Reference |
|-------------|-----------------|
| `speckit.specify.prompt.md` | `agent: speckit.specify` |
| `speckit.clarify.prompt.md` | `agent: speckit.clarify` |
| `speckit.plan.prompt.md` | `agent: speckit.plan` |
| `speckit.behavior.prompt.md` | `agent: speckit.behavior` |
| `speckit.tasks.prompt.md` | `agent: speckit.tasks` |
| `speckit.analyze.prompt.md` | `agent: speckit.analyze` |
| `speckit.checklist.prompt.md` | `agent: speckit.checklist` |
| `speckit.constitution.prompt.md` | `agent: speckit.constitution` |
| `speckit.implement.prompt.md` | `agent: speckit.implement` |
| `speckit.taskstoissues.prompt.md` | `agent: speckit.taskstoissues` |

**Pattern**: Each prompt file contains only YAML frontmatter with `agent:` reference. The user types `/speckit.specify <description>` which triggers the prompt, which delegates to the agent. The `$ARGUMENTS` variable in the agent captures the user's input.

---

## 7. Spec-Kit Workflow Pipeline

### 7.1 Linear Pipeline (SDD Flow)

```
/speckit.constitution          (optional: create/update constitution)
        │
        ▼
/speckit.specify <description> (create spec.md + branch + checklists)
        │
        ▼
/speckit.clarify               (optional: reduce spec ambiguity, max 5 Qs)
        │
        ▼
/speckit.plan                  (plan.md + research.md + data-model.md + contracts/)
        │
        ▼
/speckit.behavior              (SCENARIOS.md — behavioral matrix)
        │
        ▼
/speckit.tasks                 (tasks.md — phased task breakdown)
        │
        ▼
/speckit.analyze               (read-only consistency analysis)
        │
        ▼
/speckit.implement             (execute tasks.md phase by phase)
   or
Build feature {spec} phase {N} (via build-feature skill)
   or
/speckit.taskstoissues         (convert tasks to GitHub issues)
```

### 7.2 Artifacts Per Feature

Each feature produces artifacts in `specs/###-feature-name/`:

```
specs/###-feature-name/
├── spec.md                    # /speckit.specify
├── plan.md                    # /speckit.plan
├── research.md                # /speckit.plan (Phase 0)
├── data-model.md              # /speckit.plan (Phase 1)
├── quickstart.md              # /speckit.plan (Phase 1)
├── SCENARIOS.md               # /speckit.behavior
├── tasks.md                   # /speckit.tasks
├── contracts/                 # /speckit.plan (Phase 1)
└── checklists/
    ├── requirements.md        # /speckit.specify (auto-generated)
    ├── ux.md                  # /speckit.checklist
    ├── security.md            # /speckit.checklist
    └── ...                    # Additional domain checklists
```

### 7.3 Handoff Chain (Agent Frontmatter)

The `handoffs:` field in each agent enables VS Code Copilot Chat button-based handoffs:

```
speckit.specify ──→ speckit.plan ──→ speckit.behavior ──→ speckit.tasks ──→ speckit.analyze ──→ build-orchestrator
       │                   │                │                    │
       ▼                   ▼                ▼                    ▼
speckit.clarify    speckit.checklist  speckit.tasks        build-orchestrator
```

---

## 8. RPI Agent Orchestration Pattern

### 8.1 Core Architecture

The RPI agent is a **fully autonomous orchestrator** that dispatches specialized task-* agents through a 5-phase iterative cycle. It requires the `runSubagent` tool.

### 8.2 Phase Flow

```
Phase 1: Research  ──→  Phase 2: Plan  ──→  Phase 3: Implement  ──→  Phase 4: Review
    ▲                                                                       │
    └──────────────────── Iterate ──────────────── Escalate ────────────────┘
                                                                            │
                                                                            ▼
                                                                   Phase 5: Discover
                                                                            │
                                                                  ┌─────────┴──────────┐
                                                                  ▼                    ▼
                                                          Auto-continue         Present suggestions
                                                         (back to Phase 1)     (wait for selection)
```

### 8.3 Autonomy Modes

| Mode | Trigger | Behavior |
|------|---------|----------|
| Full autonomy | "auto", "full auto" | Continue automatically |
| Partial (default) | No signal | Auto for obvious items; present options when unclear |
| Manual | "ask me" | Always present options |

### 8.4 Subagent Dispatch Pattern

Each phase dispatches one subagent via `runSubagent`:
- Pass the agent behavior file + workflow prompt file
- Pass user requirements + iteration feedback
- The subagent **does NOT have access to `runSubagent`** itself
- Subagent creates artifacts in `.copilot-tracking/` and returns paths

### 8.5 Tracking Artifacts

```
.copilot-tracking/
├── research/      # task-researcher output
├── plans/         # task-planner output
├── details/       # task-planner output (implementation details)
├── changes/       # task-implementor output
├── reviews/       # task-reviewer output
├── memory/        # memory agent output
├── checkpoints/   # compact-context output
├── sandbox/       # prompt-builder testing
├── subagent/      # subagent research outputs
├── pr/review/     # pr-review tracking
├── doc-ops/       # doc-ops sessions
└── prd-sessions/  # prd-builder state
```

---

## 9. Build Orchestrator Pattern

### 9.1 Architecture

The build orchestrator coordinates phase builds by:
1. Resolving the target spec and phase from `specs/`
2. Running the `build-feature` skill for each phase
3. Verifying 4 mandatory gates after each phase:
   - **Lint & format gate**: `cargo fmt` + `cargo clippy` exit 0
   - **Memory gate**: Memory file exists in `.copilot-tracking/memory/`
   - **Compaction gate**: Checkpoint file exists in `.copilot-tracking/checkpoints/`
   - **Commit gate**: Clean working tree (all changes committed/pushed)
4. In full mode, looping through all incomplete phases sequentially

### 9.2 Two Modes

| Mode | Behavior |
|------|----------|
| `single` (default) | Build one phase and stop |
| `full` | Loop through all incomplete phases with gates between each |

---

## 10. Agent Cross-Reference Matrix

### Which agents reference which other agents/skills?

| Agent | References |
|-------|-----------|
| `speckit.specify` | Handoff → `speckit.plan`, `speckit.clarify` |
| `speckit.clarify` | Handoff → `speckit.plan` |
| `speckit.plan` | Handoff → `speckit.behavior`, `speckit.tasks`, `speckit.checklist`; Calls `update-agent-context.ps1` |
| `speckit.behavior` | Handoff → `speckit.tasks`, `speckit.analyze` |
| `speckit.tasks` | Handoff → `speckit.analyze`, `build-orchestrator` |
| `speckit.analyze` | Handoff → `build-orchestrator` |
| `speckit.constitution` | Handoff → `speckit.specify` |
| `speckit.implement` | Calls `check-prerequisites.ps1`; References constitution in gating |
| `rust-engineer` | Inherits `speckit.implement` behavior; Overrides steps 3, 4, 6, 9 |
| `build-orchestrator` | Invokes `build-feature` skill; References `compact-context` skill |
| `rpi-agent` | Dispatches `task-researcher`, `task-planner`, `task-implementor`, `task-reviewer` via `runSubagent` |
| `memory` | Handoff → `rpi-agent` |
| `build-feature` skill | Reads `rust-engineer.agent.md`; Invokes `compact-context` skill |

### Which templates are used by which agents?

| Template | Used By |
|----------|---------|
| `spec-template.md` | `speckit.specify` |
| `plan-template.md` | `speckit.plan` (via `setup-plan.ps1`) |
| `tasks-template.md` | `speckit.tasks` |
| `scenarios-template.md` | `speckit.behavior` |
| `checklist-template.md` | `speckit.checklist`, `speckit.specify` (quality checklist) |
| `constitution-template.md` | `speckit.constitution` |
| `agent-file-template.md` | `update-agent-context.ps1` |

---

## 11. Key Patterns & Conventions

### 11.1 Agent File Format

All agents use the `.chatagent` format with YAML frontmatter:

```yaml
---
description: Brief description
handoffs:
  - label: Button Label
    agent: target-agent    # Agent name (without .agent.md)
    prompt: Initial prompt text
    send: true/false       # Auto-send or let user edit
tools: [list]              # Optional: explicit tool restrictions
maturity: stable           # Optional
user-invokable: false      # Optional: prevents direct user invocation
argument-hint: text        # Optional: shown in agent picker
---
```

### 11.2 Script Invocation Pattern

All speckit agents follow a common pattern for environment setup:
1. Run a `.specify/scripts/powershell/*.ps1` script with `-Json` flag
2. Parse JSON output for `FEATURE_DIR`, `FEATURE_SPEC`, `IMPL_PLAN`, `TASKS`, `AVAILABLE_DOCS`
3. Use absolute paths for all file operations
4. ERROR if required files are missing (with guidance on which command to run first)

### 11.3 Prompt Delegation Pattern

Prompts are thin YAML-only files in `.github/prompts/` that reference `agent:` in frontmatter. This enables:
- User types `/speckit.specify description` in Copilot Chat
- VS Code loads the prompt file
- Prompt delegates to `speckit.specify.agent.md`
- Agent processes `$ARGUMENTS` containing the user's description

### 11.4 TDD Gate Pattern

Multiple agents enforce TDD:
- `build-feature` skill: Step 3 (red-green), Step 4 (hard gate)
- `speckit.implement`: Step 6 execution rules
- `rust-engineer`: Override Step 6
- Constitution Principle III: Test-First Development

### 11.5 Session Memory Pattern

Session continuity is managed through:
- **Memory files**: `.copilot-tracking/memory/{date}/{description}-memory.md`
- **Checkpoint files**: `.copilot-tracking/checkpoints/{date}-{time}-checkpoint.md`
- **Memory agent**: Detect → Save → Continue lifecycle
- **Build-feature skill**: Step 7 (mandatory memory) + Step 10 (mandatory compaction)

### 11.6 $ARGUMENTS Convention

Every speckit agent and most other agents include:
```markdown
## User Input
\`\`\`text
$ARGUMENTS
\`\`\`
You **MUST** consider the user input before proceeding (if not empty).
```

This captures the text the user typed after the command name.

### 11.7 Handoff Buttons

Agents define handoff buttons in frontmatter that appear in VS Code:
```yaml
handoffs:
  - label: Build Technical Plan
    agent: speckit.plan
    prompt: Create a plan for the spec. I am building with...
    send: true  # Auto-send (true) or pre-fill for user to edit (false)
```

---

## 12. Recommendations for Orchestration Agent/Skill

Based on the observed patterns, here are recommendations for what an orchestration agent/skill should contain:

### 12.1 Gap Analysis

**Current gaps**:
1. **No single orchestrator spans both speckit and build**: `speckit.implement` and `build-orchestrator` are separate from the speckit pipeline. There's no single agent that can drive `specify → plan → behavior → tasks → build all phases → commit`.
2. **`runSubagent` dependency**: The RPI agent requires `runSubagent` which is not always available. The speckit pipeline uses handoff buttons (manual user clicks) instead.
3. **No automated pipeline**: Each speckit step requires the user to manually invoke the next command or click a handoff button.
4. **Build-orchestrator has stale references**: References SurrealDB and `rust-mcp-expert.agent.md` which may not exist.

### 12.2 What an Orchestration Agent Should Contain

Based on the `build-orchestrator` and `rpi-agent` patterns:

1. **Phase inventory management**: Parse `tasks.md` to know what phases exist, which are complete/partial/not-started.
2. **Gate verification between phases**: The 4-gate pattern from build-orchestrator (lint/memory/compaction/commit) is well-defined and should be reused.
3. **Mode support**: Single-phase vs full-loop (from build-orchestrator) and autonomy levels (from RPI agent).
4. **Skill invocation**: Delegate to `build-feature` skill for actual implementation (already well-defined).
5. **Error recovery**: The RPI agent's iterate/escalate pattern for review failures.
6. **Context management**: Mandatory compact-context between phases (from build-orchestrator Step 4 compaction gate).

### 12.3 Recommended Orchestration Architecture

```
Orchestration Agent
├── Input: spec-name, mode (single|full|pipeline)
├── Pipeline mode: specify → plan → behavior → tasks → analyze → build-all
├── Full mode: build all incomplete phases sequentially
├── Single mode: build one phase
├── Gates: lint + memory + compaction + commit between each step
├── Skills: build-feature, compact-context, fix-ci
└── Fallbacks: When runSubagent unavailable, use handoff buttons
```

### 12.4 Pattern to Follow

The **build-orchestrator** agent is the best existing template for an orchestration agent. It has:
- Clear input parameters
- Phase resolution logic
- Skill invocation (delegates to build-feature)
- 4 mandatory gate checks
- Single/full mode support
- Structured completion reporting

The **RPI agent** adds:
- Autonomy mode detection
- Iterative phase cycling with feedback
- Subagent dispatch (when runSubagent available)
- Discovery phase for next-work identification

---

## Appendix: File Path Quick Reference

### Agents (26 total)
```
.github/agents/
├── adr-creation.agent.md
├── arch-diagram-builder.agent.md
├── build-orchestrator.agent.md
├── copilot-instructions.md
├── doc-ops.agent.md
├── memory.agent.md
├── pr-review.agent.md
├── prd-builder.agent.md
├── prompt-builder.agent.md
├── rpi-agent.agent.md
├── rust-engineer.agent.md
├── security-plan-creator.agent.md
├── speckit.analyze.agent.md
├── speckit.behavior.agent.md
├── speckit.checklist.agent.md
├── speckit.clarify.agent.md
├── speckit.constitution.agent.md
├── speckit.implement.agent.md
├── speckit.plan.agent.md
├── speckit.specify.agent.md
├── speckit.tasks.agent.md
├── speckit.taskstoissues.agent.md
├── task-implementor.agent.md
├── task-planner.agent.md
├── task-researcher.agent.md
└── task-reviewer.agent.md
```

### Skills (3 total)
```
.github/skills/
├── build-feature/SKILL.md
├── compact-context/SKILL.md
└── fix-ci/SKILL.md
```

### Prompts (10 total)
```
.github/prompts/
├── speckit.analyze.prompt.md
├── speckit.behavior.prompt.md
├── speckit.checklist.prompt.md
├── speckit.clarify.prompt.md
├── speckit.constitution.prompt.md
├── speckit.implement.prompt.md
├── speckit.plan.prompt.md
├── speckit.specify.prompt.md
├── speckit.tasks.prompt.md
└── speckit.taskstoissues.prompt.md
```

### Templates (7 total)
```
.specify/templates/
├── agent-file-template.md
├── checklist-template.md
├── constitution-template.md
├── plan-template.md
├── scenarios-template.md
├── spec-template.md
└── tasks-template.md
```

### Scripts (5 total)
```
.specify/scripts/powershell/
├── check-prerequisites.ps1
├── common.ps1
├── create-new-feature.ps1
├── setup-plan.ps1
└── update-agent-context.ps1
```
