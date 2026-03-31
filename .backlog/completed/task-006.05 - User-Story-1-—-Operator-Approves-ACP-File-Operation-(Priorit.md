---
id: TASK-006.05
title: "006 - User Story 1 — Operator Approves ACP File Operation (Priority: P1) 🎯 MVP"
status: Done
priority: high
assignee: []
created_date: '2026-03-27 22:39'
labels:
  - task
parent_id: TASK-006
dependencies: []
ordinal: 6050
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->

**Goal**: Wire the `ClearanceRequested` event handler to register pending clearances with `AcpDriver`, persist `ApprovalRequest` records to the database, and post interactive approval messages to Slack with Accept/Reject buttons.

**Independent Test**: Start an ACP session, trigger a clearance request from the agent, observe the Slack approval message with file path / risk level / diff content, click Accept, and verify the agent receives the approval response.

**Functional Requirements**: FR-001, FR-002, FR-003, FR-007, FR-009, FR-010, FR-011, FR-013

### Tests for User Story 1 ⚠️

> **TDD: Write these tests FIRST. Verify they compile and FAIL before proceeding to implementation (T010).**

- [x] T008 [P] [US1] Write unit tests in tests/unit/acp_event_wiring.rs for: (a) risk_level parse-or-default semantics — "low"→Low, "high"→High, "critical"→Critical, unknown string "extreme"→Low, empty ""→Low, mixed-case "High"/"LOW"→Low (S018–S023); (b) SHA-256 content hash computation — file exists→hex digest, file not found→"new_file" sentinel, empty file→SHA-256 of empty bytes, path traversal "../../etc/passwd"→rejected by path_safety, absolute path outside workspace→rejected, null bytes in path→rejected (S030–S035); (c) ClearanceRequested→ApprovalRequest field mapping — session_id direct copy, title direct copy, description→Some(description), diff→unwrap_or_default(), file_path direct copy, risk_level→parsed enum, original_hash→computed, status=Pending, consumed_at=None (S055)
- [x] T009 [P] [US1] Write contract tests in tests/contract/acp_event_contract.rs for ClearanceRequested handler pipeline with mock Slack/DB: standard flow with all fields and low risk (S001), None diff→empty diff_content (S002), high risk level (S003), critical risk level (S004), missing session→warn log+discard event+no side effects (S005), Slack unavailable→persist to DB+register with driver+skip Slack post (S006), DB persistence failure→warn+continue+driver still registered (S007), empty description string (S008), large diff >100KB→stored in full+Slack blocks truncated (S009)

### Implementation for User Story 1

- [x] T010 [US1] Implement `AgentEvent::ClearanceRequested` match arm in src/main.rs `run_acp_event_consumer` (replacing current no-op log)
- [x] T011 [US1] Run quality gates — verify all US1 unit tests (S018–S023, S030–S035, S055) and contract tests (S001–S009) pass: `cargo check && cargo clippy -- -D warnings && cargo fmt --all -- --check && cargo test`

**Checkpoint**: ClearanceRequested events produce interactive approval messages in Slack. User Story 1 independently testable.

---

<!-- SECTION:DESCRIPTION:END -->
