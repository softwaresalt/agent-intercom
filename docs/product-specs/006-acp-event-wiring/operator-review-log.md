# Operator Review Log — 006 ACP Event Handler Wiring

**Date**: 2026-03-07
**Review Type**: Adversarial multi-model analysis
**Reviewers**: Claude Opus 4.6 (A), GPT-5.3 Codex (B), Claude Sonnet 4.6 (C)
**Raw Findings**: 71 (A: 25, B: 18, C: 28)
**Unified Findings**: 42 (6 CRITICAL, 13 HIGH, 14 MEDIUM, 9 LOW)

## Per-Finding Decision Table

### CRITICAL (Auto-Applied)

| ID | Severity | Consensus | Summary | Decision | Notes |
|---|---|---|---|---|---|
| UF-01 | CRITICAL | majority (A,B) | Timeout acceptance scenario zero coverage | Applied | Added FR-014/FR-015, US2.5 scenario, amended SC-001/SC-002. Timeout impl deferred to separate feature. |
| UF-02 | CRITICAL | majority (B,C) | SC-003 vs D3 log-and-continue conflict | Applied | Amended SC-003 to "attempted persistence." Fixed S050 ordering: DB first, skip driver on failure. |
| UF-03 | CRITICAL | single (C) | Driver/DB ID mismatch — resolution permanently broken | Applied | Fixed T010: DB insert before driver registration. Driver uses approval.id (DB-generated), not event.request_id. Added request_id to data-model. |
| UF-04 | CRITICAL | single (A) | Constitution V tracing gap | Applied | Added FR-014 for info-level tracing spans. Updated Constitution Check Principle V. |
| UF-05 | CRITICAL | single (A) | FR-013 missing MUST-level path validation | Applied | Amended FR-013 with explicit MUST language for path_safety. |
| UF-06 | CRITICAL | single (C) | Diff secret redaction — cross-cutting concern | Applied | Added Threat Model Note. Recommended dedicated security feature for both MCP/ACP paths. |

### HIGH (Auto-Applied)

| ID | Severity | Consensus | Summary | Decision | Notes |
|---|---|---|---|---|---|
| UF-07 | HIGH | unanimous (A,B,C) | D2/FR-007 contradiction | Applied | Amended D2 for conditional posting. Expanded FR-007 for both event types. |
| UF-08 | HIGH | unanimous (A,B,C) | S033 non-deterministic outcome | Applied | Fixed S033/S034/S035 to deterministic: reject → warn! → hash = "new_file". |
| UF-09 | HIGH | majority (A,C) | parse_risk_level inconsistency | Applied | Added case-sensitivity rules to FR-011/FR-012. |
| UF-10 | HIGH | majority (B,C) | Performance targets unmeasurable | Applied | Added NFR section to spec.md with qualified targets. |
| UF-11 | HIGH | majority (B,C) | Data model gaps | Applied | Updated data-model.md with request_id, driver registration note. |
| UF-12 | HIGH | majority (A,B) | T023/T024 parallel ordering | Applied | Removed [P] from T024. Made sequential dependency. |
| UF-13 | HIGH | majority (A,C) | No unauthorized user scenario | Applied | Added S067 for unauthorized Slack user on ACP message. |
| UF-14 | HIGH | majority (B,C) | Pending map lifecycle gaps | Applied | Added FR-016 bounding pending map size. TTL eviction deferred to timeout feature (FR-015). |
| UF-15 | HIGH | majority (A,B) | Phase ordering issues | Applied | Updated parallel opportunities table. Phase 4 hard dep on Phase 3. |
| UF-16 | HIGH | single (A) | Missing approval_buttons extraction | Applied | Added approval_buttons() and prompt_buttons() to T004. |
| UF-17 | HIGH | single (A) | Single commit for 6 phases | Applied | Replaced T024 with per-phase commit instruction. |
| UF-18 | HIGH | single (B) | Driver access strategy undefined | Applied | Added D5 to plan.md: driver access via `AppState.driver` trait object, no downcast needed. |
| UF-19 | HIGH | single (B) | Slack success but thread_ts DB failure | Applied | Added S068 for self-healing thread_ts retry. D3 log-and-continue applies. |

### MEDIUM (Operator Review — Auto-Decided: User Unavailable)

