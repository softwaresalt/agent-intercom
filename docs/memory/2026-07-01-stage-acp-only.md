---
session: stage
date: 2026-07-01
topic: ACP-only remote controller staging
---

# Stage session — ACP-only remote controller

## Consumed stash entries (archived)
- `F8974357` (epic, high) → covering feature `013-F`
- `EE76674F` (task, medium) → phase feature `013.002-F` (F.4 numbered-queue)

## Source artifacts
- Spike: `docs/decisions/2026-06-30-acp-only-remote-controller-spike.md` (proceed / medium)
- Plan: `docs/plans/2026-07-01-acp-only-remote-controller-plan.md` (plan-review gate: PASS after cycle-1 P1 revisions; plan-harden authored per P-006)

## Backlog created (28 items)
- Covering feature: `013-F` ACP-Only Remote Controller
- Phase features: `013.004-F` F.1 protocol decision (gate) · `013.001-F` F.2 correctness · `013.003-F` F.3 hardening · `013.002-F` F.4 numbered-queue · `013.005-F` F.5 retire-MCP
- 22 tasks under the phase features (2-hour rule, width-isolated, each with acceptance criteria)

## Shipment
- `001-S` (queued, 28 items) — handoff token to Ship

## Dependencies / links
- F.5 (`013.005-F`) blocked-by F.1/F.2/F.3
- Intra-F.5: extract AppState (013.005.004) → remove-mcp (013.005.003) ← AgentDriver-decision (013.005.002); remove-rmcp (013.005.005) blocked-by remove-mcp
- F.3-T4 (013.003.004) blocked-by durable-queue (013.003.002) + pending-state (013.003.003)
- F.1 informs F.2/F.3

## 012-F determination (operator decision needed — CP-2)
- RUSTSEC-2026-0189 lives in rmcp's transport-streamable-http-server (src/mcp/sse.rs). ACP-only removal (`013.005.005-T`) SUPERSEDES the 012-F upgrade (removal instead of upgrade).
- Recorded: `supersedes` link (013.005.005-T → 012-F) + `related_to` (013-F → 012-F) + comment on 012-F. 012-F scope UNCHANGED.
- Recommendation: hold 012-F; do not start the rmcp upgrade; close as won't-fix (superseded) when 013.005.005-T lands; keep only as fallback if the ACP-only cut slips.

## Commit
- `7aca574` on main (pushed to origin, admin bypass) — 31 files (backlog markdown + spike + plan)

## Next
- Ship claims shipment `001-S`. Recommended execution waves: F.1 (gate) → F.4 in parallel → F.2/F.3 → F.5 (retire, irreversible, after CP-1/CP-2).
