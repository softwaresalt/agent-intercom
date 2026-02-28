# Contract: ACP Stream Protocol

**Feature**: 005-intercom-acp-server
**Date**: 2026-02-28

## Overview

Defines the wire format for bidirectional communication between agent-intercom (server) and a headless agent process over stdio. Uses newline-delimited JSON (NDJSON) — one JSON object per line, `\n` delimiter.

## Framing

- **Codec**: `tokio_util::codec::LinesCodec` wrapping `ChildStdout` (read) and `ChildStdin` (write)
- **Encoding**: UTF-8
- **Delimiter**: `\n` (LF)
- **Max line length**: 1 MB (configurable via `LinesCodec::new_with_max_length`)

## Message Envelope

All messages follow a JSON-RPC-like envelope:

```json
{
  "method": "clearance/request",
  "id": "optional-correlation-id",
  "params": { ... }
}
```

- `method` (string, required): Message type identifier
- `id` (string, optional): Correlation ID for request/response pairs. Present on requests that expect a response.
- `params` (object, required): Method-specific payload

## Inbound Messages (Agent → Server)

### `clearance/request`

Agent requests operator approval for a file operation.

```json
{
  "method": "clearance/request",
  "id": "req-001",
  "params": {
    "title": "Create new module",
    "description": "Adding src/driver/mod.rs",
    "diff": "--- /dev/null\n+++ b/src/driver/mod.rs\n@@ ...",
    "file_path": "src/driver/mod.rs",
    "risk_level": "low"
  }
}
```

**Response expected**: `clearance/response` with matching `id`.

### `status/update`

Agent sends a status or log message.

```json
{
  "method": "status/update",
  "params": {
    "message": "Running cargo test..."
  }
}
```

**Response expected**: None (fire-and-forget).

### `prompt/forward`

Agent forwards a continuation prompt for operator decision.

```json
{
  "method": "prompt/forward",
  "id": "prompt-001",
  "params": {
    "text": "Should I refactor the error handling?",
    "type": "continuation"
  }
}
```

**Response expected**: `prompt/response` with matching `id`.

### `heartbeat`

Agent sends a liveness signal.

```json
{
  "method": "heartbeat",
  "params": {
    "progress": [
      { "label": "Writing tests", "status": "in_progress" },
      { "label": "Implementation", "status": "pending" }
    ]
  }
}
```

**Response expected**: Optional — server may return pending steering messages.

## Outbound Messages (Server → Agent)

### `clearance/response`

Operator's decision on a clearance request.

```json
{
  "method": "clearance/response",
  "id": "req-001",
  "params": {
    "status": "approved",
    "reason": null
  }
}
```

### `prompt/send`

New prompt or instruction from the operator.

```json
{
  "method": "prompt/send",
  "params": {
    "text": "Focus on the error handling module next."
  }
}
```

### `prompt/response`

Decision on a forwarded continuation prompt.

```json
{
  "method": "prompt/response",
  "id": "prompt-001",
  "params": {
    "decision": "continue",
    "instruction": null
  }
}
```

### `session/interrupt`

Request agent to stop current work.

```json
{
  "method": "session/interrupt",
  "params": {
    "reason": "Operator requested termination"
  }
}
```

### `nudge`

Stall recovery message.

```json
{
  "method": "nudge",
  "params": {
    "message": "Continue working on the current task. Pick up where you left off."
  }
}
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Malformed JSON line | Log warning with raw line content, skip, continue reading |
| Unknown method | Log debug, skip, continue reading |
| Missing required field | Log warning with method name and missing field, skip message |
| Stream EOF (stdout closed) | Emit `SessionTerminated` event with reason "stream closed" |
| Write to closed stdin | Return `AppError::Acp("write failed: stream closed")` |

## Codec Configuration

```toml
[acp]
framing = "ndjson"          # or "lsp" for Content-Length framing
max_line_length = 1048576   # 1 MB
startup_timeout_seconds = 30
```

## Test Contract

1. **Single message parsing** — complete JSON line → parsed `AgentEvent`
2. **Batched messages** — two messages in one read → two separate events
3. **Partial delivery** — split JSON across reads → single complete event after reassembly
4. **Malformed line** — invalid JSON → logged and skipped, stream continues
5. **Unknown method** — valid JSON, unknown method → logged and skipped
6. **Stream EOF** — stdout closes → `SessionTerminated` event emitted
7. **Write serialization** — `clearance/response` → valid NDJSON line with correct `id` correlation
