# ADR-0018: Keep the AgentDriver Trait After MCP Removal

**Status**: Accepted
**Date**: 2026-07-03
**Phase**: F.5 (Make ACP default and retire MCP), Task 013.005.002-T
**Relates to**: ADR-0014 (AgentDriver trait — protocol abstraction)

## Context

ADR-0014 introduced the `AgentDriver` trait (`src/driver/mod.rs`) to decouple
the shared application core (Slack handlers, orchestrator, persistence) from the
agent communication protocol, with two implementations: `McpDriver` and
`AcpDriver`. Phase F.5 removes the MCP surface (task 013.005.003-T deletes
`McpDriver` and `src/mcp/*`).

The spike raised open questions Q3/Q4: once ACP is the **only** protocol, is the
`AgentDriver` trait still worth keeping, or should it be collapsed into a
concrete `Arc<AcpDriver>` to remove a now-seemingly-single-implementation
abstraction? This ADR records that decision so its blast radius is bounded and
decided before the removal work.

### Findings

* **The trait is not single-implementation after removal.** Two `impl
  AgentDriver` blocks survive the MCP cut:
  * `AcpDriver` — production (`src/driver/acp_driver.rs`).
  * `MockDriver` — test double (`tests/unit/acp_reader_steering_delivery.rs`),
    used to unit-test `acp::reader::deliver_queued_messages` (the F.2-T1
    exactly-once steering delivery + retry semantics) across success, failure,
    and partial-failure paths.
* **The trait is an active dependency-injection seam.** `deliver_queued_messages`
  takes `driver: &dyn AgentDriver`, and the stall consumer takes
  `Option<Arc<dyn AgentDriver>>` (`src/orchestrator/stall_consumer.rs`). These
  are driven by the mock in tests. `AcpDriver` itself is not directly testable
  in isolation because it holds live per-session stream channels.
* **`AgentEvent`** (the event enum in `src/driver/mod.rs`) is protocol-neutral
  and unaffected by this decision; it stays regardless.
* **Cost of keeping is negligible.** The trait is invoked on operator actions
  (resolve clearance/prompt/wait, send prompt, interrupt), on reconnect-flush
  delivery (`acp::reader::deliver_queued_messages` — one `dyn` dispatch per
  queued steering message), and on ACP stall-consumer auto-nudges. None of
  these are high-frequency hot paths, so the dynamic-dispatch cost is
  immaterial.
* **Cost of collapsing is real.** Replacing `Arc<dyn AgentDriver>` with
  `Arc<AcpDriver>` would touch `state::AppState`, `acp::reader`,
  `orchestrator::stall_consumer`, `main.rs`, and the tests — churn during an
  already-irreversible phase — and would forfeit the `MockDriver` test seam.

## Decision

**Keep the `AgentDriver` trait.** No call-site changes are made in this task.

Rationale:

1. It remains a genuine two-implementation abstraction (production + test
   double), not a speculative single-impl trait.
2. It is the dependency-injection seam that makes reliability-critical delivery
   and stall logic unit-testable without live agent streams.
3. Keeping it costs effectively nothing at runtime.
4. Collapsing would add churn and remove test coverage during the irreversible
   MCP-removal phase — a poor trade.

## Consequences

### Positive

* `deliver_queued_messages` and stall-consumer tests continue to inject
  `MockDriver`; no test coverage is lost.
* The removal task (013.005.003-T) is simpler: it deletes `McpDriver`'s `impl`
  and file, and the `driver` field type (`Arc<dyn AgentDriver>`) is unchanged.
* A future second protocol (or an alternate transport) can be added behind the
  same seam without re-introducing an abstraction.

### Negative / Follow-ups

* `McpDriver::new_empty()` is a **test-only** convenience constructor used by
  ~24 test call sites to populate `AppState.driver`; it has **no production
  callers** (`main.rs` builds `McpDriver::new(...)` in the MCP-mode branch and
  `AcpDriver::new()` coerced to `Arc<dyn AgentDriver>` in ACP mode). When
  013.005.003-T removes `McpDriver`, those test call sites must be repointed to
  a non-MCP default driver (e.g. `AcpDriver::new()` or a shared test helper),
  and the MCP-mode driver branch in `main.rs` is deleted along with the rest of
  the MCP surface. These are required parts of the removal task, not this one.

### Neutral

* ADR-0014 is not superseded — its protocol-abstraction rationale still holds;
  this ADR narrows it to a single production protocol while retaining the seam.
