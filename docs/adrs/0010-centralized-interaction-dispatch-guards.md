# ADR-0010: Centralized Interaction Dispatch Guards

**Status**: Accepted
**Date**: 2026-02-12
**Phase**: 14 (Polish & Cross-Cutting Concerns), Tasks T093, T094

## Context

Prior to Phase 14, authorization checks (FR-013) and double-submission
prevention (FR-022) were implemented independently in each Slack interaction
handler (`approval.rs`, `prompt.rs`, `nudge.rs`, `wait.rs`). While correct,
this approach had two drawbacks:

1. **Scattered authorization** — each new handler must remember to check
   `authorized_user_ids`. A missed check creates a security gap.
2. **Race window for double-submission** — button replacement happened
   *after* handler logic executed. A fast operator tapping twice could
   trigger the handler concurrently before the buttons were removed.

## Decision

Moved both guards into the central dispatch function `handle_interaction`
in `src/slack/events.rs`:

- **Authorization guard (T093)**: A single `is_authorized()` check runs
  before any handler dispatch. Unauthorized users are silently ignored
  (no error feedback to Slack) and logged as security events per SC-009.
  Individual handler auth checks remain as defense-in-depth.

- **Double-submission prevention (T094)**: Before routing to the handler,
  `replace_buttons_with_processing()` calls `chat.update` to replace the
  interactive buttons with a transient "Processing…" indicator. This
  closes the race window: any concurrent tap sees the updated message
  with no actionable buttons. The handler then overwrites the processing
  indicator with its own final status text.

## Consequences

### Positive

- Single enforcement point for authorization eliminates the risk of
  missing auth checks in future handlers.
- Double-submission window reduced from handler execution time (~seconds)
  to Slack API round-trip (~200ms).
- Less duplicated code across handler modules.

### Negative

- Two `chat.update` calls per interaction: one for the processing guard
  and one for the final status. Minimal cost given Slack rate limits.
- Individual handler auth checks are now redundant (but kept for
  defense-in-depth).

### Risks

- If the processing `chat.update` fails (e.g., network blip), the
  handler still executes and will attempt its own button replacement.
  The failure is logged but does not block the handler.
