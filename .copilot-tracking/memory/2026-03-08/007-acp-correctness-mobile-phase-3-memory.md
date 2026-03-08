# Phase 3 Memory — 007-acp-correctness-mobile

**Date**: 2026-03-08  
**Feature**: 007-acp-correctness-mobile  
**Phase**: 3 — US1: Reliable Operator Steering Delivery (F-06)  
**Branch**: `007-acp-correctness-mobile`

---

## What Was Built

Fixed bug **F-06** in `flush_queued_messages` (`src/acp/reader.rs`): the delivery
loop was calling `mark_consumed` unconditionally regardless of whether
`send_prompt` succeeded, causing queued steering messages to be silently
discarded on delivery failure.

### Implementation

Extracted the delivery loop into a new `pub async fn deliver_queued_messages`
that uses a `match` on the `send_prompt` result:

- **`Ok(())`** → call `mark_consumed`; if that also fails, log a warning and continue.
- **`Err(err)`** → log a warning and continue to the next message **without**
  calling `mark_consumed`, so the message remains unconsumed and can be
  retried on the next reconnect flush.

`flush_queued_messages` now delegates to `deliver_queued_messages` and emits
`StreamActivity` events (one per message) after the flush completes.

---

## Files Modified

| File | Change |
|---|---|
| `src/acp/reader.rs` | Added `use crate::models::steering::SteeringMessage`; extracted `pub async fn deliver_queued_messages`; refactored `flush_queued_messages` to delegate to it |
| `tests/unit/acp_reader_steering_delivery.rs` | **New** — 6 unit tests (T001–T006) covering S001–S004, S006, S007 |
| `tests/unit.rs` | Added `mod acp_reader_steering_delivery` |

---

## Key Decisions

1. **Extraction into `pub` function**: `deliver_queued_messages` was made `pub`
   (rather than `pub(crate)`) because unit tests live in `tests/unit/` (external
   to the crate). This enables direct testing of the delivery loop in isolation.

2. **`match` over two separate `if let Err`**: Using `match Ok(()) / Err(err)` makes
   the conditional `mark_consumed` intent explicit and avoids the original
   "call regardless" bug. Clippy pedantic accepted this pattern.

3. **StreamActivity after flush**: The per-message `StreamActivity` emission was
   moved outside `deliver_queued_messages` (into `flush_queued_messages`) to
   keep the extracted function focused on delivery semantics only. Behavior is
   preserved: N events for N messages.

4. **Mock driver with `VecDeque<bool>`**: Used `VecDeque` for ordered response
   control in tests (FIFO `pop_front`), avoiding Clippy warnings about `Vec::remove(0)`.

5. **T006 approach**: To force `mark_consumed` to fail, the test drops the
   `steering_message` table via `sqlx::query("DROP TABLE ...")`. The test
   verifies the function completes without panicking.

---

## Test Results

```
test result: ok. 444 passed; 0 failed; 0 ignored; 0 measured
```

New tests (T001–T006):
- `successful_delivery_marks_message_consumed` ✅
- `failed_delivery_preserves_unconsumed_status` ✅
- `partial_failure_only_failed_message_stays_unconsumed` ✅
- `retry_succeeds_on_next_flush` ✅
- `empty_queue_is_no_op` ✅
- `mark_consumed_failure_after_successful_send_is_handled` ✅

---

## Quality Gates

| Gate | Status |
|---|---|
| `cargo fmt --all -- --check` | ✅ PASS |
| `cargo clippy --all-targets -- -D warnings -D clippy::pedantic` | ✅ PASS |
| `cargo test` | ✅ PASS (444 tests) |
| Commits pushed | ✅ PASS |

---

## Commits

| Hash | Message |
|---|---|
| `918d9c7` | `test(007): add steering delivery reliability tests` |
| `9c6c4bd` | `fix(007): reliable steering delivery - only mark consumed on success (F-06)` |

---

## TDD Sequence

1. Wrote `tests/unit/acp_reader_steering_delivery.rs` referencing `deliver_queued_messages`
2. Ran `cargo test` → **FAIL** (compilation error: function not found) ✓
3. Added `pub async fn deliver_queued_messages` with the F-06 fix applied directly
4. Ran `cargo test` → **PASS** for all 6 new tests ✓
5. Ran full suite → all 444 tests pass ✓