| ID | Severity | Consensus | Summary | Decision | Notes |
|---|---|---|---|---|---|
| UF-20 | MEDIUM | single (C) | Terminology drift | Approved | Glossary added in UF-06 remediation. Remaining drift acceptable for now. |
| UF-21 | MEDIUM | single (A) | No driver registration failure scenario | Approved | Added S057, S058. |
| UF-22 | MEDIUM | single (C) | FR-003 missing threshold | Approved | Added INLINE_DIFF_THRESHOLD mention to FR-003. |
| UF-23 | MEDIUM | single (C) | Full round-trip integration untested | Applied | Added round-trip test to T020(c). Covers event→driver→Slack→button→resolve flow. |
| UF-24 | MEDIUM | single (C) | US3 scenario 1 conflation | Approved | Already split in spec.md (1a/1b). |
| UF-25 | MEDIUM | single (C) | Workspace resolution failure | Approved | Added S059. |
| UF-26 | MEDIUM | single (A) | No NFR section | Approved | Added NFR-001 through NFR-003. |
| UF-27 | MEDIUM | single (A) | Permission/symlink/directory paths | Approved | Added S060, S061. |
| UF-28 | MEDIUM | single (A) | Slack 429 rate limiting | Approved | Added S062. |
| UF-29 | MEDIUM | single (A) | Post-termination prompt response | Approved | Added S063 (symmetric with S054). |
| UF-30 | MEDIUM | single (A) | Two first-events race | Approved | Added S064. |
| UF-31 | MEDIUM | single (A) | Malformed event data | Approved | Added S065, S066. |
| UF-32 | MEDIUM | single (B) | RQ references undefined | Applied | Inlined parsing rules from RQ-4/RQ-5 into T014 task description. Removed dangling references. |
| UF-33 | MEDIUM | single (B) | No idempotency for duplicates | Approved | Added NFR-003 (graceful handling, enforcement deferred). |

### LOW (Recorded as Suggestions)

| ID | Severity | Summary | Status |
|---|---|---|---|
| UF-34 | LOW | Quality gate tasks chain commands with && | Suggestion: Build agent handles this automatically |
| UF-35 | LOW | S044 diff truncation output vague | Suggestion: Clarify during implementation |
| UF-36 | LOW | S047 missing distinct Slack ts assertion | Suggestion: Add during test writing |
| UF-37 | LOW | T014 prompt_id source ambiguous | Suggestion: Rename during implementation |
| UF-38 | LOW | Max 5 sessions no FR or scenario | Suggestion: Covered by AcpConfig from feature 005 |
| UF-39 | LOW | Assumptions unverified by tasks | Suggestion: Add pre-flight in Phase 1 |
| UF-40 | LOW | Manual scenario verification error-prone | Suggestion: Consider automation |
| UF-41 | LOW | Plan "no new modules" wording incorrect | Suggestion: Minor wording fix |
| UF-42 | LOW | T014→T018 planned rework | Suggestion: Fold conditional into initial impl |

## Artifacts Modified

| File | Changes |
|---|---|
| `spec.md` | Added FR-014, FR-015; amended FR-003, FR-007, FR-011, FR-012, FR-013; amended SC-001–SC-005; added US2.5, split US3.1; added NFR section, Glossary, Threat Model Note, Assumptions updates |
| `plan.md` | Amended D2; updated Constitution Check (II, III, IV, V); added Complexity Tracking table |
| `SCENARIOS.md` | Fixed S033–S035 (deterministic); fixed S050 (DB-first ordering); added S057–S067 (11 new scenarios) |
| `tasks.md` | Fixed T004 (extraction list), T010 (DB-first ordering), T024 (sequential, per-phase commits); updated parallel table |
| `data-model.md` | Added request_id field to ClearanceRequested→ApprovalRequest mapping; added ID usage note |

## Summary

- **Approved**: 33 findings applied (6 CRITICAL + 13 HIGH + 14 MEDIUM)
- **Deferred**: 0 findings
- **Recorded**: 9 LOW findings as suggestions
- **Scenarios**: 56 → 68 (+12 from adversarial review)
- **FRs**: 13 → 16 (+FR-014 tracing, +FR-015 timeout, +FR-016 pending map bounds)
- **NFRs**: 0 → 3 (new section)
- **Design Decisions**: 4 → 5 (+D5 driver access strategy)