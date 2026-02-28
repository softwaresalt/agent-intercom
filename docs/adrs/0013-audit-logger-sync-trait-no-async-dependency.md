# ADR-0013: Synchronous AuditLogger Trait (No async-trait Dependency)

**Status**: Accepted
**Date**: 2026-02-25
**Phase**: 004-intercom-advanced-features Phase 1 (T004-T005)

## Context

The `AuditLogger` trait introduced in Phase 1 needs to be usable from multiple
contexts: MCP tool handlers (async), Slack event handlers (async), session
lifecycle callbacks (async), and potentially future synchronous contexts.

Rust's built-in `async fn` in traits (stabilized in Rust 1.75) works for
concrete types but cannot be used with `dyn Trait` without boxing — effectively
requiring the `async-trait` crate or manual `Box<dyn Future>` returns. The
`async-trait` crate adds a proc-macro dependency and introduces heap allocation
per call. The project constitution forbids new external dependencies unless
justified.

JSONL file writes are low-latency operations (microseconds for buffered I/O).
The audit log is a fire-and-forget side channel — callers do not need
back-pressure or cancellation on individual writes.

## Decision

Define `AuditLogger` as a synchronous trait:

```rust
pub trait AuditLogger: Send + Sync {
    fn log_entry(&self, entry: AuditEntry) -> crate::Result<()>;
}
```

`JsonlAuditWriter` uses `std::sync::Mutex<Option<WriterState>>` internally to
protect the daily-rotating `BufWriter<File>`. The mutex is uncontended in the
common case (single writer).

Async callers that need to isolate blocking I/O from the tokio runtime use
`tokio::task::spawn_blocking`. For audit logging — a non-critical background
activity — direct synchronous calls from async code are also acceptable given
the sub-millisecond write latency.

## Consequences

**Positive**:
- No new crate dependencies (`async-trait` not added)
- Simple, inspectable implementation — no hidden heap allocation per call
- Usable from both sync and async code without trait object complexity
- `std::sync::Mutex` is simpler than `tokio::sync::Mutex` for this use case

**Negative**:
- Callers in hot async paths should use `spawn_blocking` to avoid blocking the
  tokio executor, adding minor call-site complexity
- If a future implementation requires truly async writes (e.g., remote audit
  sinks), the trait signature would need to change or a separate async variant
  would need to be introduced

**Risk**: Low. Audit writes are infrequent relative to tokio task scheduling
intervals. The `BufWriter` flushes after every entry, bounding latency.
