# ADR-0009: IPC JSON-Line Protocol Over Local Sockets

**Status**: Accepted
**Date**: 2026-02-11
**Phase**: 12 (User Story 10), Tasks T087, T088

## Context

Operational mode switching (US10) requires a local control channel so that
operators on the same machine can approve, reject, resume, and switch modes
without relying on Slack. The MCP server already communicates with agents
via stdio/SSE, but needs a separate IPC path for the companion CLI
(`monocoque-ctl`).

Design requirements:

- Cross-platform: Windows named pipes and Unix domain sockets
- Low latency: local-only, no network stack
- Simple framing: single request, single response per connection
- No additional runtime dependencies beyond what the crate already uses

## Decision

Adopted a **JSON-line protocol over `interprocess::local_socket`**:

- Each IPC connection carries exactly one newline-delimited JSON request
  followed by one newline-delimited JSON response, then the connection closes.
- Request format: `{"command": "<verb>", ...optional fields}\n`
- Response format: `{"ok": true|false, "data": ..., "error": "..."}\n`
- The `interprocess` crate (already a dependency) provides `ListenerOptions`
  and `Stream` types that abstract over named pipes (Windows) and Unix domain
  sockets (Linux/macOS) with async tokio support.
- Socket name is derived from the workspace root path to allow multiple
  server instances on the same machine.

Supported commands: `list`, `approve`, `reject`, `resume`, `mode`.

## Alternatives Considered

### HTTP on localhost

Using axum on `127.0.0.1:<port>` would reuse existing HTTP infrastructure
but introduces port allocation conflicts, firewall considerations, and
unnecessary TCP overhead for purely local communication.

### Unix domain sockets without `interprocess`

Direct `tokio::net::UnixListener` works on Linux/macOS but requires
a separate Windows implementation. The `interprocess` crate provides
a single API for both platforms.

### gRPC / Cap'n Proto

Over-engineered for a simple command-response protocol with five verbs.
Would add significant compile-time cost and complexity.

## Consequences

### Positive

- Zero network exposure — IPC is invisible to external hosts
- Simple to debug — JSON payloads are human-readable
- Cross-platform with a single code path
- Connection-per-command model avoids stale connection issues

### Negative

- No multiplexing — each command opens a new connection (acceptable
  given low command frequency from CLI usage)
- Socket name collision is possible if workspace paths differ only by
  case on case-insensitive file systems (mitigated by canonicalization)

### Risks

- The `interprocess` crate's async API is relatively new; breaking changes
  in future major versions may require migration effort
